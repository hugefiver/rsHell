use rshell_core::{ExitStatus, SessionFailure, SessionState};
use secrecy::ExposeSecret;

use crate::{
    SessionCommand, SessionError, SessionEvent, SessionTransport, TransportEvent,
    actor::{ActorControl, SessionActor},
};

impl SessionActor {
    pub(crate) async fn handle_command(
        &mut self,
        transport: &mut dyn SessionTransport,
        command: SessionCommand,
    ) -> ActorControl {
        match command {
            SessionCommand::Input(input) => {
                let bytes = match self.engine.encode_input(input) {
                    Ok(bytes) => bytes,
                    Err(_) => return ActorControl::Failure(SessionFailure::Platform),
                };
                match transport.write(&bytes).await {
                    Ok(()) => ActorControl::Continue,
                    Err(error) => ActorControl::Failure(error.failure()),
                }
            }
            SessionCommand::Mouse(event) => {
                let bytes = match self.engine.encode_mouse(event) {
                    Ok(bytes) => bytes,
                    Err(_) => return ActorControl::Failure(SessionFailure::Platform),
                };
                match transport.write(&bytes).await {
                    Ok(()) => ActorControl::Continue,
                    Err(error) => ActorControl::Failure(error.failure()),
                }
            }
            SessionCommand::Paste(secret) => {
                match transport.write(secret.expose_secret().as_bytes()).await {
                    Ok(()) => ActorControl::Continue,
                    Err(error) => ActorControl::Failure(error.failure()),
                }
            }
            SessionCommand::Resize(size) => {
                if self.engine.resize(size).is_err() {
                    return ActorControl::Failure(SessionFailure::Platform);
                }
                self.viewport.rows = size.rows;
                match transport.resize(size).await {
                    Ok(()) => {
                        self.frame_clock.mark_dirty();
                        ActorControl::Continue
                    }
                    Err(error) => ActorControl::Failure(error.failure()),
                }
            }
            SessionCommand::Scroll(rows) => {
                if self.engine.scroll(rows).is_err() {
                    return ActorControl::Failure(SessionFailure::Platform);
                }
                self.viewport.top_stable_row = self
                    .viewport
                    .top_stable_row
                    .saturating_add(i64::from(rows))
                    .max(0);
                self.frame_clock.mark_dirty();
                ActorControl::Continue
            }
            SessionCommand::Search(query) => match self.engine.search(&query) {
                Ok(matches) => {
                    let _ = self.events.send(SessionEvent::SearchCompleted(matches));
                    ActorControl::Continue
                }
                Err(_) => ActorControl::Failure(SessionFailure::Platform),
            },
            SessionCommand::Select(range) => {
                self.selection = Some(range);
                self.frame_clock.mark_dirty();
                ActorControl::Continue
            }
            SessionCommand::CopySelection => {
                let text = match self.selection {
                    Some(range) => match self.engine.selected_text(range) {
                        Ok(text) => text,
                        Err(_) => return ActorControl::Failure(SessionFailure::Platform),
                    },
                    None => String::new(),
                };
                let _ = self.events.send(SessionEvent::CopyReady(text));
                ActorControl::Continue
            }
            SessionCommand::Respond(id, response) => {
                match self.interactions.respond(id, response) {
                    Ok(()) => ActorControl::Continue,
                    Err(error) => ActorControl::Failure(error.failure()),
                }
            }
            SessionCommand::Reconnect => ActorControl::Reconnect,
            SessionCommand::Shutdown => ActorControl::Shutdown,
        }
    }

    pub(crate) async fn handle_transport_event(
        &mut self,
        transport: &mut dyn SessionTransport,
        result: Result<TransportEvent, crate::TransportError>,
    ) -> ActorControl {
        match result {
            Ok(TransportEvent::Connected) => {
                if self.lifecycle.state() == SessionState::Connected
                    || self.transition(SessionState::Connected)
                {
                    ActorControl::Continue
                } else {
                    ActorControl::IllegalTransition
                }
            }
            Ok(TransportEvent::AwaitingHostKey) => {
                if self.transition(SessionState::AwaitingHostKey) {
                    ActorControl::Continue
                } else {
                    ActorControl::IllegalTransition
                }
            }
            Ok(TransportEvent::AwaitingAuthentication) => {
                if self.transition(SessionState::AwaitingAuthentication) {
                    ActorControl::Continue
                } else {
                    ActorControl::IllegalTransition
                }
            }
            Ok(TransportEvent::Output(bytes)) => {
                let delta = match self.engine.advance(&bytes) {
                    Ok(delta) => delta,
                    Err(_) => return ActorControl::Failure(SessionFailure::Platform),
                };
                if !delta.outbound.is_empty()
                    && let Err(error) = transport.write(&delta.outbound).await
                {
                    return ActorControl::Failure(error.failure());
                }
                if delta.dirty {
                    self.frame_clock.mark_dirty();
                }
                ActorControl::Continue
            }
            Ok(TransportEvent::Eof) => ActorControl::Exit(ExitStatus {
                code: None,
                success: true,
            }),
            Ok(TransportEvent::Exit(status)) => ActorControl::Exit(status),
            Ok(TransportEvent::Failure(error)) | Err(error) => {
                ActorControl::Failure(error.failure())
            }
        }
    }

    pub(crate) fn publish_frame(&mut self) -> Result<(), crate::EngineError> {
        let frame = self.engine.render(self.viewport, self.selection)?;
        let first_frame = self.frames.borrow().is_none();
        self.frames.send_replace(Some(frame.clone()));
        if first_frame {
            let _ = self.events.send(SessionEvent::FrameReady(frame));
        }
        self.frame_clock.published();
        Ok(())
    }

    async fn finish_frame(&mut self) {
        if self.frame_clock.is_dirty() {
            tokio::time::sleep_until(self.frame_clock.deadline()).await;
            let _ = self.publish_frame();
        }
    }

    pub(crate) async fn shutdown(
        &mut self,
        transport: &mut dyn SessionTransport,
    ) -> Result<(), SessionError> {
        if !self.transition(SessionState::Closing) {
            self.lifecycle.illegal(&self.events);
            return Err(SessionError::ActorJoin);
        }
        match transport.shutdown().await {
            Ok(()) => self.lifecycle.exit(
                ExitStatus {
                    code: None,
                    success: true,
                },
                &self.events,
            ),
            Err(error) => {
                self.lifecycle.fail(error.failure(), &self.events);
                self.finish_frame().await;
                return Err(SessionError::TransportShutdown(error.failure()));
            }
        }
        self.finish_frame().await;
        Ok(())
    }

    pub(crate) async fn exit(
        &mut self,
        transport: &mut dyn SessionTransport,
        status: ExitStatus,
    ) -> Result<(), SessionError> {
        match transport.shutdown().await {
            Ok(()) => self.lifecycle.exit(status, &self.events),
            Err(error) => {
                self.lifecycle.fail(error.failure(), &self.events);
                self.finish_frame().await;
                return Err(SessionError::TransportShutdown(error.failure()));
            }
        }
        self.finish_frame().await;
        Ok(())
    }

    pub(crate) async fn fail(
        &mut self,
        transport: &mut dyn SessionTransport,
        failure: SessionFailure,
    ) -> Result<(), SessionError> {
        self.lifecycle.fail(failure, &self.events);
        let result = transport
            .shutdown()
            .await
            .map_err(|error| SessionError::TransportShutdown(error.failure()));
        self.finish_frame().await;
        result
    }

    pub(crate) async fn illegal(
        &mut self,
        transport: &mut dyn SessionTransport,
    ) -> Result<(), SessionError> {
        self.lifecycle.illegal(&self.events);
        let result = transport
            .shutdown()
            .await
            .map_err(|error| SessionError::TransportShutdown(error.failure()));
        self.finish_frame().await;
        result
    }
}
