//! Routing system for message packets.

use allocator_api2::alloc::Allocator;
use ixc_message_api::code::{ErrorCode, SystemCode};
use ixc_message_api::handler::HostBackend;
use ixc_message_api::message_selector::MessageSelector;
use ixc_message_api::packet::MessagePacket;

/// A router for message packets.
pub unsafe trait Router
where
    Self: 'static,
{
    /// The message routes sorted by message selector.
    const SORTED_MSG_ROUTES: &'static [Route<Self>];

    /// The query routes sorted by message selector.
    const SORTED_QUERY_ROUTES: &'static [QueryRoute<Self>];

    /// The system routes sorted by message selector.
    const SORTED_SYSTEM_ROUTES: &'static [Route<Self>];
}

/// A route for a message packet.
pub type Route<T> = (
    u64,
    fn(
        &T,
        &mut MessagePacket,
        callbacks: &mut dyn HostBackend,
        allocator: &dyn Allocator,
    ) -> Result<(), ErrorCode>,
);

/// A route for a message packet.
pub type QueryRoute<T> = (
    u64,
    fn(
        &T,
        &mut MessagePacket,
        callbacks: &dyn HostBackend,
        allocator: &dyn Allocator,
    ) -> Result<(), ErrorCode>,
);

/// Execute a message packet on a router.
pub fn exec_route<R: Router + ?Sized>(
    rtr: &R,
    packet: &mut MessagePacket,
    callbacks: &mut dyn HostBackend,
    allocator: &dyn Allocator,
) -> Result<(), ErrorCode> {
    match find_route(R::SORTED_MSG_ROUTES, packet.header().message_selector) {
        Some(rt) => rt(rtr, packet, callbacks, allocator),
        None => Err(ErrorCode::SystemCode(SystemCode::MessageNotHandled)),
    }
}

/// Execute a query message packet on a router.
pub fn exec_query_route<R: Router + ?Sized>(
    rtr: &R,
    packet: &mut MessagePacket,
    callbacks: &dyn HostBackend,
    allocator: &dyn Allocator,
) -> Result<(), ErrorCode> {
    match find_route(R::SORTED_QUERY_ROUTES, packet.header().message_selector) {
        Some(rt) => rt(rtr, packet, callbacks, allocator),
        None => Err(ErrorCode::SystemCode(SystemCode::MessageNotHandled)),
    }
}

/// Find a route for a message selector.
pub fn find_route<R>(sorted_routes: &[(u64, R)], sel: MessageSelector) -> Option<&R> {
    let res = sorted_routes.binary_search_by_key(&sel, |(selector, _)| *selector);
    match res {
        Ok(idx) => Some(&sorted_routes[idx].1),
        Err(_) => None,
    }
}

/// Sorts the routes by message selector.
pub const fn sort_routes<const N: usize, T: ?Sized>(mut arr: [Route<T>; N]) -> [Route<T>; N] {
    sort_routes_base(arr)
}

/// Sorts the query routes by message selector.
pub const fn sort_query_routes<const N: usize, T: ?Sized>(mut arr: [QueryRoute<T>; N]) -> [QueryRoute<T>; N] {
    sort_routes_base(arr)
}

const fn sort_routes_base<const N: usize, R: ?Sized + Copy>(mut arr: [(u64, R); N]) -> [(u64, R); N] {
    // const bubble sort
    loop {
        let mut swapped = false;
        let mut i = 1;
        while i < N {
            if arr[i - 1].0 > arr[i].0 {
                let left = arr[i - 1];
                let right = arr[i];
                arr[i - 1] = right;
                arr[i] = left;
                swapped = true;
            }
            i += 1;
        }
        if !swapped {
            break;
        }
    }
    arr
}

// Concatenates two arrays of routes.
// pub const fn const_cat<T: ?Sized>(arr: &[Route<T>], arr2: &[Route<T>]) -> &'static [Route<T>] {
//     const N: usize = arr.len() + arr2.len();
//     let mut res: [Route::<T>; N] = [(0, |_, _, _, _| Err(HandlerError::KnownCode(HandlerErrorCode::MessageNotHandled))); arr.len() + arr2.len()];
//     let mut i = 0;
//     let mut j = 0;
//     let mut k = 0;
//     while i < arr.len() {
//         res[k] = arr[i];
//         i += 1;
//         k += 1;
//     }
//     while j < arr2.len() {
//         res[k] = arr2[j];
//         j += 1;
//         k += 1;
//     }
//     res
// }
//
// pub const fn const_map<T: ?Sized, U: ?Sized>(f: fn(&U) -> &T, arr: &'static [Route<T>]) -> &'static [Route<U>] {
//     let mut i = 0;
//     let n = arr.len();
//     let mut res: [Route::<U>; n] = [(0, |_, _, _, _| Err(HandlerError::KnownCode(HandlerErrorCode::MessageNotHandled))); N];
//     while i < n {
//         let route = arr[i];
//         let selector = route.0;
//         let g = route.1;
//         res[i] = (selector, |t, packet, callbacks, allocator| g(f(t), packet, callbacks, allocator));
//         i += 1;
//     }
//     res
// }

// TODO: can use https://docs.rs/array-concat/latest/array_concat/ to concat arrays then the above function to sort
// or https://docs.rs/constcat/latest/constcat/
