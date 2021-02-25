// Copyright 2019 Ian Castleden
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::{
    filter_visible, ident_from_str, ParseContext, QuoteMaker, QuoteMakerKind, QuoteMakerUnionKind,
};
use crate::source_builder::SourceBuilder;
use serde_derive_internals::{ast, ast::Variant, attr::TagType};
const CONTENT: &str = "fields"; // default content tag
                                // const TAG: &'static str = "kind"; // default tag tag
struct TagInfo<'a> {
    /// #[serde(tag = "...")]
    tag: Option<&'a str>,
    /// #[serde(content = "...")]
    content: Option<&'a str>,
    /// flattened without tag `{ "key1": "", "key2": "" }`
    untagged: bool,
}

impl<'a> TagInfo<'a> {
    fn from_enum(e: &'a TagType) -> Self {
        match e {
            TagType::Internal { tag, .. } => TagInfo {
                tag: Some(tag),
                content: None,
                untagged: false,
            },
            TagType::Adjacent { tag, content, .. } => TagInfo {
                tag: Some(tag),
                content: Some(&content),
                untagged: false,
            },
            TagType::External => TagInfo {
                tag: None,
                content: None,
                untagged: false,
            },
            TagType::None => TagInfo {
                tag: None,
                content: None,
                untagged: true,
            },
        }
    }
}

struct VariantQuoteMaker {
    /// message type possibly including tag key value
    pub source: SourceBuilder,
    /// inner type token stream
    pub inner_type_opt: Option<SourceBuilder>,
}

#[allow(clippy::or_fun_call, clippy::bind_instead_of_map)]
impl<'a> ParseContext {
    pub(crate) fn derive_enum(
        &self,
        variants: &[ast::Variant<'a>],
        ast_container: &ast::Container,
    ) -> QuoteMaker {
        // https://serde.rs/enum-representations.html
        let taginfo = TagInfo::from_enum(ast_container.attrs.tag());
        // remove skipped ( check for #[serde(skip)] )
        let variants: Vec<&ast::Variant<'a>> = variants
            .iter()
            .filter(|v| !v.attrs.skip_serializing())
            .collect();

        // is fsharp enum compatible
        let is_enum = taginfo.tag.is_none()
            && taginfo.content.is_none()
            && variants.iter().all(|v| matches!(v.style, ast::Style::Unit));

        if is_enum {
            let comment_sources = variants
                .iter()
                .map(|variant| crate::attrs::Attrs::from_variant(variant).to_comment_source())
                .collect::<Vec<_>>();
            let v = &variants
                .into_iter()
                .map(|v| v.attrs.name().serialize_name()) // use serde name instead of v.ident
                .collect::<Vec<_>>();

            let k = v.iter().map(|v| ident_from_str(&v)).collect::<Vec<_>>();

            return QuoteMaker {
                extra_top_level_types: None,
                source: {
                    // quote! ( { #(#(#comments)* #k = #v),* } )
                    let mut src = SourceBuilder::default();
                    for ((enum_value, comment_src), enum_variant_name) in
                        comment_sources.into_iter().enumerate().zip(k.into_iter())
                    {
                        src.push_source(comment_src);
                        src.ln_push("| ");
                        src.push(&enum_variant_name.to_string());
                        src.push(" = ");
                        src.push(&format!("{}", enum_value));
                    }

                    src
                },
                kind: QuoteMakerKind::Enum,
            };
        }

        let content: Vec<(&Variant, VariantQuoteMaker)> = variants
            .iter()
            .map(|variant| {
                (
                    *variant,
                    match variant.style {
                        ast::Style::Struct => self.derive_struct_variant(
                            &taginfo,
                            variant,
                            &variant.fields,
                            ast_container,
                        ),
                        ast::Style::Newtype => {
                            self.derive_newtype_variant(&taginfo, variant, &variant.fields[0])
                        }
                        ast::Style::Tuple => {
                            self.derive_tuple_variant(&taginfo, variant, &variant.fields)
                        }
                        ast::Style::Unit => self.derive_unit_variant(&taginfo, variant),
                    },
                )
            })
            .collect::<Vec<_>>();

        let comment_sources = variants
            .iter()
            .map(|variant| crate::attrs::Attrs::from_variant(variant).to_comment_source())
            .collect::<Vec<_>>();

        let mut top_level_types = SourceBuilder::default();

        let mut src = SourceBuilder::default();
        for (
            variant_comment_src,
            (
                variant,
                VariantQuoteMaker {
                    source,
                    inner_type_opt,
                },
            ),
        ) in comment_sources.into_iter().zip(content.into_iter())
        {
            src.ln_note("variant source ✈︎");
            src.push_source(source);

            if let Some(inner_type) = inner_type_opt {
                let mut variant_type_alias = ast_container.ident.to_string();
                variant_type_alias.push_str(&variant.ident.to_string());

                top_level_types.ln_note("variant ☀︎");
                top_level_types.push_source(variant_comment_src);
                top_level_types.ln_push("type ");
                // concat container name with variant name
                top_level_types.push(&variant_type_alias);
                top_level_types.push(" = ");
                top_level_types.push_source_1(inner_type);

                src.push(" of ");
                src.push(&variant_type_alias);
            }
        }

        // derive rust enum that is not an fsharp "enum" (unit enum)
        QuoteMaker {
            extra_top_level_types: Some(top_level_types),
            source: src,
            kind: QuoteMakerKind::Union(match (taginfo.tag, taginfo.content) {
                (Some(tag), Some(content)) => QuoteMakerUnionKind::Tagged {
                    tag: tag.to_string(),
                    content: content.to_string(),
                },
                (None, None) => QuoteMakerUnionKind::Untagged,
                (tag_opt, content_opt) => {
                    panic!(
                        "FSharpDefinitions: While generating for {:?}, we could not mix either tag ({:?}) or content ({:?})",
                        &ast_container.ident.to_string(), tag_opt, content_opt
                    )
                }
            }),
        }
    }

    /// Depends on TagInfo for layout
    fn derive_unit_variant(&self, _taginfo: &TagInfo, variant: &Variant) -> VariantQuoteMaker {
        let variant_name = variant.attrs.name().serialize_name();
        let comment_source = crate::attrs::Attrs::from_variant(variant).to_comment_source();

        return VariantQuoteMaker {
            source: {
                let mut src = SourceBuilder::default();
                src.ln_note("unit variant ☉");
                src.push_source(comment_source);
                src.ln_push("| ");
                src.push(&variant_name);

                src
            },
            inner_type_opt: None,
        };
        // if taginfo.tag.is_none() {
        // }
        // let tag = ident_from_str(taginfo.tag.unwrap());
        // VariantQuoteMaker {
        //     source: quote! (
        //         { #(#comments)* #tag: #variant_name }
        //     ),
        //     inner_type: None,
        // }
    }

    /// Depends on TagInfo for layout
    /// example variant: `C(u32)`
    fn derive_newtype_variant(
        &self,
        taginfo: &TagInfo,
        variant: &Variant,
        field: &ast::Field<'a>,
    ) -> VariantQuoteMaker {
        if field.attrs.skip_serializing() {
            return self.derive_unit_variant(taginfo, variant);
        };
        let comment_source = crate::attrs::Attrs::from_variant(variant).to_comment_source();
        let inner_type = self.field_to_fs(field);
        let variant_name = self.variant_name(variant);

        return VariantQuoteMaker {
            source: {
                let mut src = SourceBuilder::default();
                src.ln_note("newtype variant ☂︎");
                // debug trying to figure out EditDeployExpression
                // src.ln_note(&format!("{:?}", &inner_type));
                src.push_source(comment_source);
                src.ln_push("| ");
                src.push(&variant_name);
                src
            },
            inner_type_opt: Some(inner_type),
        };
    }

    /// Depends on TagInfo for layout
    /// `C { a: u32, b: u32 }` => `C: { a: number, b: number }`
    fn derive_struct_variant(
        &self,
        taginfo: &TagInfo,
        variant: &Variant,
        fields: &[ast::Field<'a>],
        ast_container: &ast::Container,
    ) -> VariantQuoteMaker {
        let fields = filter_visible(fields);
        if fields.is_empty() {
            return self.derive_unit_variant(taginfo, variant);
        }
        self.check_flatten(&fields, ast_container);

        let comment_source = crate::attrs::Attrs::from_variant(variant).to_comment_source();
        let contents = self.derive_fields(&fields).collect::<Vec<_>>();
        let variant_name = self.variant_name(variant);

        let mut inner_type = SourceBuilder::default();
        inner_type.push("{");
        for c in contents {
            inner_type.push_source_1(c);
        }
        inner_type.ln_push("}");

        // let ty_inner = quote!(#(#contents);*);

        VariantQuoteMaker {
            // { #(#comments)* #tag : #ty  }
            source: {
                // quote! ( #( #newls | #body)* )
                let mut src = SourceBuilder::default();
                src.ln_note("struct variant ♛");
                src.push_source(comment_source);
                src.ln_push("| ");
                src.push(&variant_name);

                src
            },
            inner_type_opt: Some(inner_type),
        }
    }

    #[inline]
    fn variant_name(&self, variant: &Variant) -> String {
        variant.attrs.name().serialize_name() // use serde name instead of variant.ident
    }

    /// `B(u32, u32)` => `B: [number, number]`
    fn derive_tuple_variant(
        &self,
        _taginfo: &TagInfo,
        variant: &Variant,
        fields: &[ast::Field<'a>],
    ) -> VariantQuoteMaker {
        let variant_name = self.variant_name(variant);
        let fields = filter_visible(fields);
        let comment_source = crate::attrs::Attrs::from_variant(variant).to_comment_source();
        let contents = self.derive_field_tuple(&fields);
        // let ty = quote!([ #(#contents),* ]);
        let mut ty = SourceBuilder::default();
        let mut first = true;
        for c in contents {
            if first {
                first = false;
            } else {
                ty.push(" * ");
            }
            ty.push_source_1(c);
        }

        return VariantQuoteMaker {
            // source: quote! ({ #(#comments)* #tag : #ty }),
            source: {
                // quote! ( #( #newls | #body)* )
                let mut src = SourceBuilder::default();
                src.ln_note("tuple variant ⚃");
                src.push_source(comment_source);

                src.ln_push("| ");
                src.push(&variant_name);

                src
            },
            inner_type_opt: Some(ty),
        };
    }
}
