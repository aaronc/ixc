//! State object traits.

mod field_types;
mod key;
mod key_field;
mod prefix;
mod value;
mod value_field;

use alloc::vec::Vec;
use ixc_schema_macros::SchemaValue;
use crate::field::Field;
pub use key::{decode_object_key, encode_object_key, ObjectKey};
pub use key_field::KeyFieldValue;
pub use prefix::PrefixKey;
pub use value::{decode_object_value, encode_object_value, ObjectValue};
pub use value_field::{Bytes, ObjectFieldValue, Str};
use crate::encoding::Encoding;

#[cfg(feature = "std")]
/// A type representing objects stored in key-value store state.
#[derive(Debug, Clone, Eq, PartialEq, SchemaValue, Default)]
#[non_exhaustive]
pub struct StateObjectDescriptor<'a> {
    /// The name of the object.
    pub name: &'a str,
    /// The bytes prefix of the state object.
    pub prefix: Vec<u8>,
    /// The encoding of the state object.
    pub encoding: Encoding,
    /// The fields that make up the primary key.
    pub key_fields: Vec<Field<'a>>,
    /// The fields that make up the value.
    pub value_fields: Vec<Field<'a>>,
    /// Whether to retain deletions in off-chain, indexed state.
    pub retain_deletions: bool,
    /// Indicates that the value of the state object behaves like an accumulator.
    /// There must be a single value field of type u128 if this is true.
    pub is_accumulator: bool,
    /// Indicates that the state object is scoped to the account ID
    /// which is the first part of its key.
    /// The first part of the key must be an account ID if this is true.
    pub is_account_scoped: bool,
}
