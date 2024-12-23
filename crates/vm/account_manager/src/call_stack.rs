use arrayvec::ArrayVec;
use core::cell::RefCell;
use ixc_message_api::code::ErrorCode;
use ixc_message_api::AccountID;

#[derive(Debug)]
pub(crate) struct CallStack<const CALL_STACK_LIMIT: usize> {
    call_stack: RefCell<ArrayVec<Frame, CALL_STACK_LIMIT>>,
}

#[derive(Debug)]
pub(crate) struct Frame {
    active_account: AccountID,
}

impl<const CALL_STACK_LIMIT: usize> CallStack<CALL_STACK_LIMIT> {
    pub(crate) fn new(active_account: AccountID,) -> Self {
        let res = Self {
            call_stack: RefCell::new(Default::default()),
        };
        res.push(active_account).unwrap(); // TODO
        res
    }

    pub(crate) fn push(
        &self,
        account_id: AccountID,
    ) -> Result<(), ErrorCode> {
        let frame = Frame {
            active_account: account_id,
        };
        self.call_stack.borrow_mut().try_push(frame).map_err(|_| {
            ErrorCode::SystemCode(ixc_message_api::code::SystemCode::CallStackOverflow)
        })
    }

    /// Pops the top frame from the call stack and returns the gas consumed
    /// since the frame was pushed.
    pub(crate) fn pop(&self) {
        self.call_stack.borrow_mut().pop();
    }

    pub(crate) fn caller(&self) -> Result<AccountID, ErrorCode> {
        let call_stack = self.call_stack.borrow();
        let len = call_stack.len();
        call_stack
            .get(len - 2)
            .map(|f| f.active_account)
            .ok_or(ErrorCode::SystemCode(
                ixc_message_api::code::SystemCode::FatalExecutionError,
            ))
    }

    pub(crate) fn active_account(&self) -> Result<AccountID, ErrorCode> {
        self.call_stack
            .borrow()
            .last()
            .map(|f| f.active_account)
            .ok_or(ErrorCode::SystemCode(
                ixc_message_api::code::SystemCode::FatalExecutionError,
            ))
    }
}
