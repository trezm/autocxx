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

use syn::{parse_quote, punctuated::Punctuated, FnArg, ReturnType, Token, Type, TypePath};

/// Mod to handle stripping paths off the front of types.

fn unqualify_type_path(mut typ: TypePath) -> TypePath {
    if typ.path.segments.len() > 1 {
        // If we've still got more than one
        // path segment then this is referring to a type within
        // C++ namespaces. Strip them off for now, until cxx supports
        // nested mods within a cxx::bridge.
        // This is 'safe' because earlier code will already have
        // failed with 'DuplicateType' if we had several types called
        // the same thing.
        let last_seg = typ.path.segments.last().unwrap();
        typ.path.segments = parse_quote!(
            #last_seg
        );
    }
    typ
}

fn unqualify_type(typ: Type) -> Type {
    match typ {
        Type::Path(typ) => Type::Path(unqualify_type_path(typ)),
        Type::Reference(mut typeref) => {
            typeref.elem = unqualify_boxed_type(typeref.elem);
            Type::Reference(typeref)
        }
        _ => typ,
    }
}

fn unqualify_boxed_type(typ: Box<Type>) -> Box<Type> {
    Box::new(unqualify_type(*typ))
}

pub(crate) fn unqualify_ret_type(ret_type: ReturnType) -> ReturnType {
    match ret_type {
        ReturnType::Type(tok, boxed_type) => {
            ReturnType::Type(tok, unqualify_boxed_type(boxed_type))
        }
        _ => ret_type,
    }
}

pub(crate) fn unqualify_params(
    params: Punctuated<FnArg, Token![,]>,
) -> Punctuated<FnArg, Token![,]> {
    params
        .into_iter()
        .map(|p| match p {
            FnArg::Typed(mut pt) => {
                pt.ty = unqualify_boxed_type(pt.ty);
                FnArg::Typed(pt)
            }
            _ => p,
        })
        .collect()
}
