use rshell_core::UiCommand;

use crate::{MainWindow, PaneAction, PaneHostMsg, SessionTabBarMsg};

impl MainWindow {
    pub(crate) fn send_active_pane_action(&self, action: PaneAction) -> Result<(), &'static str> {
        let Some(tab) = self.view_model.workspace.active_tab else {
            return Err("no_active_tab");
        };
        let pane = self
            .view_model
            .workspace
            .tab(tab)
            .map_err(|_| "no_active_tab")?
            .active_pane;
        self.send_pane(PaneHostMsg::Action { pane, action });
        Ok(())
    }

    pub(crate) fn route_smoke_close_all(&mut self) -> Result<bool, &'static str> {
        let tabs = self
            .view_model
            .workspace
            .tabs
            .iter()
            .map(|tab| tab.id)
            .collect::<Vec<_>>();
        if let Some(tab) = tabs.first().copied() {
            if self.smoke_state.close_all_last_tabs != Some(tabs.len()) {
                self.smoke_state.close_all_last_tabs = Some(tabs.len());
                self.send_tab(SessionTabBarMsg::Close(tab));
            }
            return Ok(false);
        }
        match crate::command_port::dispatch(&self.command_port, UiCommand::Shutdown) {
            Ok(()) => {}
            Err(rshell_core::UiPortError::Busy) => return Ok(false),
            Err(rshell_core::UiPortError::Closed) => return Err("shutdown_command_rejected"),
        }
        if let Some(driver) = &mut self.smoke {
            driver.record_shutdown_sent();
        }
        Ok(true)
    }
}
