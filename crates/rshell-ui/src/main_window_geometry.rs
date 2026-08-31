use rshell_core::{SessionUiCommand, UiCommand};

use crate::StartupProbe;

pub(crate) fn observe(probe: Option<&StartupProbe>, command: &UiCommand) {
    if let (
        Some(probe),
        UiCommand::Session {
            command: SessionUiCommand::Resize(size),
            ..
        },
    ) = (probe, command)
    {
        probe.observe_terminal_geometry(*size);
    }
}

#[cfg(test)]
mod tests {
    use rshell_core::{SessionId, TerminalSize};

    use super::*;

    #[test]
    fn root_dispatch_observes_only_real_positive_terminal_resize_commands() {
        let probe = StartupProbe::new();
        let command = UiCommand::Session {
            session: SessionId::new(),
            command: SessionUiCommand::Resize(TerminalSize {
                cols: 80,
                rows: 24,
                pixel_width: 880,
                pixel_height: 480,
                dpi: 96,
            }),
        };

        observe(Some(&probe), &command);
        assert!(probe.report(false).measured_terminal_geometry_ready);
    }
}
