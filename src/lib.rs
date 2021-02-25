// Copyright 2019 Ian Castleden
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
#![allow(unused_imports)]

//! # Generate FSharp types from Rust source code.
//!
//! Please see documentation at [crates.io](https://crates.io/crates/fsharp-definitions)
//! or the [README](README/index.html).
//!
// we add this so `cargo doc` shows re-export.

#[macro_use]
pub extern crate fsharp_definitions_derive;

// re-export macros (note pub)
use serde::ser::Serializer;
use std::borrow::Cow;
pub use fsharp_definitions_derive::*;

/// # Trait implemented by `FSharpify` derive macro.
///
/// Please see documentation at [crates.io](https://crates.io/crates/fsharp-definitions)
/// or the [README](README/index.html).
pub trait FSharpifyTrait {
    fn fsharp_ify() -> Cow<'static, str>;

    #[cfg(feature = "type-enum-factories")]
    /// Available with `--features="type-enum-factories"`
    ///
    ///  * [`name`] – is raw string identifier for factory, otherwise defaults to `${NAME}Factory`
    ///
    /// Example ([Internally Tagged](https://serde.rs/enum-representations.html#internally-tagged)):
    ///
    /// Input
    /// ```rust
    /// #[derive(Deserialize, Serialize, FSharpify)]
    /// #[serde(tag = "kind")]
    /// enum Foo { A { value: String }, B { bar: i32 } }
    /// ```
    /// Output
    /// ```fsharp
    /// export const FooFactory = <R>(fn: (message: Foo) => R) => Object.freeze({
    ///     A(content: { value: string }): R { return fn({ kind: "A", ...content }) }
    ///     B(content: { bar: number }): R { return fn({ kind: "B", ...content }) }
    /// })
    /// ```
    fn fsharp_enum_factory() -> Result<Cow<'static, str>, &'static str>;

    #[cfg(feature = "type-enum-handlers")]
    /// Available with `--features="type-enum-handlers"`
    ///
    ///  * [`name`] – is raw string identifier for interface identifier, otherwise defaults to `${NAME}Handler`
    ///  * [`return_type`] – optional raw string type definition for the return type, otherwise defaults to "any"
    ///
    /// Example ([Adjacently Tagged](https://serde.rs/enum-representations.html#adjacently-tagged)):
    ///
    /// Input
    /// ```rust
    /// #[derive(Deserialize, Serialize, FSharpify)]
    /// #[serde(tag = "kind", content = "value"))]
    /// enum Foo { A { value: String }, B { bar: i32 } }
    /// ```
    ///
    /// Output
    /// ```fsharp
    /// export interface FooHandler {
    ///     A(inner: { value: string }): void
    ///     B(inner: { bar: number }): void
    /// }
    /// /** Apply deserialized `Foo` object to the handler `FooHandler` and return the handler's result */
    /// export function applyFoo(to: FooHandler): (outer: Foo) => void { return outer => to[outer["kind"]](outer["value"]) }
    /// ```
    fn fsharp_enum_handlers() -> Result<Cow<'static, str>, &'static str>;
}
/// # String serializer for `u8` byte buffers.
///
/// Use `#[serde(serialize_with="fsharp_definitions::as_byte_string")]`
/// on a `[u8]` or `Vec<u8>` object to  make the output type a `string` (instead of a `number[]`).
/// The encoding is a simple `\xdd` format.
///
/// Or provide your own serializer:
/// `fsharp-definitions` only checks the final *name* "as_byte_string" of the path.
///
pub fn as_byte_string<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // probably not possible to serialze this as a stream
    // we have no access to the underlying io stream... :(
    let t = bytes
        .iter()
        .map(|b| format!(r"\x{:02x}", b))
        .collect::<Vec<_>>()
        .join("");

    serializer.serialize_str(&t)
}
