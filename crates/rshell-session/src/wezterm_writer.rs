use std::{
    io::{self, Write},
    sync::{Arc, Condvar, Mutex},
};

#[derive(Default)]
struct SharedWriterState {
    bytes: Mutex<Vec<u8>>,
    changed: Condvar,
}

#[derive(Clone, Default)]
pub(crate) struct SharedWriter(Arc<SharedWriterState>);

impl SharedWriter {
    pub(crate) fn take(&self) -> Vec<u8> {
        std::mem::take(
            &mut *self
                .0
                .bytes
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
        )
    }

    pub(crate) fn take_through(&self, barrier: &[u8]) -> Vec<u8> {
        let mut bytes = self
            .0
            .bytes
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        while !bytes.ends_with(barrier) {
            bytes = self
                .0
                .changed
                .wait(bytes)
                .unwrap_or_else(|error| error.into_inner());
        }
        let payload_len = bytes.len() - barrier.len();
        bytes.truncate(payload_len);
        std::mem::take(&mut *bytes)
    }
}

impl Write for SharedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .bytes
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .extend_from_slice(bytes);
        self.0.changed.notify_all();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
