use crate::{
    ConnectionEditorMsg, ConnectionSidebarMsg, ImportDialogMsg, InteractionAction,
    InteractionDialogMsg, MainWindow, PaneAction, PaneHostMsg, SessionTabBarMsg, SmokeAction,
    SmokeConnectionField, TerminalViewMsg,
    main_window_smoke_input::{smoke_terminal_messages, split_smoke_terminal_submission},
};
use gtk::gdk;

impl MainWindow {
    pub(crate) fn route_smoke_action(&mut self, action: SmokeAction) -> Result<bool, &'static str> {
        match action {
            SmokeAction::NewTab => self.send_tab(SessionTabBarMsg::NewLocalTab),
            SmokeAction::OpenConnectionEditor => {
                self.send_sidebar(ConnectionSidebarMsg::CreateConnection)
            }
            SmokeAction::SetConnectionField(field) => self.route_smoke_field(field)?,
            SmokeAction::SubmitConnection => self.send_editor(ConnectionEditorMsg::Save),
            SmokeAction::SelectConnection(name) => {
                let mut matches = self
                    .view_model
                    .catalog
                    .connections
                    .iter()
                    .filter(|(_, profile)| profile.name == name)
                    .map(|(id, _)| *id);
                let connection = matches.next().ok_or("connection_name_not_found")?;
                if matches.next().is_some() {
                    return Err("connection_name_ambiguous");
                }
                let Some(driver) = &mut self.smoke else {
                    return Err("smoke_driver_unavailable");
                };
                driver.record_selection_target(connection);
                self.send_sidebar(ConnectionSidebarMsg::SelectConnection(connection));
            }
            SmokeAction::Connect => {
                let Some(connection) = self
                    .smoke
                    .as_ref()
                    .and_then(|driver| driver.selected_connection())
                else {
                    return Err("connection_not_selected");
                };
                self.send_pane(PaneHostMsg::Connect { connection });
            }
            SmokeAction::RespondHostKey { accept } => {
                let interaction = self.smoke_interaction()?;
                if let Some(driver) = &mut self.smoke {
                    driver.record_auth_route(interaction, true);
                }
                self.send_interaction(InteractionDialogMsg::Action(if accept {
                    InteractionAction::AcceptAndStore
                } else {
                    InteractionAction::Reject
                }))
            }
            SmokeAction::RespondAuth { prompt, env_var } => {
                let interaction = self.smoke_auth_interaction(prompt)?;
                let value = std::env::var(env_var).map_err(|_| "missing_secret_environment")?;
                let submit = self.smoke_auth_submission_ready(prompt);
                if let Some(driver) = &mut self.smoke {
                    driver.record_auth_route(interaction, submit);
                }
                self.send_interaction(InteractionDialogMsg::Answer(prompt, value));
                if submit {
                    self.send_interaction(InteractionDialogMsg::Action(InteractionAction::Submit));
                }
            }
            SmokeAction::SendTerminalText {
                text,
                expected_color_marker,
            } => {
                let (text, submit) = split_smoke_terminal_submission(text);
                if let Some(marker) = expected_color_marker {
                    self.prepare_smoke_color(text.clone(), marker);
                }
                for message in smoke_terminal_messages(text, submit) {
                    self.send_terminal(message);
                }
            }
            SmokeAction::PasteTextFromEnv {
                env_var,
                effect_marker,
            } => {
                let text = std::env::var(env_var).map_err(|_| "missing_secret_environment")?;
                self.prepare_smoke_paste(text.clone(), effect_marker)?;
                self.send_terminal(TerminalViewMsg::PasteText(text));
                for message in smoke_terminal_messages(String::new(), true) {
                    self.send_terminal(message);
                }
            }
            SmokeAction::ResizeTerminal {
                width,
                height,
                scale,
            } => {
                self.prepare_smoke_resize(width, height, scale);
                self.send_terminal(TerminalViewMsg::Resize {
                    width,
                    height,
                    scale,
                });
            }
            SmokeAction::SplitHorizontal => {
                self.send_active_pane_action(PaneAction::SplitHorizontal)?
            }
            SmokeAction::SplitVertical => {
                self.send_active_pane_action(PaneAction::SplitVertical)?
            }
            SmokeAction::SwitchTab(index) => {
                let tab = self
                    .view_model
                    .workspace
                    .tabs
                    .get(index)
                    .map(|tab| tab.id)
                    .ok_or("tab_index_not_found")?;
                self.send_tab(SessionTabBarMsg::Activate(tab));
            }
            SmokeAction::SearchTerminal {
                text,
                case_sensitive,
                regex,
            } => self.send_terminal(TerminalViewMsg::Search {
                text,
                case_sensitive,
                regex,
            }),
            SmokeAction::SelectRange {
                start_x,
                start_y,
                end_x,
                end_y,
                rectangular,
                expected_text,
                expect_wide_midpoint,
            } => {
                self.prepare_smoke_selection(
                    expected_text,
                    expect_wide_midpoint,
                    [
                        start_x.to_bits(),
                        start_y.to_bits(),
                        end_x.to_bits(),
                        end_y.to_bits(),
                    ],
                );
                self.send_terminal(TerminalViewMsg::Selection {
                    start_x,
                    start_y,
                    end_x,
                    end_y,
                    rectangular,
                });
            }
            SmokeAction::CopySelection => self.send_terminal(TerminalViewMsg::Copy),
            SmokeAction::Reconnect => {
                self.prepare_smoke_reconnect()?;
                self.send_active_pane_action(PaneAction::Reconnect)?;
            }
            SmokeAction::VisualCheckpoint(checkpoint) => {
                return self.route_visual_checkpoint(checkpoint);
            }
            SmokeAction::PreviewImport {
                source,
                path,
                expected,
            } => {
                self.prepare_smoke_import(source, expected);
                self.send_import(ImportDialogMsg::PreviewPath(source, path))
            }
            SmokeAction::CommitImport => self.send_import(ImportDialogMsg::Commit),
            SmokeAction::CancelImport => self.send_import(ImportDialogMsg::Close),
            SmokeAction::CloseAll => return self.route_smoke_close_all(),
            SmokeAction::InterruptTerminal => self.send_terminal(TerminalViewMsg::Key {
                key: gdk::Key::c,
                state: gdk::ModifierType::CONTROL_MASK,
            }),
            SmokeAction::ResetDisplay => self.send_active_pane_action(PaneAction::ResetDisplay)?,
            SmokeAction::ResizeWindow {
                width,
                height,
                expected_mode,
            } => self.route_smoke_window_resize(width, height, expected_mode)?,
            SmokeAction::WaitWindowRealized | SmokeAction::WaitFrameContains(_) => {
                return Err("wait_action_routed");
            }
        }
        Ok(true)
    }

    fn route_smoke_field(&self, field: SmokeConnectionField) -> Result<(), &'static str> {
        let message = match field {
            SmokeConnectionField::Text { field, value } => {
                ConnectionEditorMsg::TextChanged(field, value)
            }
            SmokeConnectionField::Port(port) => ConnectionEditorMsg::PortChanged(port),
            SmokeConnectionField::Transport(transport) => {
                ConnectionEditorMsg::TransportChanged(match transport {
                    rshell_core::TransportKind::SystemOpenSsh => 0,
                    rshell_core::TransportKind::NativeSsh => 1,
                })
            }
            SmokeConnectionField::Authentication(authentication) => {
                ConnectionEditorMsg::AuthenticationChanged(authentication)
            }
            SmokeConnectionField::SecretFromEnv { env_var } => ConnectionEditorMsg::SecretChanged(
                std::env::var(env_var).map_err(|_| "missing_secret_environment")?,
            ),
        };
        self.send_editor(message);
        Ok(())
    }

    fn smoke_auth_interaction(
        &self,
        prompt: usize,
    ) -> Result<rshell_core::InteractionId, &'static str> {
        let interaction = self.smoke_interaction()?;
        if prompt >= self.smoke_state.interaction_prompt_count {
            return Err("auth_prompt_out_of_range");
        }
        Ok(interaction)
    }

    fn smoke_interaction(&self) -> Result<rshell_core::InteractionId, &'static str> {
        let interaction = self.smoke_state.interaction.ok_or("interaction_not_open")?;
        if self.smoke_state.interaction_pending {
            return Err("interaction_pending");
        }
        Ok(interaction)
    }

    fn smoke_auth_submission_ready(&self, prompt: usize) -> bool {
        (0..self.smoke_state.interaction_prompt_count).all(|index| {
            index == prompt
                || self
                    .smoke_state
                    .interaction_answered_prompts
                    .contains(&index)
        })
    }

    fn send_terminal(&self, message: TerminalViewMsg) {
        self.send_pane(PaneHostMsg::ActiveTerminal(message));
    }
}
