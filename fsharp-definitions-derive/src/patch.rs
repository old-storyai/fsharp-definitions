// Copyright 2019 Ian Castleden
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! # Patch
//!
//! we are generating *fsharp* from rust tokens so
//! the final result when rendered to a string has a fsharp
//! formatting problem. This mod just applies a few patches
//! to make the final result a little more acceptable.
//!

use lazy_static::lazy_static;
use proc_macro2::Literal;
use regex::{Captures, Regex};
use std::borrow::Cow;

// In fsharp '===' is a single token whereas
// for rust this would be two tokens '==' and '=',
// and fails to generate correct fsharp/javascript.
// So we subsitute the operator with this identifier and then patch
// it back *after* we generate the string.
// The problem is that someone, somewhere might have
// an identifer that is this... We hope and pray.
//
// This is also the reason we prefer !(x === y) to x !== y ..
// too much patching.

// no field names have anything but ascii at the moment.

const TRIPPLE_EQ: &str = "\"__============__\"";
const NL_PATCH: &str = "\"__nlnlnlnl__\"";
const PURE_PATCH: &str = "\"__pure__\"";
const FS_IGNORE_PATCH: &str = "\"__ts_ignore__\"";
// type N = [(&'static str, &'static str); 10];
const NAMES: [(&str, &str); 16] = [
    ("brack", r"\s*\[\s+\]"),
    ("brace", r"\{\s+\}"),
    ("colon", r"\s+[:]\s"),
    ("enl", r"\n+\}"),
    ("fnl", r"\{\n+"),
    ("te", TRIPPLE_EQ), // for ===
    ("lt", r"\s<\s"),
    ("gt", r"\s>(\s|$)"),
    ("semi", r"\s+;"),
    ("call", r"\s\(\s+\)\s"),
    ("dot", r"\s\.\s"),
    ("nlpatch", NL_PATCH),         // for adding newlines to output string
    ("tsignore", FS_IGNORE_PATCH), // for adding ts-ignore comments to output string
    ("pure", PURE_PATCH),          // for adding ts-ignore comments to output str"doc", ing
    ("doc", r#"#\s*\[\s*doc\s*=\s*"(?P<comment>.*?)"\]"#), // for fixing mishandled ts doc comments
    ("nl", r"\n+"),                // last!
];
lazy_static! {
    static ref RE: Regex = {
        let v = NAMES
            .iter()
            .map(|(n, re)| format!("(?P<{}>{})", n, re))
            .collect::<Vec<_>>()
            .join("|");
        Regex::new(&v).unwrap()
    };
}

trait Has {
    fn has(&self, s: &'static str) -> bool;
    fn key(&self) -> &'static str;
}

impl Has for Captures<'_> {
    #[inline]
    fn has(&self, s: &'static str) -> bool {
        self.name(s).is_some()
    }

    fn key(&self) -> &'static str {
        for n in &NAMES {
            if self.has(n.0) {
                return n.0;
            }
        }
        "?"
    }
    /*
    fn key(&self) -> &'static str {
        for n in RE.capture_names() {
            if let Some(m) = n {
                if self.has(m) {
                    return m;
                }
            }
        };

        "?"
    }
    */
}

// TODO: where does the newline come from? why the double spaces?
// maybe use RegexSet::new(&[.....])
pub fn patch(s: &str) -> Cow<'_, str> {
    RE.replace_all(s, |c: &Captures| {
        let key = c.key();
        let m = match key {
            "brace" => "{}",
            "brack" => "[]",
            "colon" => ": ",
            "fnl" => "{ ",
            // "bar" => "\n  | {",
            "enl" => " }",
            "nl" => " ",
            // "result" => "|",
            "te" => "===",
            "lt" => "<",
            "gt" => ">",
            "semi" => ";",
            "dot" => ".",
            "call" => " () ",
            "nlpatch" => "\n",
            "tsignore" => "//@ts-ignore\n",
            "pure" => "/*#__PURE__*/",
            "doc" => {
                return c.name("comment").map_or(Cow::Borrowed(""), |m| {
                    (String::from("\n    /**") + &unescape(&m.as_str()) + "*/\n").into()
                })
            }
            _ => return Cow::Owned(c.get(0).unwrap().as_str().to_owned()), // maybe should just panic?
        };
        Cow::Borrowed(m)
    })
}

lazy_static! {
    static ref UNESCAPE: Regex = Regex::new(r"\\(.)").unwrap();
}

// when we get the string, e.g. newlines, backslashes and quotes are escaped
fn unescape(input: &str) -> Cow<'_, str> {
    UNESCAPE.replace_all(input, |c: &Captures| {
        Cow::Borrowed(match c.get(1).map_or("", |m| m.as_str()) {
            "n" => "\n",
            "\"" => "\"",
            "\\" => "\\",
            x => return Cow::Owned(x.into()),
        })
    })
}

#[inline]
pub fn nl() -> Literal {
    Literal::string(&NL_PATCH[1..NL_PATCH.len() - 1])
}

// #[inline]
// pub fn pure() -> Literal {
//     Literal::string(&PURE_PATCH[1..PURE_PATCH.len() - 1])
// }

#[inline]
pub fn tsignore() -> Literal {
    Literal::string(&FS_IGNORE_PATCH[1..FS_IGNORE_PATCH.len() - 1])
}

// #[inline]
// pub fn vbar() -> Ident {
//     ident_from_str(RESULT_BAR)
// }
