#![allow(unused)]
use allocator_api2::alloc::Allocator;
use imbl::{HashMap, OrdMap, Vector};
use ixc_core_macros::message_selector;
use ixc_message_api::packet::MessagePacket;
use ixc_message_api::AccountID;
use std::alloc::Layout;
use std::cell::RefCell;
use thiserror::Error;
use ixc_account_manager::state_handler::{StateHandler};
use ixc_account_manager::state_handler::std::{StdStateError, StdStateManager};
use ixc_message_api::message_selector::MessageSelector;

#[derive(Default, Clone)]
pub struct VersionedMultiStore {
    versions: Vector<MultiStore>,
}

impl VersionedMultiStore {
    pub fn new_transaction(&self) -> Tx {
        let latest = self.versions.last().cloned().unwrap_or_default();
        Tx {
            call_stack: vec![
                Frame {
                    store: latest,
                },
            ],
        }
    }

    pub fn commit(&mut self, tx: Tx) -> Result<(), ()> {
        if tx.call_stack.len() != 1 {
            return Err(());
        }
        let current_frame = tx.current_frame()
            .map_err(|_| ())?;
        self.versions.push_back(current_frame.store.clone());
        Ok(())
    }
}

#[derive(Default, Clone, Debug)]
pub struct MultiStore {
    stores: HashMap<AccountID, Store>,
}

#[derive(Default, Clone, Debug)]
pub struct Store {
    kv_store: OrdMap<std::vec::Vec<u8>, std::vec::Vec<u8>>,
}

pub struct Tx {
    call_stack: std::vec::Vec<Frame>,
}

const GET_SELECTOR: MessageSelector = message_selector!("ixc.store.v1.get");
const SET_SELECTOR: MessageSelector = message_selector!("ixc.store.v1.set");
const DELETE_SELECTOR: MessageSelector = message_selector!("ixc.store.v1.delete");

impl Tx {
    fn current_frame(&self) -> Result<&Frame, StdStateError> {
        self.call_stack.last().ok_or(StdStateError::FatalExecutionError)
    }

    fn current_frame_mut(&mut self) -> Result<&mut Frame, StdStateError> {
        self.call_stack.last_mut().ok_or(StdStateError::FatalExecutionError)
    }
}

impl StdStateManager for Tx {
    fn kv_get<A: Allocator>(&self, account_id: AccountID, scope: Option<AccountID>, key: &[u8], allocator: A) -> Result<Option<allocator_api2::vec::Vec<u8, A>>, StdStateError> {
        if scope.is_some() {
            todo!("scoped kv_get")
        }
        if let Some(store) = self.current_frame()?.store.stores.get(&account_id) {
            if let Some(value) = store.kv_store.get(key) {
                let mut copy = allocator_api2::vec::Vec::new_in(allocator);
                copy.extend_from_slice(value.as_slice());
                Ok(Some(copy))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn kv_set(&mut self, account_id: AccountID, scope: Option<AccountID>, key: &[u8], value: &[u8]) -> Result<(), StdStateError> {
        if scope.is_some() {
            todo!("scoped kv_set")
        }
        let multistore = &mut self.current_frame_mut()?.store;
        if let Some(store) = multistore.stores.get_mut(&account_id) {
            store.kv_store.insert(key.to_vec(), value.to_vec());
        } else {
            let mut store = Store::default();
            store.kv_store.insert(key.to_vec(), value.to_vec());
            multistore.stores.insert(account_id, store);
        }
        Ok(())
    }

    fn kv_delete(&mut self, account_id: AccountID, scope: Option<AccountID>, key: &[u8]) -> Result<(), StdStateError> {
        if scope.is_some() {
            todo!("scoped kv_delete")
        }
        let multistore = &mut self.current_frame_mut()?.store;
        multistore.stores.remove(&account_id);
        Ok(())
    }

    fn accumulator_get(&self, account_id: AccountID, scope: Option<AccountID>, key: &[u8]) -> Result<u128, StdStateError> {
        todo!("accumulator_get")
    }

    fn accumulator_add(&mut self, account_id: AccountID, scope: Option<AccountID>, key: &[u8], value: u128) -> Result<(), StdStateError> {
        todo!("accumulator_add")
    }

    fn accumulator_safe_sub(&mut self, account_id: AccountID, scope: Option<AccountID>, key: &[u8], value: u128) -> Result<bool, StdStateError> {
        todo!("accumulator_safe_sub")
    }

    fn begin_tx(&mut self) -> Result<(), StdStateError> {
        self.call_stack.push(Frame {
            store: self.current_frame()?.store.clone(),
        });
        Ok(())
    }

    fn commit_tx(&mut self) -> Result<(), StdStateError> {
        // when we commit, we pop the current frame and set the store in the next frame to the current frame's store
        let new_multi_store = self.current_frame()?.store.clone();
        self.call_stack.pop();
        let next_frame = self.current_frame_mut()?;
        next_frame.store = new_multi_store;
        Ok(())
    }

    fn rollback_tx(&mut self) -> Result<(), StdStateError> {
        // when we rollback we simply pop the current frame
        self.call_stack.pop();
        Ok(())
    }

    fn create_account_storage(&mut self, account: AccountID) -> Result<(), StdStateError> {
        let mut current_frame = self.current_frame_mut()?;
        current_frame.store.stores.insert(account, Store::default());
        Ok(())
    }

    fn delete_account_storage(&mut self, account: AccountID) -> Result<(), StdStateError> {
        let mut current_frame = self.current_frame_mut()?;
        current_frame.store.stores.remove(&account);
        Ok(())
    }

    fn emit_event(&mut self, sender: AccountID, data: &[u8]) -> Result<(), StdStateError> {
        todo!("emit_event")
    }

    // fn handle(
    //     &self,
    //     message_packet: &mut MessagePacket,
    //     allocator: &dyn Allocator,
    // ) -> Result<(), ErrorCode> {
    //     unsafe {
    //         let header = message_packet.header();
    //         match header.message_selector {
    //             GET_SELECTOR => self.get(message_packet, allocator),
    //             SET_SELECTOR => self.set(message_packet),
    //             DELETE_SELECTOR => self.delete(message_packet),
    //             _ => Err(ErrorCode::SystemCode(InvalidHandler)),
    //         }
    //     }
    // }
}

impl Tx {
    // unsafe fn get(
    //     &self,
    //     packet: &mut MessagePacket,
    //     allocator: &dyn Allocator,
    // ) -> Result<(), ErrorCode> {
    //     let key = packet.header().in_pointer1.get(packet);
    //     self.track_access(key, Access::Read)
    //         .map_err(|_| SystemCode(InvalidHandler))?;
    //     let mut current_frame = self.current_frame.borrow_mut();
    //     let account = current_frame.account;
    //     let current_store = current_frame.get_kv_store(account);
    //     match current_store.kv_store.get(key) {
    //         None => {
    //             return Err(HandlerCode(0)); // KV-stores should use handler code 0 to indicate not found
    //         }
    //         Some(value) => unsafe {
    //             let out = allocator
    //                 .allocate(Layout::from_size_align_unchecked(value.len(), 16))
    //                 .map_err(|_| SystemCode(FatalExecutionError))?;
    //             let out_slice =
    //                 core::slice::from_raw_parts_mut(out.as_ptr() as *mut u8, value.len());
    //             out_slice.copy_from_slice(value.as_slice());
    //             packet.header_mut().out_pointer1.set_slice(out_slice);
    //         },
    //     }
    //     Ok(())
    // }
    //
    // unsafe fn set(&self, packet: &mut MessagePacket) -> Result<(), ErrorCode> {
    //     let key = packet.header().in_pointer1.get(packet);
    //     let value = packet.header().in_pointer2.get(packet);
    //     self.track_access(key, Access::Write)
    //         .map_err(|_| SystemCode(InvalidHandler))?;
    //     let mut current_frame = self.current_frame.borrow_mut();
    //     let account = current_frame.account;
    //     let current_store = current_frame.get_kv_store(account);
    //     current_store.kv_store.insert(key.to_vec(), value.to_vec());
    //     current_frame.changes.push(Update {
    //         account,
    //         key: key.to_vec(),
    //         operation: Operation::Set(value.to_vec()),
    //     });
    //     Ok(())
    // }
    //
    // unsafe fn delete(&self, packet: &mut MessagePacket) -> Result<(), ErrorCode> {
    //     let key = packet.header().in_pointer1.get(packet);
    //     self.track_access(key, Access::Write)
    //         .map_err(|_| SystemCode(InvalidHandler))?;
    //     let mut current_frame = self.current_frame.borrow_mut();
    //     let account = current_frame.account;
    //     let current_store = current_frame.get_kv_store(account);
    //     current_store.kv_store.remove(key);
    //     current_frame.changes.push(Update {
    //         account,
    //         key: key.to_vec(),
    //         operation: Operation::Remove,
    //     });
    //     Ok(())
    // }
    //
    // fn track_access(&self, key: &[u8], access: Access) -> Result<(), AccessError> {
    //     // TODO track reads and writes for parallel execution
    //     Ok(())
    // }
}

#[derive(Debug, Error)]
enum Error {
    #[error("allocation error")]
    AllocError(#[from] allocator_api2::alloc::AllocError),
    #[error("access error")]
    AccessError(#[from] AccessError),
}

#[derive(Debug, Error)]
#[error("access error")]
struct AccessError;

#[derive(Clone)]
pub struct Frame {
    store: MultiStore,
}
