//! The message selector type and utilities.

/// A message selector code.
pub type MessageSelector = u64;

/// Registers a message selector with a name.
/// In debug builds, this will panic if the selector is already registered with a different name
/// and message selectors will be stored in a global registry which can be queried with
/// [`debug::resolve_message_selector`].
pub fn register_message_selector(selector: u64, name: &str) -> u64 {
    #[cfg(feature = "debug")]
    debug::register_message_selector(selector, name);
    selector
}

#[cfg(feature = "debug")]
/// Debug-only utilities for resolving message selectors to names.
pub mod debug {
    extern crate std;

    use alloc::collections::BTreeMap;
    use alloc::string::String;
    use std::sync::{LazyLock, RwLock};

    static MESSAGE_SELECTOR_REGISTRY: LazyLock<RwLock<BTreeMap<u64, String>>> = LazyLock::new(|| {
        RwLock::new(BTreeMap::new())
    });

    pub(crate) fn register_message_selector(selector: u64, name: &str) {
        let mut registry = MESSAGE_SELECTOR_REGISTRY.write().unwrap();
        if let Some(existing)   = registry.get(&selector) {
            if existing != name {
                panic!("message selector conflict: {} is already registered as {} for selector {}", name, existing, selector);
            }
        } else {
            registry.insert(selector, name.into());
        }
    }

    /// Resolves a message selector to a name.
    pub fn resolve_message_selector(selector: u64) -> Option<String> {
        let registry = MESSAGE_SELECTOR_REGISTRY.read().unwrap();
        registry.get(&selector).cloned()
    }
}

