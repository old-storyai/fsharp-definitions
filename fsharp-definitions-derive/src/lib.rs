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
use serde_derive_internals::{ast, Ctxt, Derive};
use syn::DeriveInput;

use source_builder::SourceBuilder;

mod attrs;
mod derive_enum;
mod derive_struct;
mod patch;
mod source_builder;
mod tests;
mod tots;
mod utils;

use attrs::Attrs;
use utils::*;

use patch::patch;

// too many TokenStreams around! give it a different name
type QuoteT = proc_macro2::TokenStream;

struct QuoteMaker {
    pub source: QuoteT,
    pub kind: QuoteMakerKind,
}

enum QuoteMakerKind {
    Object,
    Enum,
    Union,
}

/* #region helpers */

#[allow(unused)]
fn is_wasm32() -> bool {
    use std::env;
    if let Ok(ref v) = env::var("WASM32") {
        return v == "1";
    }
    let mut t = env::args().skip_while(|t| t != "--target").skip(1);
    if let Some(target) = t.next() {
        if target.contains("wasm32") {
            return true;
        }
    };
    false
}

/// derive proc_macro to expose FSharp definitions to `wasm-bindgen`.
///
/// Please see documentation at [crates.io](https://crates.io/crates/fsharp-definitions).
#[proc_macro_derive(FSharpDefinition, attributes(ts))]
pub fn derive_fsharp_definition(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = QuoteT::from(input);
    do_derive_fsharp_definition(input).into()
}

/// derive proc_macro to expose FSharp definitions as a static function.
///
/// Please see documentation at [crates.io](https://crates.io/crates/fsharp-definitions).
#[proc_macro_derive(FSharpify, attributes(ts))]
pub fn derive_fsharp_ify(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = QuoteT::from(input);
    do_derive_fsharp_ify(input).into()
}

fn do_derive_fsharp_definition(input: QuoteT) -> QuoteT {
    let tsy = FSharpify::new(input);
    let parsed = tsy.parse();
    let export_source = parsed.export_type_definition_source();
    let export_string = format!(
        // we're still going to include the values so we can separate them out in an additional step for webpack
        // the alternative to this seems like it might be to fork wasm-bindgen... which I don't want to do.
        "/*StartDefinitionFor__{}__*/\n{}\ntype __StartValuesFor__{}__ = `\n{}\n`/*EndValuesFor__{}__*/\n/*EndDefinitionFor__{}__*/",
        parsed.ident.as_str(),
        export_source.declarations,
        parsed.ident.as_str(),
        export_source.values.replace("`", "\\`"),
        parsed.ident.as_str(),
        parsed.ident.as_str()
    );
    let name = tsy.ident.to_string().to_uppercase();

    let export_ident = ident_from_str(&format!("FS_EXPORT_{}", name));

    let mut q = quote! {
        #[wasm_bindgen(fsharp_custom_section)]
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
        eprintln!("{}", patch(&q.to_string()));
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
    pub fn new(input: QuoteT) -> Self {
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

        // collect and check #[ts(...attrs)]
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
            ident: patch(&container.ident.to_string()).into(),
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
    fn export_type_handler_source(&self) -> Result<String, &'static str> {
        self.q_maker
            .enum_handler
            .as_ref()
            .map(|content| {
                format!(
                    "{}{}",
                    self.pctxt.global_attrs.to_comment_str(),
                    patch(&content.to_string())
                )
            })
            .map_err(|e| *e)
    }

    fn export_type_definition_source(&self) -> SourceBuilder {
        match self.q_maker.kind {
            QuoteMakerKind::Enum => SourceBuilder {
                declarations: format!(
                    "{}export enum {} {}",
                    self.pctxt.global_attrs.to_comment_str(),
                    self.ident,
                    patch(&self.q_maker.source.to_string())
                ),
                values: format!(
                    "{}export enum {} {}",
                    self.pctxt.global_attrs.to_comment_str(),
                    self.ident,
                    patch(&self.q_maker.source.to_string())
                ),
            },
            QuoteMakerKind::Union => SourceBuilder {
                declarations: format!(
                    "{}export type {} = {}",
                    self.pctxt.global_attrs.to_comment_str(),
                    self.ident,
                    patch(&self.q_maker.source.to_string()),
                ),
                values: format!(
                    "{}\n{}",
                    self.export_type_factory_source()
                        .expect("factory exists for union"),
                    self.export_type_handler_source()
                        .expect("handler exists for union"),
                ),
            },
            QuoteMakerKind::Object => SourceBuilder {
                declarations: format!(
                    "{}export type {} = {}",
                    self.pctxt.global_attrs.to_comment_str(),
                    self.ident,
                    patch(&self.q_maker.source.to_string()),
                ),
                values: format!(
                    "{}export const {} = (check: {}) => check\n",
                    // check create function
                    self.pctxt.global_attrs.to_comment_str(),
                    self.ident,
                    self.ident,
                ),
            },
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
    global_attrs: Attrs, // global #[ts(...)] attributes
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
    fn field_to_ts(&self, field: &ast::Field<'a>) -> QuoteT {
        let attrs = Attrs::from_field(field, self.ctxt.as_ref());
        // if user has provided a type ... use that
        if attrs.ts_type.is_some() {
            use std::str::FromStr;
            let s = attrs.ts_type.unwrap();
            return match QuoteT::from_str(&s) {
                Ok(tokens) => tokens,
                Err(..) => {
                    self.err_msg(
                        field.original,
                        &format!("{}: can't parse type {}", self.ident, s),
                    );
                    quote!()
                }
            };
        }

        let fc = FieldContext {
            attrs,
            ctxt: &self,
            field,
        };
        if let Some(ref ty) = fc.attrs.ts_as {
            fc.type_to_ts(ty)
        } else {
            fc.type_to_ts(&field.ty)
        }
    }

    /// returns { #field_name: #ty }
    fn derive_field(&self, field: &ast::Field<'a>) -> QuoteT {
        let field_name = field.attrs.name().serialize_name(); // use serde name instead of field.member
        let field_name = ident_from_str(&field_name);
        let ty = self.field_to_ts(&field);
        let comment = Attrs::from_field(field, self.ctxt.as_ref()).to_comment_attrs();
        quote!(#(#comment)* #field_name: #ty)
    }

    fn derive_fields(
        &'a self,
        fields: &'a [&'a ast::Field<'a>],
    ) -> impl Iterator<Item = QuoteT> + 'a {
        fields.iter().map(move |f| self.derive_field(f))
    }

    fn derive_field_tuple(
        &'a self,
        fields: &'a [&'a ast::Field<'a>],
    ) -> impl Iterator<Item = QuoteT> + 'a {
        fields.iter().map(move |f| self.field_to_ts(f))
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
