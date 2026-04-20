//! Tri-state deserialization helpers for PATCH semantics.
//!
//! JSON PATCH bodies need to distinguish three input states for an optional
//! field `f` of type `T`:
//!
//! | JSON input        | Rust representation       | Meaning                |
//! |-------------------|---------------------------|------------------------|
//! | field omitted     | `None`                    | leave existing value   |
//! | `"f": null`       | `Some(None)`              | clear to NULL          |
//! | `"f": <value>`    | `Some(Some(value))`       | set to `value`         |
//!
//! Plain `Option<T>` cannot represent "explicit null vs. absent" — serde
//! collapses both inputs to `None`. This module exposes `double_option` for
//! use with `#[serde(default, deserialize_with = "...")]` on
//! `Option<Option<T>>` fields so PATCH handlers can honor true clear semantics.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Deserialize a potentially-null, potentially-absent field into
/// `Option<Option<T>>`. When combined with `#[serde(default)]` on the field,
/// the three JSON states map as documented in the module-level table.
pub fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

/// Mirror serializer so round-tripping through a client library preserves
/// the tri-state distinction where possible. `Some(None)` → `null`,
/// `None` is emitted by `skip_serializing_if` upstream.
pub fn serialize_double_option<T, S>(
    value: &Option<Option<T>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    T: Serialize,
    S: Serializer,
{
    match value {
        Some(Some(v)) => serializer.serialize_some(v),
        Some(None) => serializer.serialize_none(),
        None => serializer.serialize_none(),
    }
}
