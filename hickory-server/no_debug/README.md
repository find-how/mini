# no_debug

[![no_debug](https://github.com/Cypher1/no_debug/actions/workflows/no_debug.yml/badge.svg)](https://github.com/Cypher1/no_debug/actions/workflows/no_debug.yml)
[![license](https://img.shields.io/github/license/Cypher1/no_debug)](./LICENSE)
[![docs.rs](https://img.shields.io/docsrs/no_debug)](https://docs.rs/no_debug/latest/no_debug/)
[![crates.io](https://img.shields.io/crates/v/no_debug)](https://crates.io/crates/no_debug)

## Helper types to omit debug info for select values.

Provides wrapper structs with default `Debug` impls.
This allows you to use the default implementation of `Debug` for large structs while enabling you
to:
- avoid using `Debug` impls that traverse deeply nested or overly large structures,
- avoid using a `Debug` impl that leaks info that should not be logged.

This can improve:
- readability (logs can focus on the information you care about),
- debuggability & security (logs can be more complete without accidentally leaking private
info),
- and performance (complex data structures don't need to be traversed for debugging unless intentionally requested via `Deref`).

Example usage: Hiding a user's password from logs.
```rust
use no_debug::{NoDebug, WithTypeInfo, Ellipses};

#[derive(Debug)]
struct UserInfo {
  username: String,
  password: NoDebug<String>, // Defaults to WithTypeInfo
  posts: NoDebug<Vec<String>, Ellipses>,
}

let user = UserInfo {
    username: "Cypher1".to_string(),
    password: "hunter2".to_string().into(),
    posts: vec![
        "long post 1...".to_string(),
        "long post 2...".to_string(),
        "long post 3...".to_string(),
    ].into()
};

// The password is hidden by default
assert_eq!(
    format!("{:#?}", user),
    r#"UserInfo {
    username: "Cypher1",
    password: <no debug: alloc::string::String>,
    posts: ...,
}"#
);
// And when accessed
assert_eq!(format!("{:?}", user.password), r#"<no debug: alloc::string::String>"#);
// But it can be extracted easily for operating on the data inside, at which point it is
// visible again.
assert_eq!(format!("{:?}", *user.password), r#""hunter2""#);

// The debug output is based on the Msg type.
assert_eq!(format!("{:?}", user.posts), r#"..."#);

// Output can be changed easily with a type conversion
let post_with_type: NoDebug<Vec<String>, WithTypeInfo> = user.posts.take().into();
assert_eq!(format!("{:?}", post_with_type), r#"<no debug: alloc::vec::Vec<alloc::string::String>>"#);
```
