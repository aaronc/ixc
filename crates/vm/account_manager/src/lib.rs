//! Rust Cosmos SDK RFC 003 hypervisor/state-handler function implementation.
#![no_std]
extern crate alloc;

mod authz;
pub mod id_generator;
pub mod state_handler;
pub mod native_vm;

use crate::authz::AuthorizationMiddleware;
use crate::id_generator::IDGenerator;
use crate::state_handler::gas::GasMeter;
use crate::state_handler::{
    destroy_account_data, get_account_handler_id, init_next_account, StateHandler,
};
use allocator_api2::vec::Vec;
use core::borrow::BorrowMut;
use core::alloc::Layout;
use ixc_core_macros::message_selector;
use ixc_message_api::code::ErrorCode;
use ixc_message_api::code::ErrorCode::SystemCode;
use ixc_message_api::code::SystemCode::{
    AccountNotFound, FatalExecutionError, HandlerNotFound, InvalidHandler, MessageNotHandled,
    UnauthorizedCallerAccess,
};
use ixc_message_api::handler::{Allocator, HostBackend};
use ixc_message_api::packet::MessagePacket;
use ixc_message_api::AccountID;
use ixc_vm_api::{ReadonlyStore, VM};
use core::cell::RefCell;

/// The account manager manages the execution, creation, and destruction of accounts.
pub struct AccountManager<'a, CM: VM> {
    code_manager: &'a CM,
    call_stack_limit: usize,
}

impl<'a, CM: VM> AccountManager<'a, CM> {
    /// Creates a new account manager.
    pub fn new(code_manager: &'a CM) -> Self {
        Self {
            code_manager,
            call_stack_limit: 128,
        }
    }
}

impl<'a, CM: VM> AccountManager<'a, CM> {
    /// Invokes a message packet in the context of the provided state handler.
    pub fn invoke_msg<'b, ST: StateHandler, IDG: IDGenerator, AUTHZ: AuthorizationMiddleware>(
        &'b self,
        state_handler: &'b mut ST,
        id_generator: &'b mut IDG,
        authz: &'b AUTHZ,
        message_packet: &mut MessagePacket,
        allocator: &'b dyn Allocator,
    ) -> Result<(), ErrorCode> {
        let mut call_stack = Vec::new_in(allocator);
        call_stack.push(Frame {
            active_account: message_packet.header().caller,
        });
        let mut exec_context = ExecContext {
            account_manager: self,
            state_handler,
            id_generator,
            authz,
            call_stack,
        };
        exec_context.invoke_msg(message_packet, allocator)
    }

    /// Invokes a message packet in the context of the provided state handler.
    pub fn invoke_query<'b, ST: StateHandler>(
        &self,
        state_handler: &'b ST,
        message_packet: &mut MessagePacket,
        allocator: &dyn Allocator,
    ) -> Result<(), ErrorCode> {
        let mut call_stack = Vec::new_in(allocator);
        call_stack.push(Frame {
            active_account: message_packet.header().caller,
        });
        let query_ctx = QueryContext {
            account_manager: self,
            state_handler,
            call_stack: RefCell::new(call_stack),
        };
        query_ctx.invoke_query(message_packet, allocator)
    }
}

struct ExecContext<'a, CM: VM, ST: StateHandler, IDG: IDGenerator, AUTHZ: AuthorizationMiddleware> {
    account_manager: &'a AccountManager<'a, CM>,
    state_handler: &'a mut ST,
    id_generator: &'a mut IDG,
    authz: &'a AUTHZ,
    call_stack: Vec<Frame, &'a dyn Allocator>,
}

#[derive(Debug)]
struct Frame {
    active_account: AccountID,
}

struct QueryContext<'a, CM: VM, ST: StateHandler> {
    account_manager: &'a AccountManager<'a, CM>,
    state_handler: &'a ST,
    call_stack: RefCell<Vec<Frame, &'a dyn Allocator>>,
}

const ROOT_ACCOUNT: AccountID = AccountID::new(1);
const STATE_ACCOUNT: AccountID = AccountID::new(2);

extern crate std;

/// Invoke a message packet in the context of the provided state handler.
impl<'a, CM: VM, ST: StateHandler, IDG: IDGenerator, AUTHZ: AuthorizationMiddleware> HostBackend
    for ExecContext<'a, CM, ST, IDG, AUTHZ>
{
    fn invoke_msg(
        &mut self,
        message_packet: &mut MessagePacket,
        allocator: &dyn Allocator,
    ) -> Result<(), ErrorCode> {
        let caller = message_packet.header().caller;
        let target_account = message_packet.header().account;
        std::println!("call stack: {:?}, caller: {:?}, target: {:?}, message selector: {:?}", self.call_stack, caller, target_account, message_packet.header().message_selector);
        let active_account = self
            .call_stack
            .last()
            .map(|f| f.active_account)
            .ok_or(SystemCode(FatalExecutionError))?;

        // check if the caller matches the active account
        if caller != active_account {
            if target_account == STATE_ACCOUNT {
                // when calling the state handler, we NEVER allow impersonation
                return Err(SystemCode(UnauthorizedCallerAccess));
            }
            // otherwise we check the authorization middleware to see if impersonation is allowed
            self.authz
                .authorize(message_packet.header().caller, message_packet)?;
        }

        // if the target account is the state account, we can just run the state handler
        if target_account == STATE_ACCOUNT {
            return self.state_handler.handle_exec(message_packet, allocator);
        }

        let mut gas = GasMeter::new(message_packet.header().gas_left);

        // begin a transaction
        self.state_handler
            .begin_tx(&mut gas)
            .map_err(|_| SystemCode(InvalidHandler))?;
        // push the current caller onto the call stack
        self.call_stack.push(Frame { active_account });

        message_packet.header_mut().gas_left = gas.get();

        let res = if target_account == ROOT_ACCOUNT {
            // if the target account is the root account, we can just run the system message
            self.handle_system_message(message_packet, allocator)
        } else {
            // find the account's handler ID
            let handler_id =
                get_account_handler_id(self.state_handler, target_account, &mut gas, allocator)?
                    .ok_or(SystemCode(AccountNotFound))?;

            // run the handler
            let handler = self.account_manager.code_manager.resolve_handler(
                &ReadOnlyStoreWrapper::wrap(self.state_handler, &mut gas),
                &handler_id,
                allocator,
            )?;
            handler.handle_msg(message_packet, self, allocator)
        };

        // commit or rollback the transaction
        if res.is_ok() {
            self.state_handler
                .commit_tx(&mut gas)
                .map_err(|_| SystemCode(InvalidHandler))?;
        } else {
            self.state_handler
                .rollback_tx(&mut gas)
                .map_err(|_| SystemCode(InvalidHandler))?;
        }

        // pop the call stack
        self.call_stack.pop();

        res
    }

    fn invoke_query(
        &self,
        message_packet: &mut MessagePacket,
        allocator: &dyn Allocator,
    ) -> Result<(), ErrorCode> {
        let target_account = message_packet.header().account;
        let mut call_stack = Vec::new_in(allocator);
        call_stack.push(Frame {
            active_account: target_account,
        });
        // create a nested execution frame for the target account
        let query_ctx = QueryContext {
            account_manager: self.account_manager,
            state_handler: self.state_handler,
            call_stack: RefCell::new(call_stack),
        };
        //
        // we never pass the caller to query handlers and any value set in the caller field is ignored
        message_packet.header_mut().caller = AccountID::EMPTY;

        let target_account = message_packet.header().account;
        if target_account == STATE_ACCOUNT {
            return self.state_handler.handle_query(message_packet, allocator);
        }

        let mut gas = GasMeter::new(message_packet.header().gas_left);

        // find the account's handler ID
        let handler_id =
            get_account_handler_id(self.state_handler, target_account, &mut gas, allocator)?
                .ok_or(SystemCode(AccountNotFound))?;

        // run the handler
        let handler = self.account_manager.code_manager.resolve_handler(
            &ReadOnlyStoreWrapper::wrap(self.state_handler, &mut gas),
            &handler_id,
            allocator,
        )?;
        handler.handle_query(message_packet, &query_ctx, allocator)
    }
}

impl<'a, CM: VM, ST: StateHandler> HostBackend for QueryContext<'a, CM, ST> {
    fn invoke_msg(
        &mut self,
        _message_packet: &mut MessagePacket,
        _allocator: &dyn Allocator,
    ) -> Result<(), ErrorCode> {
        Err(SystemCode(
            ixc_message_api::code::SystemCode::VolatileAccessError,
        ))
    }

    fn invoke_query(
        &self,
        message_packet: &mut MessagePacket,
        allocator: &dyn Allocator,
    ) -> Result<(), ErrorCode> {
        let target_account = message_packet.header().account;
        if target_account == STATE_ACCOUNT {
            message_packet.header_mut().caller =
                self.call_stack.borrow().last().unwrap().active_account;
            return self.state_handler.handle_query(message_packet, allocator);
        }

        message_packet.header_mut().caller = AccountID::EMPTY;

        let mut gas = GasMeter::new(message_packet.header().gas_left);

        // find the account's handler ID
        let handler_id =
            get_account_handler_id(self.state_handler, target_account, &mut gas, allocator)?
                .ok_or(SystemCode(AccountNotFound))?;

        // create a nested execution frame for the target account
        let frame = Frame {
            active_account: target_account,
        };

        self.call_stack.borrow_mut().push(frame);

        // run the handler
        let handler = self.account_manager.code_manager.resolve_handler(
            &ReadOnlyStoreWrapper::wrap(self.state_handler, &mut gas),
            &handler_id,
            allocator,
        )?;
        handler.handle_query(message_packet, self, allocator)
    }
}

impl<'a, CM: VM, ST: StateHandler, IDG: IDGenerator, AUTHZ: AuthorizationMiddleware>
    ExecContext<'a, CM, ST, IDG, AUTHZ>
{
    fn handle_system_message(
        &mut self,
        message_packet: &mut MessagePacket,
        allocator: &dyn Allocator,
    ) -> Result<(), ErrorCode> {
        unsafe {
            match message_packet.header().message_selector {
                CREATE_SELECTOR => self.handle_create(message_packet, allocator),
                SELF_DESTRUCT_SELECTOR => self.handle_self_destruct(message_packet),
                _ => Err(SystemCode(MessageNotHandled)),
            }
        }
    }

    unsafe fn handle_create(
        &mut self,
        message_packet: &mut MessagePacket,
        allocator: &dyn Allocator,
    ) -> Result<(), ErrorCode> {
        // get the input data
        let create_header = message_packet.header_mut();
        let handler_id = create_header.in_pointer1.get(message_packet);
        let init_data = create_header.in_pointer2.get(message_packet);

        let mut gas = GasMeter::new(message_packet.header().gas_left);

        // resolve the handler ID and retrieve the VM
        let handler_id = self
            .account_manager
            .code_manager
            .resolve_handler_id(
                &ReadOnlyStoreWrapper::wrap(self.state_handler, &mut gas),
                handler_id,
                allocator,
            )?.ok_or(SystemCode(HandlerNotFound))?;

        // get the next account ID and initialize the account storage
        let id = init_next_account(self.id_generator, self.state_handler, &handler_id, allocator, &mut gas)
            .map_err(|_| SystemCode(InvalidHandler))?;

        // create a packet for calling on_create
        let mut on_create_packet =
            MessagePacket::allocate(allocator, 0).map_err(|_| SystemCode(FatalExecutionError))?;
        let on_create_header = on_create_packet.header_mut();
        on_create_header.account = id;
        on_create_header.caller = create_header.caller;
        on_create_header.message_selector = ON_CREATE_SELECTOR;
        on_create_header.in_pointer1.set_slice(init_data);

        // run the on_create handler
        let handler =  self.account_manager.code_manager.resolve_handler(
            &ReadOnlyStoreWrapper::wrap(self.state_handler, &mut gas),
            &handler_id,
            allocator,
        )?;

        let res = handler.handle_system(
            &mut on_create_packet,
            self,
            allocator,
        );

        let is_ok = match res {
            Ok(_) => true,
            // we accept the case where the handler doesn't have an on_create method
            Err(SystemCode(MessageNotHandled)) => true,
            _ => false,
        };

        if is_ok {
            // the result is ID of the newly created account
            let mut res = allocator
                .allocate(Layout::from_size_align_unchecked(16, 16))
                .map_err(|_| SystemCode(FatalExecutionError))?;
            let id: u128 = id.into();
            res.as_mut().copy_from_slice(&id.to_le_bytes());
            create_header.in_pointer1.set_slice(res.as_ref());
            Ok(())
        } else {
            res
        }
    }

    unsafe fn handle_self_destruct(
        &mut self,
        message_packet: &mut MessagePacket,
    ) -> Result<(), ErrorCode> {
        let mut gas = GasMeter::new(message_packet.header().gas_left);
        destroy_account_data(self.state_handler, message_packet.header().caller, &mut gas)
            .map_err(|_| SystemCode(FatalExecutionError))
    }
}

const CREATE_SELECTOR: u64 = message_selector!("ixc.account.v1.create");
const ON_CREATE_SELECTOR: u64 = message_selector!("ixc.account.v1.on_create");
const SELF_DESTRUCT_SELECTOR: u64 = message_selector!("ixc.account.v1.self_destruct");

struct ReadOnlyStoreWrapper<'a, S: StateHandler> {
    state_handler: &'a S,
    gas: RefCell<&'a mut GasMeter>,
}

impl<'a, S: StateHandler> ReadOnlyStoreWrapper<'a, S> {
    fn wrap(state_handler: &'a S, gas: &'a mut GasMeter) -> Self {
        Self {
            state_handler,
            gas: RefCell::new(gas),
        }
    }
}

impl<'a, S: StateHandler> ReadonlyStore for ReadOnlyStoreWrapper<'a, S> {
    fn get<'b>(
        &self,
        account_id: AccountID,
        key: &[u8],
        allocator: &'b dyn Allocator,
    ) -> Result<Option<Vec<u8, &'b dyn Allocator>>, ErrorCode> {
        let mut gas = self
            .gas
            .try_borrow_mut()
            .map_err(|_| SystemCode(FatalExecutionError))?;
        self.state_handler.kv_get(account_id, key, *gas, allocator)
    }
}
