// Copyright 2020 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//    https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub use autocxx_engine::Error as EngineError;
pub use autocxx_engine::ParseError;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Errors returned during creation of a cc::Build from an include_cxx
/// macro.
#[derive(Debug)]
pub enum Error {
    /// The cxx module couldn't parse the code generated by autocxx.
    /// This could well be a bug in autocxx.
    InvalidCxx(EngineError),
    /// The .rs file didn't exist or couldn't be parsed.
    ParseError(ParseError),
    /// We couldn't create a temporary directory to store the c++ code.
    TempDirCreationFailed(std::io::Error),
    /// We couldn't write the c++ code to disk.
    FileWriteFail(std::io::Error, PathBuf),
    /// No `include_cxx` macro was found anywhere.
    NoIncludeCxxMacrosFound,
    /// Problem converting the `AUTOCXX_INC` environment variable
    /// to a set of canonical paths.
    IncludeDirProblem(EngineError),
    /// Unable to create one of the directories to which we need to write
    UnableToCreateDirectory(std::io::Error, PathBuf),
}

/// Build autocxx C++ files and return a cc::Build you can use to build
/// more from a build.rs file.
/// You need to provide the Rust file path and the iterator of paths
/// which should be used as include directories.
pub fn build<P1, I, T>(rs_file: P1, autocxx_incs: I) -> Result<cc::Build, Error>
where
    P1: AsRef<Path>,
    I: IntoIterator<Item = T>,
    T: AsRef<OsStr>,
{
    build_to_custom_directory(rs_file, autocxx_incs, out_dir())
}

/// Like build, but you can specify the location where files should be generated.
/// Not generally recommended for use in build scripts.
pub fn build_to_custom_directory<P1, I, T>(
    rs_file: P1,
    autocxx_incs: I,
    outdir: PathBuf,
) -> Result<cc::Build, Error>
where
    P1: AsRef<Path>,
    I: IntoIterator<Item = T>,
    T: AsRef<OsStr>,
{
    let gendir = outdir.join("autocxx-build");
    let incdir = gendir.join("include");
    ensure_created(&incdir)?;
    let cxxdir = gendir.join("cxx");
    ensure_created(&cxxdir)?;
    // We are incredibly unsophisticated in our directory arrangement here
    // compared to cxx. I have no doubt that we will need to replicate just
    // about everything cxx does, in due course...
    let mut builder = cc::Build::new();
    builder.cpp(true);
    // Write cxx.h to that location, as it may be needed by
    // some of our generated code.
    write_to_file(&incdir, "cxx.h", autocxx_engine::HEADER.as_bytes())?;
    let autocxx_inc = build_autocxx_inc(autocxx_incs, &incdir);
    let autocxxes =
        autocxx_engine::parse_file(rs_file, Some(&autocxx_inc)).map_err(Error::ParseError)?;
    let mut counter = 0;
    for include_cpp in autocxxes {
        for inc_dir in include_cpp
            .include_dirs()
            .map_err(Error::IncludeDirProblem)?
        {
            builder.include(inc_dir);
        }
        let generated_code = include_cpp
            .generate_h_and_cxx()
            .map_err(Error::InvalidCxx)?;
        for filepair in generated_code.0 {
            let fname = format!("gen{}.cxx", counter);
            counter += 1;
            let gen_cxx_path = write_to_file(&cxxdir, &fname, &filepair.implementation)?;
            builder.file(gen_cxx_path);

            write_to_file(&incdir, &filepair.header_name, &filepair.header)?;
        }
    }
    if counter == 0 {
        Err(Error::NoIncludeCxxMacrosFound)
    } else {
        // Configure cargo to give the same set of include paths to autocxx
        // when expanding the macro.
        println!("cargo:rustc-env=AUTOCXX_INC={}", autocxx_inc);
        Ok(builder)
    }
}

fn ensure_created(dir: &PathBuf) -> Result<(), Error> {
    std::fs::create_dir_all(dir).map_err(|e| Error::UnableToCreateDirectory(e, dir.clone()))
}

fn out_dir() -> PathBuf {
    std::env::var_os("OUT_DIR").map(PathBuf::from).unwrap()
}

fn build_autocxx_inc<I, T>(paths: I, extra_path: &PathBuf) -> String
where
    I: IntoIterator<Item = T>,
    T: AsRef<OsStr>,
{
    let mut all_paths: Vec<_> = paths
        .into_iter()
        .map(|p| PathBuf::from(p.as_ref()))
        .collect();
    all_paths.push(extra_path.clone());
    std::env::join_paths(all_paths)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
}

fn write_to_file(dir: &PathBuf, filename: &str, content: &[u8]) -> Result<PathBuf, Error> {
    let path = dir.join(filename);
    try_write_to_file(&path, content).map_err(|e| Error::FileWriteFail(e, path.clone()))?;
    Ok(path)
}

fn try_write_to_file(path: &PathBuf, content: &[u8]) -> std::io::Result<()> {
    let mut f = File::create(path)?;
    f.write_all(content)
}
