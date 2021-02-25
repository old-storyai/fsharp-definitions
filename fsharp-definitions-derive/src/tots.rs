// Copyright 2019 Ian Castleden
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::{is_bytes, last_path_element, FSType, FieldContext};

type SourcePart = String;

fn s(v: &str) -> SourcePart {
    v.to_string()
}
fn stodo(v: &str) -> SourcePart {
    format!("(* SourcePart todo: {} *)", v)
}

impl<'a> FieldContext<'a> {
    #[allow(clippy::cognitive_complexity)]
    fn generic_to_fs(&self, fs: &FSType) -> SourcePart {
        let to_fs = |ty: &syn::Type| self.type_to_fs(ty);
        let name = fs.ident.to_string();
        match name.as_ref() {
            "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64"
            | "i128" | "isize" => s("int64"),
            "f64" | "f32" => s("float"),
            "String" | "str" | "char" | "Path" | "PathBuf" => s("string"),
            "bool" => s("bool"),
            "Box" | "Cow" | "Rc" | "Arc" | "Cell" | "RefCell" if fs.args.len() == 1 => {
                to_fs(&fs.args[0])
            }
            "Duration" => s("{ secs: int64; nanos: int64 }"),
            "SystemTime" => s("{ secs_since_epoch: int64; nanos_since_epoch: int64 }"),
            // std::collections
            "Vec" | "VecDeque" | "LinkedList" if fs.args.len() == 1 => {
                self.type_to_array(&fs.args[0])
            }
            "HashMap" | "BTreeMap" if fs.args.len() == 2 => {
                let k = to_fs(&fs.args[0]);
                let v = to_fs(&fs.args[1]);
                // quote!(Map<#k,#v>)
                // quote!( { [key: #k]:#v } )
                format!("Map<{}, {}>", k, v)
            }
            "HashSet" | "BTreeSet" if fs.args.len() == 1 => {
                let k = to_fs(&fs.args[0]);
                //quote!(Set<#k>)
                // quote! ( #k[] )
                format!("Set<{}>", k)
            }
            "Option" if fs.args.len() == 1 => {
                let k = to_fs(&fs.args[0]);
                format!("{} option", k)
            }
            "Result" if fs.args.len() == 2 => {
                let k = to_fs(&fs.args[0]);
                let v = to_fs(&fs.args[1]);
                // quote!(  { Ok : #k } | { Err : #v }  )
                format!("RsResult<{}, {}>", k, v)
            }
            // "Either" if fs.args.len() == 2 => {
            //     let k = to_fs(&fs.args[0]);
            //     let v = to_fs(&fs.args[1]);
            //     // quote!(  { Left : #k } | { Right : #v }  )
            //     format!("RsResult<{}, {}>", k, v)
            // }
            // "Fn" | "FnOnce" | "FnMut" => {
            //     let args = self.derive_syn_types(&fs.args);
            //     if let Some(ref rt) = fs.return_type {
            //         let rt = to_fs(rt);
            //         quote! { (#(#args),*) => #rt }
            //     } else {
            //         quote! { (#(#args),*) => undefined }
            //     }
            // }
            name_str => {
                let owned = fs.path();
                let path: Vec<&str> = owned.iter().map(|s| s.as_ref()).collect();
                match path[..] {
                    ["serde_json", "Value"] => s("obj"),
                    ["chrono", "DateTime"] => s("string"),
                    _ => {
                        if !fs.args.is_empty() {
                            let args = self.derive_syn_types(&fs.args);
                            let mut src = String::from(name_str);
                            // TODO: This is probably broken, because FSharp renders generics differently
                            src.push('<');
                            src.push_str(&args.collect::<Vec<String>>().join(", "));
                            src.push('>');
                            // quote! { #ident<#(#args),*> }
                            // src
                            todo!("FSharpDefinitions does not yet handle generics for {:?}", fs.ident);
                        } else {
                            name_str.to_string()
                        }
                    }
                }
            }
        }
    }

    fn type_to_array(&self, elem: &syn::Type) -> SourcePart {
        // check for [u8] or Vec<u8>

        if let Some(ty) = self.get_path(elem) {
            if ty.ident == "u8" && is_bytes(&self.field) {
                return stodo("u8 list? string?");
            };
        };

        format!("{} list", self.type_to_fs(elem))
    }
    /// # convert a `syn::Type` rust type to a
    /// `TokenStream` of fsharp type: basically i32 => number etc.
    ///
    /// field is the current Field for which we are trying a conversion
    pub fn type_to_fs(&self, ty: &syn::Type) -> SourcePart {
        // `type_to_fs` recursively calls itself occationally
        // finding a Path which it hands to last_path_element
        // which generates a "simplified" FSType struct which
        // is handed to `generic_to_fs` which possibly "bottoms out"
        // by generating tokens for fsharp types.

        use syn::Type::*;
        use syn::{
            TypeArray, TypeBareFn, TypeGroup, TypeImplTrait, TypeParamBound, TypeParen, TypePath,
            TypePtr, TypeReference, TypeSlice, TypeTraitObject, TypeTuple
        };
        match ty {
            Slice(TypeSlice { elem, .. })
            | Array(TypeArray { elem, .. })
            | Ptr(TypePtr { elem, .. }) => self.type_to_array(elem),
            Reference(TypeReference { elem, .. }) => self.type_to_fs(elem),
            // fn(a: A,b: B, c:C) -> D
            BareFn(TypeBareFn { inputs, .. }) => {
                self.ctxt
                    .err_msg(inputs, "we do not support FSharpifying functions");
                stodo("obj bare fn") // any type?
            }
            Never(..) => stodo("never?"),
            Tuple(TypeTuple { elems, .. }) => elems
                .iter()
                .map(|t| self.type_to_fs(t))
                .collect::<Vec<String>>()
                .join(" * "),

            Path(TypePath { path, .. }) => match last_path_element(&path) {
                Some(ref fs) => self.generic_to_fs(fs),
                _ => stodo("type path?"),
            },
            TraitObject(TypeTraitObject { bounds, .. })
            | ImplTrait(TypeImplTrait { bounds, .. }) => {
                let elems = bounds
                    .iter()
                    .filter_map(|t| match t {
                        TypeParamBound::Trait(t) => last_path_element(&t.path),
                        _ => None, // skip lifetime etc.
                    })
                    .map(|t| self.generic_to_fs(&t))
                    .collect::<Vec<String>>()
                    .join(" + ");

                // TODO check for zero length?
                // A + B + C => A & B & C
                stodo(&format!("trait object {}", elems))
            }
            Paren(TypeParen { elem, .. }) | Group(TypeGroup { elem, .. }) => {
                let tp = self.type_to_fs(elem);
                stodo(&format!("type paren {}", tp))
            }
            Infer(..) | Macro(..) | Verbatim(..) => stodo("infer, macro, or verbatim?"),
            // Recommended way to test exhaustiveness without breaking API https://github.com/dtolnay/syn/releases/tag/1.0.60
            #[cfg(test)]
            syn::Type::__TestExhaustive(_) => unimplemented!(),
            #[cfg(not(test))]
            _ => stodo("other?"),
        }
    }

    pub fn derive_syn_types(
        &'a self,
        types: &'a [syn::Type],
    ) -> impl Iterator<Item = SourcePart> + 'a {
        types.iter().map(move |ty| self.type_to_fs(ty))
    }
}
