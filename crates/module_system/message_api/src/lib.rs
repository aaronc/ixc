//! **WARNING: This is an API preview! Expect major bugs, glaring omissions, and breaking changes!**
//!
//! This crate provides a low-level implementation of the Cosmos SDK RFC 003 message passing API.
#![no_std]
extern crate alloc;

mod account_id;
pub mod code;
pub mod data_pointer;
pub mod handler;
pub mod header;
pub mod packet;
pub mod message_selector;
pub use account_id::AccountID;
