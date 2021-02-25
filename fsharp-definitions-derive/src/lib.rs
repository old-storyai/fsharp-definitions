// Copyright 2019 Ian Castleden
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Exports serde-serializable structs and enums to FSharp definitions.
//!
//! Please see documentation at [crates.io](https://crates.io/crates/fsharp-definitions)

extern crate proc_macro;
use quote::quote;
use serde_derive_internals::{ast, Ctxt, Derive};
use syn::DeriveInput;

use source_builder::SourceBuilder;

mod attrs;
mod derive_enum;
mod derive_struct;
mod source_builder;
mod tests;
mod tots;
mod utils;

use attrs::Attrs;
use utils::*;

// too many TokenStreams around! give it a different name
type RustQuote = proc_macro2::TokenStream;

struct QuoteMaker {
    pub source: SourceBuilder,
    pub kind: QuoteMakerKind,
}

enum QuoteMakerKind {
    Object,
    Enum,
    Union,
}

/* #region helpers */

/// derive proc_macro to expose FSharp definitions to `wasm-bindgen`.
///
/// Please see documentation at [crates.io](https://crates.io/crates/fsharp-definitions).
#[proc_macro_derive(FSharpDefinition, attributes(fs))]
pub fn derive_fsharp_definition(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = RustQuote::from(input);
    do_derive_fsharp_definition(input).into()
}

fn do_derive_fsharp_definition(input: RustQuote) -> RustQuote {
    let tsy = FSharpify::new(input);
    let parsed = tsy.parse();
    let export_string = parsed.export_type_definition_source().finish();
    let name = tsy.ident.to_string().to_uppercase();

    let export_ident = ident_from_str(&format!("FS_EXPORT_{}", name));

    // #[wasm_bindgen(fsharp_custom_section)]
    let mut q = quote! {
        pub const #export_ident : &'static str = #export_string;
    };

    // just to allow testing... only `--features=test` seems to work
    if cfg!(any(test, feature = "test")) {
        let fsharp_ident = ident_from_str(&format!("{}___fsharp_definition", &tsy.ident));

        q.extend(quote!(
            fn #fsharp_ident ( ) -> &'static str {
                #export_string
            }

        ));
    }
    if let Some("1") = option_env!("FSFY_SHOW_CODE") {
        // eprintln!("{}", &q.to_string());
        eprintln!("{}", &export_string);
    }

    q
}

/* #endregion helpers */

pub(crate) struct FSharpify {
    ident: syn::Ident,
    generics: syn::Generics,
    input: DeriveInput,
}

impl FSharpify {
    pub fn new(input: RustQuote) -> Self {
        let input: DeriveInput = syn::parse2(input).unwrap();

        let cx = Ctxt::new();

        let mut attrs = attrs::Attrs::new();
        attrs.push_doc_comment(&input.attrs);
        attrs.push_attrs(&input.ident, &input.attrs, Some(&cx));

        let container = ast::Container::from_ast(&cx, &input, Derive::Serialize)
            .expect("container was derived from AST");

        // must track this in case of errors so we can check them
        // if we don't consume the errors, we'll get an "unhandled errors" panic whether or not there were errors
        cx.check().unwrap();

        Self {
            generics: container.generics.clone(),
            ident: container.ident,
            input,
        }
    }

    fn parse(&self) -> FSOutput {
        let input = &self.input;
        let cx = Ctxt::new();

        // collect and check #[fs(...attrs)]
        let attrs = {
            let mut attrs = attrs::Attrs::new();
            attrs.push_doc_comment(&input.attrs);
            attrs.push_attrs(&input.ident, &input.attrs, Some(&cx));
            attrs
        };

        let container = ast::Container::from_ast(&cx, &input, Derive::Serialize)
            .expect("container was derived from AST");

        let (fsharp, pctxt) = {
            let pctxt = ParseContext {
                ctxt: Some(cx),
                global_attrs: attrs,
                ident: container.ident.clone(),
            };

            let fsharp = match container.data {
                ast::Data::Enum(ref variants) => pctxt.derive_enum(variants, &container),
                ast::Data::Struct(style, ref fields) => {
                    pctxt.derive_struct(style, fields, &container)
                }
            };

            // erase serde context
            (fsharp, pctxt)
        };

        FSOutput {
            ident: container.ident.to_string(),
            pctxt,
            q_maker: fsharp,
        }
    }
}

struct FSOutput {
    ident: String,
    pctxt: ParseContext,
    q_maker: QuoteMaker,
}

impl FSOutput {
    fn export_type_definition_source(&self) -> SourceBuilder {
        match self.q_maker.kind {
            QuoteMakerKind::Union | QuoteMakerKind::Enum => {
                let mut source_builder = SourceBuilder::simple({
                    &format!(
                        "{}type {} =",
                        self.pctxt.global_attrs.to_comment_str(),
                        self.ident,
                    )
                });
                source_builder.push_source_1(self.q_maker.source.clone());
                source_builder
            }
            QuoteMakerKind::Object => {
                let mut source_builder = SourceBuilder::simple({
                    &format!(
                        "{}type {} =",
                        self.pctxt.global_attrs.to_comment_str(),
                        self.ident,
                    )
                });
                source_builder.push_source_1(self.q_maker.source.clone());
                source_builder
            }
        }
    }
}

fn return_type(rt: &syn::ReturnType) -> Option<syn::Type> {
    match rt {
        syn::ReturnType::Default => None, // e.g. undefined
        syn::ReturnType::Type(_, tp) => Some(*tp.clone()),
    }
}

// represents a fsharp type T<A,B>
struct FSType {
    ident: syn::Ident,
    args: Vec<syn::Type>,
    path: Vec<syn::Ident>,          // full path
    return_type: Option<syn::Type>, // only if function
}

impl FSType {
    fn path(&self) -> Vec<String> {
        self.path.iter().map(|i| i.to_string()).collect() // hold the memory
    }
}

fn last_path_element(path: &syn::Path) -> Option<FSType> {
    let fullpath = path
        .segments
        .iter()
        .map(|s| s.ident.clone())
        .collect::<Vec<_>>();
    match path.segments.last() {
        Some(t) => {
            let ident = t.ident.clone();
            let args = match &t.arguments {
                syn::PathArguments::AngleBracketed(ref path) => &path.args,
                // closures Fn(A,B) -> C
                syn::PathArguments::Parenthesized(ref path) => {
                    let args: Vec<_> = path.inputs.iter().cloned().collect();
                    let ret = return_type(&path.output);
                    return Some(FSType {
                        ident,
                        args,
                        path: fullpath,
                        return_type: ret,
                    });
                }
                syn::PathArguments::None => {
                    return Some(FSType {
                        ident,
                        args: vec![],
                        path: fullpath,
                        return_type: None,
                    });
                }
            };
            // ignore lifetimes
            let args = args
                .iter()
                .filter_map(|p| match p {
                    syn::GenericArgument::Type(t) => Some(t),
                    syn::GenericArgument::Binding(t) => Some(&t.ty),
                    syn::GenericArgument::Constraint(..) => None,
                    syn::GenericArgument::Const(..) => None,
                    _ => None, // lifetimes, expr, constraints A : B ... skip!
                })
                .cloned()
                .collect::<Vec<_>>();

            Some(FSType {
                ident,
                path: fullpath,
                args,
                return_type: None,
            })
        }
        None => None,
    }
}

pub(crate) struct FieldContext<'a> {
    pub ctxt: &'a ParseContext,    // global parse context
    pub field: &'a ast::Field<'a>, // field being parsed
    pub attrs: Attrs,              // field attributes
}

impl<'a> FieldContext<'a> {
    pub fn get_path(&self, ty: &syn::Type) -> Option<FSType> {
        use syn::Type::Path;
        use syn::TypePath;
        match ty {
            Path(TypePath { path, .. }) => last_path_element(&path),
            _ => None,
        }
    }
}

pub(crate) struct ParseContext {
    ctxt: Option<Ctxt>,  // serde parse context for error reporting
    global_attrs: Attrs, // global #[fs(...)] attributes
    ident: syn::Ident,   // name of enum struct
}

impl Drop for ParseContext {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            // must track this in case of errors so we can check them
            // if we don't consume the errors, we'll get an "unhandled errors" panic whether or not there were errors
            if let Some(ctxt) = self.ctxt.take() {
                ctxt.check().expect("no errors")
            }
        }
    }
}

impl<'a> ParseContext {
    // Some helpers

    fn err_msg<A: quote::ToTokens>(&self, tokens: A, msg: &str) {
        if let Some(ref ctxt) = self.ctxt {
            ctxt.error_spanned_by(tokens, msg);
        } else {
            panic!(msg.to_string())
        }
    }

    /// returns { #ty } of
    fn field_to_fs(&self, field: &ast::Field<'a>) -> SourceBuilder {
        let attrs = Attrs::from_field(field, self.ctxt.as_ref());
        // if user has provided a type ... use that
        if attrs.fs_type.is_some() {
            // use std::str::FromStr;
            let s = attrs.fs_type.unwrap();
            return SourceBuilder::todo(&format!("fs_type={}", s));
            // match QuoteT::from_str(&s) {
            //     Ok(tokens) => ,
            //     Err(..) => {
            //         self.err_msg(
            //             field.original,
            //             &format!("{}: can't parse type {}", self.ident, s),
            //         );
            //         quote!()
            //     }
            // };
        }

        let fc = FieldContext {
            attrs,
            ctxt: &self,
            field,
        };

        let inner = if let Some(ref ty) = fc.attrs.ts_as {
            fc.type_to_fs(ty)
        } else {
            fc.type_to_fs(&field.ty)
        };

        SourceBuilder::simple(&inner)
    }

    /// returns `#field_name: #ty`
    fn derive_field(&self, field: &ast::Field<'a>) -> SourceBuilder {
        let field_name = field.attrs.name().serialize_name(); // use serde name instead of field.member
        let ty = self.field_to_fs(&field);
        let comment = Attrs::from_field(field, self.ctxt.as_ref()).to_comment_attrs();
        let mut source = SourceBuilder::default();
        for c in comment {
            source.ln_push(&c.tokens.to_string());
        }
        source.ln_push(&field_name);
        source.push(":");
        source.push_source_1(ty);
        source
    }

    fn derive_fields(
        &'a self,
        fields: &'a [&'a ast::Field<'a>],
    ) -> impl Iterator<Item = SourceBuilder> + 'a {
        fields.iter().map(move |f| self.derive_field(f))
    }

    fn derive_field_tuple(
        &'a self,
        fields: &'a [&'a ast::Field<'a>],
    ) -> impl Iterator<Item = SourceBuilder> + 'a {
        fields.iter().map(move |f| self.field_to_fs(f))
    }

    fn check_flatten(&self, fields: &[&'a ast::Field<'a>], ast_container: &ast::Container) -> bool {
        let has_flatten = fields.iter().any(|f| f.attrs.flatten()); // .any(|f| f);
        if has_flatten {
            self.err_msg(
                &self.ident,
                &format!(
                    "{}: #[serde(flatten)] does not work for fsharp-definitions.",
                    ast_container.ident
                ),
            );
        };
        has_flatten
    }
}
