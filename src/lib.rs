#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

#[doc(inline)]
pub use ixc_core::{Context, Result, EventBus, Service, create_account, ensure, bail, error};
#[doc(inline)]
pub use ixc_core::resource::Resources;

#[doc(inline)]
pub use ixc_message_api::AccountID;
#[doc(inline)]
pub use ixc_schema::{Str, Bytes};
#[doc(inline)]
pub use state_objects::{Map, Item, Accumulator, AccumulatorMap};
#[doc(inline)]
pub use simple_time::{Time, Duration};

pub use ixc_schema as schema;
pub use ixc_message_api as message_api;
pub use ixc_core as core;

#[allow(unused_imports)]
#[macro_use]
extern crate ixc_core_macros;
#[doc(inline)]
pub use ixc_core_macros::{handler, on_create, publish, handler_api, Resources};

#[allow(unused_imports)]
#[macro_use]
extern crate ixc_schema_macros;
#[doc(inline)]
pub use ixc_schema_macros::*;

#[allow(unused_imports)]
#[macro_use]
extern crate state_objects_macros;
#[doc(inline)]
pub use state_objects_macros::*;
