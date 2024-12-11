use crate::state_handler::std::manager::StdStateManager;
use crate::state_handler::{GasMeter, StateHandler};
use allocator_api2::alloc::Allocator;
use ixc_core_macros::message_selector;
use ixc_message_api::code::ErrorCode;
use ixc_message_api::code::SystemCode::{FatalExecutionError, MessageNotHandled};
use ixc_message_api::packet::MessagePacket;
use ixc_message_api::AccountID;
use core::alloc::Layout;
use ixc_message_api::code::ErrorCode::SystemCode;
use ixc_message_api::message_selector::MessageSelector;

/// The standard state handler.
pub struct StdStateHandler<'a, S: StdStateManager> {
    state: &'a mut S,
    gas_config: GasConfig,
}

/// Gas configuration for the standard state handler.
#[derive(Default)]
pub struct GasConfig {
    /// The cost of deleting a value from storage.
    pub delete_cost: u64,
    /// The flat cost of reading a value from storage.
    pub read_cost_flat: u64,
    /// The cost per byte of reading a value from storage.
    pub read_cost_per_byte: u64,
    /// The flat cost of writing a value to storage.
    pub write_cost_flat: u64,
    /// The cost per byte of writing a value to storage.
    pub write_cost_per_byte: u64,
}

impl<'a, S: StdStateManager> StdStateHandler<'a, S> {
    /// Create a new standard state handler.
    pub fn new(state: &'a mut S, gas_config: GasConfig) -> Self {
        Self { state, gas_config }
    }
}

impl<'a, S: StdStateManager> StateHandler for StdStateHandler<'a, S> {
    fn kv_get<A: Allocator>(
        &self,
        account_id: AccountID,
        key: &[u8],
        gas: &mut GasMeter,
        allocator: A,
    ) -> Result<Option<allocator_api2::vec::Vec<u8, A>>, ErrorCode> {
        self.state
            .kv_get(account_id, None, key, allocator)
            .map_err(|_| ErrorCode::SystemCode(FatalExecutionError))
    }

    fn kv_set(
        &mut self,
        account_id: AccountID,
        key: &[u8],
        value: &[u8],
        gas: &mut GasMeter,
    ) -> Result<(), ErrorCode> {
        self.state
            .kv_set(account_id, None, key, value)
            .map_err(|_| ErrorCode::SystemCode(FatalExecutionError))
    }

    fn kv_delete(
        &mut self,
        account_id: AccountID,
        key: &[u8],
        gas: &mut GasMeter,
    ) -> Result<(), ErrorCode> {
        self.state
            .kv_delete(account_id, None, key)
            .map_err(|_| ErrorCode::SystemCode(FatalExecutionError))
    }

    fn begin_tx(&mut self, gas: &mut GasMeter) -> Result<(), ErrorCode> {
        self.state
            .begin_tx()
            .map_err(|_| ErrorCode::SystemCode(FatalExecutionError))
    }

    fn commit_tx(&mut self, gas: &mut GasMeter) -> Result<(), ErrorCode> {
        self.state
            .commit_tx()
            .map_err(|_| ErrorCode::SystemCode(FatalExecutionError))
    }

    fn rollback_tx(&mut self, gas: &mut GasMeter) -> Result<(), ErrorCode> {
        self.state
            .rollback_tx()
            .map_err(|_| ErrorCode::SystemCode(FatalExecutionError))
    }

    fn handle_exec(
        &mut self,
        packet: &mut MessagePacket,
        _allocator: &dyn Allocator,
    ) -> Result<(), ErrorCode> {
        unsafe {
            let header = packet.header();
            match header.message_selector {
                SET_SELECTOR => {
                    let key = header.in_pointer1.get(packet);
                    let value = header.in_pointer2.get(packet);
                    let mut gas = GasMeter::new(header.gas_left);
                    self.kv_set(header.account, key, value, &mut gas)?;
                    packet.header_mut().gas_left = gas.get();
                    Ok(())
                }
                DELETE_SELECTOR => {
                    let key = header.in_pointer1.get(packet);
                    let mut gas = GasMeter::new(header.gas_left);
                    self.kv_delete(header.account, key, &mut gas)?;
                    packet.header_mut().gas_left = gas.get();
                    Ok(())
                }
                _ => Err(ErrorCode::SystemCode(MessageNotHandled)),
            }
        }
    }

    fn handle_query(
        &self,
        packet: &mut MessagePacket,
        allocator: &dyn Allocator,
    ) -> Result<(), ErrorCode> {
        unsafe {
            let header = packet.header();
            match header.message_selector {
                GET_SELECTOR => {
                    let key = header.in_pointer1.get(packet);
                    let mut gas = GasMeter::new(header.gas_left);
                    let value = self.kv_get(header.account, key, &mut gas, allocator)?;
                    packet.header_mut().gas_left = gas.get();
                    if let Some(value) = value {
                        let out = allocator
                            .allocate(Layout::from_size_align_unchecked(value.len(), 16))
                            .map_err(|_| SystemCode(FatalExecutionError))?;
                        let out_slice =
                            core::slice::from_raw_parts_mut(out.as_ptr() as *mut u8, value.len());
                        out_slice.copy_from_slice(value.as_slice());
                        packet.header_mut().out_pointer1.set_slice(out_slice);
                        Ok(())
                    } else {
                        Err(ErrorCode::HandlerCode(0)) // KV-stores should use handler code 0 to indicate not found
                    }
                }
                _ => Err(SystemCode(MessageNotHandled)),
            }
        }
    }

    fn create_account_storage(
        &mut self,
        account: AccountID,
        gas: &mut GasMeter,
    ) -> Result<(), ErrorCode> {
        self.state
            .create_account_storage(account)
            .map_err(|_| ErrorCode::SystemCode(FatalExecutionError))
    }

    fn delete_account_storage(
        &mut self,
        account: AccountID,
        gas: &mut GasMeter,
    ) -> Result<(), ErrorCode> {
        self.state
            .delete_account_storage(account)
            .map_err(|_| ErrorCode::SystemCode(FatalExecutionError))
    }
}

const GET_SELECTOR: MessageSelector = message_selector!("ixc.store.v1.get");
const SET_SELECTOR: MessageSelector = message_selector!("ixc.store.v1.set");
const DELETE_SELECTOR: MessageSelector = message_selector!("ixc.store.v1.delete");
