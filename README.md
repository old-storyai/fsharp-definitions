<img alt="repo logo of Ferrix hugging the FSharp logo"
 src="https://github.com/storyscript/fsharp-definitions/blob/master/assets/fsharp-definitions.svg?raw=true"
 width="200"
 height="200"
/>


# fsharp-definitions

> Caution: This is being developed for a specific use case at StoryScript where we need to generate a bunch of TypeScript and FSharp.Json compatible type definitions. We make pretty extensive use of the types, but there are likely some features missing, so please ask if you have a question about using this code!

## Storyscript changelog

### February 2021

 - Fork and change everything from TypeScript to FSharp

### April 2020

 - Significant refactoring in the `fsharp-definitions-derive` crate which enable additional fsharpify trait functions
 - Improved documentation around derive functions
 - Add two additional fsharpify generators for enum factory, enum handlers, and more to facilitate message passing between WASM to JSON and such.

## <a name='Serdeattributes.'></a>Serde attributes.

See Serde [Docs](https://serde.rs/enum-representations.html#internally-tagged).

`fsharp-definitions` tries to adhere to the meaning of serde attributes
like`#[serde(tag="type")]` and `#[serde(tag="tag", content="fields")]`.

Before 0.1.8 we had an implicit default tag of "kind" for enums. Now we don't (although we still have a implicit `transparent` on NewTypes).


Serde attributes understood

* `rename`, `rename_all`:
* `tag`:
* `content`:
* `skip`: (`fsharp-definitions` also skips - by default -  PhantomData fields ... sorry ghost who walks)
* serialize_with="fsharp_definitions::as_byte_string"
* transparent: NewTypes are automatically transparent. Structs with a single field can be marked transparent.

`serialize_with`, if placed on a `[u8]` or `Vec<u8>` field, will take that field to be a string. (And serde_json will output a `\xdd` encoded string of the array. *or* you can create your own... just ensure to name it `as_byte_string`)

```rust
use serde::Serialize;
use fsharp_definitions::{FSharpify, FSharpifyTrait};

#[derive(Serialize, FSharpify)]
struct S {
     #[serde(serialize_with="fsharp_definitions::as_byte_string")]
     #[fs(fs_type="string")]
     image : Vec<u8>,
     buffer: &'static [u8],
}
```

 prints `export type S = { image: string, buffer: number[] };`.

Serde attributes understood but *rejected*:

* `flatten` (this will produce a panic). Probably will never be fixed.

All others are just ignored.

If you have specialized serialization then you
will have to tell `fsharp-definitions`
what the result is ... see the next section.


## <a name='fsharp-definitionattributes'></a>fsharp-definition attributes

There are 2 ways to intervene to correct the
fsharp output.

* `fs_as`: a rust path to another rust type
  that this value serializes like:
* `fs_type`: a *fsharp* type that should be
used.

e.g. some types, for example `chrono::DateTime`, will serializes themselves in an opaque manner. You need to tell `fsharp-definitions`, viz:

```rust
use serde::Serialize;
use fsharp_definitions::{FSharpify, FSharpifyTrait};
// with features=["serde"]
use chrono::{DateTime, Local, Utc};
// with features=["serde-1"]
use arrayvec::ArrayVec;

#[derive(Serialize, FSharpify)]
pub struct Chrono {
    #[fs(fs_type="string")]
    pub local: DateTime<Local>,
    #[fs(fs_as="str")]
    pub utc: DateTime<Utc>,
    #[fs(fs_as="[u8]")]
    pub ip4_addr1 : ArrayVec<[u8; 4]>,
    #[fs(fs_type="number[]")]
    pub ip4_addr2 : ArrayVec<[u8; 4]>
}
```

## <a name='Limitations'></a>Limitations


### <a name='LimitationsofJSON'></a>Limitations of JSON

e.g. Maps with non string keys: This

```rust
use wasm_bindgen::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use fsharp_definitions::FSharpDefinition;
#[derive(Serialize, FSharpDefinition)]
pub struct IntMap {
    pub intmap: HashMap<i32, i32>,
}
```

will generate:

```fsharp

export type IntMap = { intmap: { [key: number]: number } };
```

But the fsharp compiler will type check this:

```fsharp
let v : IntMap = { intmap: {  "6": 6, 4: 4 } };
```

So the generated guard also checks for integer keys with `(+key !== NaN)`.

You can short circuit any field with some attribute
markup 

* `fs_type` specify the serialization.


### <a name='LimitationsofGenerics'></a>Limitations of Generics

`fsharp-definitions` has limited support for verifing generics.

Rust and fsharp diverge a lot on what genericity means. Generic Rust structs don't map well to generic fsharp types. However we don't give up totally.

This will work:

```rust
use wasm_bindgen::prelude::*;
use serde::Serialize;
use fsharp_definitions::FSharpDefinition;

#[derive(Serialize, FSharpDefinition)]
pub struct Value<T> {
    pub value: T,
}

#[derive(Serialize, FSharpDefinition)]
pub struct DependsOnValue {
    pub value: Vec<Value<i32>>,
}
```
Since the monomorphization of `Value` in `DependsOnValue` is one of
`number`, `string` or `boolean`. 

Beyond this you will have to write your own guards e.g.:

```rust
use wasm_bindgen::prelude::*;
use serde::Serialize;
use fsharp_definitions::FSharpDefinition;

#[derive(Serialize, FSharpDefinition)]
pub struct Value<T> {
    pub value: T,
}

#[derive(Serialize, FSharpDefinition)]
pub struct DependsOnValue {
    #[fs(ts_guard="{value: number[]}")]
    pub value: Value<Vec<i32>>,
}
```
*OR* you will have to rewrite the generated guard
for generic type `value: T` yourself. viz:

```fsharp
const isT = <T>(o: any, typename: string): o is T => {
    // typename is the stringified type that we are
    // expecting e.g. `number` or `{a: number, b: string}[]` etc.
    // 
    if (typename !== "number[]") return false;
    if (!Array.isArray(o)) return false;
    for (let v of o) {
        if (typeof v !== "number") return false;
    }
    return true
}
```

Watch out for function name collisions especially if you use simple names such as `T`, for a generic
type name.

The generated output file should really be passed through something like [prettier](https://www.npmjs.com/package/prettier).

## <a name='Examples'></a>Examples

Top level doc (`///` or `//!` ) comments are converted to javascript (line) comments:

```rust
use serde::Serialize;
use fsharp_definitions::{FSharpify, FSharpifyTrait};
#[derive(Serialize, FSharpify)]
/// This is some API Event.
struct Event {
    what : String,
    pos : Vec<(i32,i32)>
}

assert_eq!(Event::fsharp_ify(), "\
// This is some API Event.
export type Event = { what: string; pos: [ number , number ][] };"
)
```

## <a name='Problems'></a>Problems

Oh yes there are problems...

Currently `fsharp-descriptions` will not fail (AFAIK) even for structs and enums with function pointers `fn(a:A, b: B) -> C` (generates fsharp lambda `(a:A, b:B) => C`)
and closures `Fn(A,B) -> C` (generates `(A,B) => C`). These make no sense in the current context (data types, json serialization) so this might be considered a bug.
Watchout!

This might change if use cases show that an error would be better.

If you reference another type in a struct e.g.

```rust
// #[cfg(target_arch="wasm32")]
use wasm_bindgen::prelude::*;
use serde::Serialize;
use fsharp_definitions::{FSharpDefinition};
#[derive(Serialize)]
struct B<T> {q: T}

#[derive(Serialize, FSharpDefinition)]
struct A {
    x : f64,
    b: B<f64>,
}
```

then this will "work" (producing `export type A = { x: number ,b: B<number> })`) but B will be opaque to
fsharp unless B is *also* `#[derive(FSharpDefinition)]`.

Currently there is no check for this omission.

----

The following types are rendered as:

* `Option<T>` => `T | null` (can't use undefined because this will mess with object checking)
* `HashMap<K,V>` => `{ [key:K]:V }` (same for `BTreeMap`)
* `HashSet<V>` => `V[]` (same for `BTreeSet`)
* `&[u8]` and `Vec<u8>` are expected to be byte buffers but are still rendered as `number[]` since
  this is what `serde_json` does. However you can force the output to be a string using
  `#[serde(serialize_with="fsharp_defintions::as_byte_string")]`

An `enum` that is all Unit types such as

```rust
enum Color {
    Red,
    Green,
    Blue
}
```
is rendered as a fsharp enum:

```fsharp
enum Color {
    Red = "Red",
    Green ="Green",
    Blue = "Blue"
}
```

because serde_json will render `Color::Red` as the string `"Red"` instead of `Color.Red` (because JSON).

Serde always seems to render `Result` (in json) as `{"Ok": T } | {"Err": E}` i.e as "External" so we do too.


Formatting is rubbish and won't pass tslint. This is due to the quote! crate taking control of the output token stream. I don't know what it does with whitespace for example... (is whitespace a token in rust?). Anyhow... this crate applies a few band-aid regex patches to pretty things up. But use
[prettier](https://www.npmjs.com/package/prettier).


We are not as clever as serde or the compiler in determining the actual type. For example this won't "work":

```rust
use std::borrow::Cow as Pig;
use fsharp_definitions::{FSharpify,FSharpifyTrait};

#[derive(FSharpify)]
struct S<'a> {
    pig: Pig<'a, str>,
}
println!("{}", S::fsharp_ify());
```

gives `export type S = { pig : Pig<string> }` instead of `export type S = { pig : string }`
Use `#[fs(ts_as="Cow")]` to fix this.

At a certain point `fsharp-definitions` just *assumes* that the token identifier `i32` (say) *is* really the rust signed 32 bit integer and not some crazy renamed struct in your code!

Complex paths are ignored `std::borrow::Cow` and `mycrate::mod::Cow` are the same to us. We're not going to re-implement the compiler to find out if they are *actually* different. A Cow is always "Clone on write".

We can't reasonably obey serde attributes like "flatten" since we would need to find the *actual* Struct object (from somewhere) and query its fields.


## <a name='Credits'></a>Credits

For initial inspiration see http://timryan.org/2019/01/22/exporting-serde-types-to-fsharp.html

Forked from [`wasm-fsharp-definition` by @tcr](https://github.com/tcr/wasm-fsharp-definition?files=1)
which was forked from [`rust-serde-schema` by @srijs](https://github.com/srijs/rust-serde-schema?files=1).

`fsharp_ify` idea from [`fsharpify` by @n3phtys](https://github.com/n3phtys/fsharpify)

Probably some others...

## <a name='License'></a>License

MIT or Apache-2.0, at your option.
