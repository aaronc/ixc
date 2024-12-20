use crate::call_stack::CallStack;
use crate::state_handler::{get_account_handler_id, StateHandler};
use crate::{AccountManager, ReadOnlyStoreWrapper};
use allocator_api2::alloc::Allocator;
use ixc_core_macros::message_selector;
use ixc_message_api::code::ErrorCode;
use ixc_message_api::code::ErrorCode::System;
use ixc_message_api::code::StdCode::MessageNotHandled;
use ixc_message_api::code::SystemCode::{AccountNotFound};
use ixc_message_api::handler::{HostBackend, InvokeParams};
use ixc_message_api::message::{Message, Request, Response};
use ixc_message_api::ROOT_ACCOUNT;
use ixc_vm_api::VM;

pub(crate) struct QueryContext<'b, 'a: 'b, CM: VM, ST: StateHandler, const CALL_STACK_LIMIT: usize>
{
    account_manager: &'a AccountManager<'a, CM, CALL_STACK_LIMIT>,
    state_handler: &'a ST,
    call_stack: &'b CallStack<CALL_STACK_LIMIT>,
}

impl<'b, 'a: 'b, CM: VM, ST: StateHandler, const CALL_STACK_LIMIT: usize>
    QueryContext<'b, 'a, CM, ST, CALL_STACK_LIMIT>
{
    pub(crate) fn new(
        account_manager: &'a AccountManager<'a, CM, CALL_STACK_LIMIT>,
        state_handler: &'a ST,
        call_stack: &'b CallStack<CALL_STACK_LIMIT>,
    ) -> Self {
        Self {
            account_manager,
            state_handler,
            call_stack,
        }
    }
}

impl<'b, 'a: 'b, CM: VM, ST: StateHandler, const CALL_STACK_LIMIT: usize> HostBackend
    for QueryContext<'b, 'a, CM, ST, CALL_STACK_LIMIT>
{
    fn invoke_msg<'c>(
        &mut self,
        _message: &Message,
        _invoke_params: &InvokeParams<'c>,
    ) -> Result<Response<'c>, ErrorCode> {
        Err(System(
            ixc_message_api::code::SystemCode::VolatileAccessError,
        ))
    }

    fn invoke_query<'c>(
        &self,
        message: &Message,
        invoke_params: &InvokeParams<'c>,
    ) -> Result<Response<'c>, ErrorCode> {
        let target_account = message.target_account();
        let allocator = invoke_params.allocator;

        if target_account == ROOT_ACCOUNT {
            return self.handle_system_query(message.request(), allocator);
        }

        let gas = &self.call_stack.gas;

        // find the account's handler ID
        let handler_id =
            get_account_handler_id(self.state_handler, target_account, gas, allocator)?
                .ok_or(System(AccountNotFound))?;

        // create a nested execution frame for the target account
        self.call_stack.push(target_account, None)?;

        // run the handler
        let handler = self.account_manager.code_manager.resolve_handler(
            &ReadOnlyStoreWrapper::wrap(self.state_handler, gas, allocator),
            handler_id,
            allocator,
        )?;

        let res = handler.handle_query(message, self, allocator);

        // pop the call stack
        self.call_stack.pop();

        res
    }

    fn update_state<'c>(
        &mut self,
        _req: &Request,
        _invoke_params: &InvokeParams<'c>,
    ) -> Result<Response<'c>, ErrorCode> {
        Err(System(
            ixc_message_api::code::SystemCode::VolatileAccessError,
        ))
    }

    fn query_state<'c>(
        &self,
        req: &Request,
        invoke_params: &InvokeParams<'c>,
    ) -> Result<Response<'c>, ErrorCode> {
        let active_account = self.call_stack.active_account()?;
        let gas_meter = &self.call_stack.gas;
        self.state_handler
            .handle_query(active_account, req, gas_meter, invoke_params.allocator)
    }

    fn consume_gas(&self, gas: u64) -> Result<(), ErrorCode> {
        self.call_stack.consume_gas(gas)
    }
}

impl<'b, 'a: 'b, CM: VM, ST: StateHandler, const CALL_STACK_LIMIT: usize>
    QueryContext<'b, 'a, CM, ST, CALL_STACK_LIMIT>
{
    fn handle_system_query<'c>(
        &self,
        req: &Request,
        allocator: &'c dyn Allocator,
    ) -> Result<Response<'c>, ErrorCode> {
        unsafe {
            match req.message_selector() {
                GET_HANDLER_ID_SELECTOR => self.handle_get_handler_id(req, allocator),
                _ => Err(MessageNotHandled.into()),
            }
        }
    }

    unsafe fn handle_get_handler_id<'c>(
        &self,
        req: &Request,
        allocator: &'c dyn Allocator,
    ) -> Result<Response<'c>, ErrorCode> {
        // get the account ID from the in pointer
        let account_id = req.in1().expect_account_id()?;

        // look up the handler ID
        let gas = &self.call_stack.gas;
        let handler_id = get_account_handler_id(self.state_handler, account_id, gas, allocator)?
            .ok_or(System(AccountNotFound))?;

        // copy the handler ID to the out pointer

        Ok(Response::new1(handler_id.into()))
    }
}

const GET_HANDLER_ID_SELECTOR: u64 = message_selector!("ixc.account.v1.get_handler_id");
