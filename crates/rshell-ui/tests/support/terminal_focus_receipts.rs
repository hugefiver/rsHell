use rshell_core::{
    NewLocalTabCompletion, NewLocalTabIdentity, NewLocalTabReceipt, NewLocalTabReceiptClosed,
    NewLocalTabSubmitError, UiCommand, UiCommandPort, UiPortError,
};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub struct Receipts {
    pending: Mutex<Vec<Option<async_channel::Sender<NewLocalTabCompletion>>>>,
}

#[derive(Default)]
pub struct RecordingPort {
    pub commands: Mutex<Vec<UiCommand>>,
    pub reject: AtomicBool,
    pub receipts: Receipts,
}
impl UiCommandPort for RecordingPort {
    fn try_send(&self, command: UiCommand) -> Result<(), UiPortError> {
        self.commands.lock().unwrap().push(command);
        if self.reject.load(Ordering::Relaxed) {
            Err(UiPortError::Closed)
        } else {
            Ok(())
        }
    }
    fn try_new_local_tab_with_completion(
        &self,
    ) -> Result<NewLocalTabReceipt, NewLocalTabSubmitError> {
        self.commands.lock().unwrap().push(UiCommand::NewLocalTab);
        self.receipts.submit(self.reject.load(Ordering::Relaxed))
    }
}
impl RecordingPort {
    pub fn new_count(&self) -> usize {
        self.commands
            .lock()
            .unwrap()
            .iter()
            .filter(|c| matches!(c, UiCommand::NewLocalTab))
            .count()
    }
}
impl Receipts {
    pub fn submit(&self, reject: bool) -> Result<NewLocalTabReceipt, NewLocalTabSubmitError> {
        if reject {
            return Err(NewLocalTabSubmitError::Rejected(UiPortError::Closed));
        }
        let (sender, receiver) = async_channel::bounded(1);
        self.pending.lock().unwrap().push(Some(sender));
        Ok(Box::pin(async move {
            receiver.recv().await.map_err(|_| NewLocalTabReceiptClosed)
        }))
    }
    pub fn count(&self) -> usize {
        self.pending.lock().unwrap().len()
    }
    pub fn finish(&self, index: usize, completion: NewLocalTabCompletion) {
        if let Some(sender) = self.pending.lock().unwrap()[index].take() {
            let _ = sender.try_send(completion); // Aborted UI receiver must not cancel producer work.
        }
    }
    pub fn created(&self, index: usize, identity: NewLocalTabIdentity) {
        self.finish(index, NewLocalTabCompletion::Created(identity));
    }
    pub fn latest(&self, identity: NewLocalTabIdentity) {
        if let Some(index) = self.count().checked_sub(1) {
            self.created(index, identity);
        }
    }
    pub fn close(&self, index: usize) {
        self.pending.lock().unwrap()[index].take();
    }
    pub fn receiver_closed(&self, index: usize) -> bool {
        self.pending.lock().unwrap()[index]
            .as_ref()
            .is_none_or(|sender| sender.is_closed())
    }
}
