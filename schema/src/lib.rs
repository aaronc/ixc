#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]
#![no_std]

#[cfg(feature = "std")]
extern crate alloc;

// this is to allow this crate to use its own macros
#[cfg(not(feature = "use_ixc_macro_path"))]
extern crate self as ixc_schema;
#[cfg(feature = "use_ixc_macro_path")]
extern crate self as ixc;

pub mod value;
pub mod types;
pub mod structs;
mod enums;
mod oneof;
pub mod state_object;
pub mod codec;
pub mod decoder;
pub mod list;
pub mod binary;
pub mod encoder;
pub mod kind;
pub mod field;
mod fields;
pub mod buffer;
pub mod mem;
mod bump;
pub mod schema;
mod message;
mod json;

pub use value::SchemaValue;
pub use state_object::{Str, Bytes};