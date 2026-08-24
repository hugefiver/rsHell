use std::sync::Arc;

use rshell_core::{UiCommand, UiCommandPort, UiPortError};

pub(crate) fn dispatch(
    port: &Arc<dyn UiCommandPort>,
    command: UiCommand,
) -> Result<(), UiPortError> {
    port.try_send(command)
}
