//! The Message trait for invoking messages dynamically.

use ixc_message_api::code::HandlerCode;
use ixc_message_api::message_selector::MessageSelector;
use ixc_schema::codec::Codec;
use ixc_schema::structs::StructSchema;
use ixc_schema::value::{OptionalValue, SchemaValue};

/// The MessageBase trait for invoking messages dynamically.
pub trait MessageBase<'a>: SchemaValue<'a> + StructSchema {
    /// The message selector.
    const SELECTOR: MessageSelector;
    /// The optional response type.
    type Response<'b>: OptionalValue<'b>;
    /// The optional error type.
    type Error: HandlerCode;
    /// The codec to use for encoding and decoding the message.
    type Codec: Codec + Default;
}

/// The Message trait for invoking messages dynamically.
pub trait Message<'a>: MessageBase<'a> {}

/// The QueryMessage trait for invoking query messages dynamically.
pub trait QueryMessage<'a>: MessageBase<'a> {}

/// Extract the response and error types from a Result.
/// Used internally in macros for building the Message implementation and ClientResult type.
pub trait ExtractResponseTypes {
    /// The response type.
    type Response;
    /// The error type.
    type Error;
    /// The client result type.
    type ClientResult;
}

impl<R, E: HandlerCode> ExtractResponseTypes for crate::Result<R, E> {
    type Response = R;
    type Error = E;
    type ClientResult = crate::result::ClientResult<R, E>;
}
