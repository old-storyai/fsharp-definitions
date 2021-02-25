// Copyright 2019 Ian Castleden
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use serde_derive_internals::ast;

use super::{filter_visible, ParseContext, QuoteMaker, QuoteMakerKind};

use crate::SourceBuilder;

const DEFAULT_ERROR: Result<SourceBuilder, &'static str> =
    Err("struct cannot have a handler or factory");

impl<'a> ParseContext {
    pub(crate) fn derive_struct(
        &self,
        style: ast::Style,
        fields: &[ast::Field<'a>],
        container: &ast::Container,
    ) -> QuoteMaker {
        match style {
            ast::Style::Struct => self.derive_struct_named_fields(fields, container),
            ast::Style::Newtype => self.derive_struct_newtype(&fields[0], container),
            ast::Style::Tuple => self.derive_struct_tuple(fields, container),
            ast::Style::Unit => self.derive_struct_unit(),
        }
    }

    fn derive_struct_newtype(
        &self,
        field: &ast::Field<'a>,
        ast_container: &ast::Container,
    ) -> QuoteMaker {
        if field.attrs.skip_serializing() {
            return self.derive_struct_unit();
        }
        self.check_flatten(&[field], ast_container);

        QuoteMaker {
            source: self.field_to_fs(field),
            kind: QuoteMakerKind::Object,
        }
    }

    fn derive_struct_unit(&self) -> QuoteMaker {
        QuoteMaker {
            source: SourceBuilder::todo("derive_struct_unit"),
            kind: QuoteMakerKind::Object,
        }
    }

    fn derive_struct_named_fields(
        &self,
        fields: &[ast::Field<'a>],
        ast_container: &ast::Container,
    ) -> QuoteMaker {
        let fields = filter_visible(fields);
        if fields.is_empty() {
            return self.derive_struct_unit();
        };

        if fields.len() == 1 && ast_container.attrs.transparent() {
            return self.derive_struct_newtype(&fields[0], ast_container);
        };
        self.check_flatten(&fields, ast_container);
        let content = self.derive_fields(&fields);

        let mut source = SourceBuilder::default();
        source.push("{ ");
        for c in content {
            source.push_source_1(c);
            source.push(";"); // for safety
        }
        source.push(" }");

        QuoteMaker {
            // source: quote!({ #(#content);* }),
            source,
            kind: QuoteMakerKind::Object,
        }
    }

    fn derive_struct_tuple(
        &self,
        fields: &[ast::Field<'a>],
        ast_container: &ast::Container,
    ) -> QuoteMaker {
        let fields = filter_visible(fields);
        if fields.is_empty() {
            return self.derive_struct_unit();
        }

        if fields.len() == 1 && ast_container.attrs.transparent() {
            return self.derive_struct_newtype(&fields[0], ast_container);
        };
        self.check_flatten(&fields, ast_container);
        let content = self.derive_field_tuple(&fields);

        let mut source = SourceBuilder::default();
        let mut first = true;
        for c in content {
            if first {
                first = false;
            } else {
                source.push(" *"); // tuple separator
            }
            source.push_source_1(c);
        }
        QuoteMaker {
            // source: quote!([#(#content),*]),
            source,
            kind: QuoteMakerKind::Object,
        }
    }
}
