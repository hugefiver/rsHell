mod support;

use std::{sync::Arc, time::Duration};

use rshell_core::{
    AuthPrompt, CellPosition, ExitStatus, HostKeyDecision, HostKeyPrompt, InteractionId,
    InteractionRequest, InteractionResponse, KeyModifiers, MouseButton, MouseEventKind,
    SelectionRange, SessionFailure, SessionState, TerminalInput, TerminalMouseEvent,
};
use rshell_session::{
    SessionCommand, SessionError, SessionEvent, SessionManager, TransportError, TransportEvent,
};

#[test]
fn copy_ready_debug_redacts_clipboard_text() {
    let event = SessionEvent::CopyReady("COPY-READY-SENSITIVE".into());
    assert!(!format!("{event:?}").contains("COPY-READY-SENSITIVE"));
}
use secrecy::SecretString;
use support::{FakeFactory, NextBehavior, TransportScript, WriteBlocker};
use tokio::sync::broadcast;

const WAIT: Duration = Duration::from_secs(2);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn launch_orders_io_resize_and_clean_shutdown() {
    let (factory, probe) = FakeFactory::new([TransportScript::events([
        TransportEvent::Connected,
        TransportEvent::Output(b"hello\r\n".to_vec()),
    ])]);
    let manager = SessionManager::new(factory);
    let (launch, engine) = support::launch(&probe);
    let mut client = manager.launch(launch).expect("launch");

    let states = collect_through_state(&mut client.events, SessionState::Connected).await;
    assert_eq!(
        states,
        [
            SessionState::Created,
            SessionState::Connecting,
            SessionState::Connected
        ]
    );

    tokio::time::timeout(WAIT, client.frames.changed())
        .await
        .expect("frame timeout")
        .expect("frame channel closed");
    let frame = client.frames.borrow_and_update().clone().expect("frame");
    assert_eq!(frame.rows[0].cells[0].text, "hello");
    assert_eq!(engine.bytes(), b"hello\r\n");

    client
        .try_command(SessionCommand::Select(SelectionRange {
            start: CellPosition {
                stable_row: 0,
                column: 0,
            },
            end: CellPosition {
                stable_row: 0,
                column: 1,
            },
            rectangular: false,
        }))
        .expect("selection accepted");
    tokio::time::timeout(WAIT, client.frames.changed())
        .await
        .expect("selection frame timeout")
        .expect("selection frame channel closed");
    let selected = client.frames.borrow_and_update().clone().expect("frame");
    assert!(selected.rows[0].cells[0].selected);

    manager
        .command(
            client.id,
            SessionCommand::Input(TerminalInput::CommittedText("typed".to_owned())),
        )
        .expect("input accepted");
    client
        .try_command(SessionCommand::Paste(SecretString::from(
            "paste-secret".to_owned(),
        )))
        .expect("paste accepted");
    client
        .try_command(SessionCommand::Resize(support::size()))
        .expect("resize accepted");

    wait_until(|| probe.writes().len() == 2).await;
    wait_until(|| {
        probe
            .log()
            .iter()
            .any(|entry| entry == "transport:resize:1")
    })
    .await;
    assert_eq!(
        probe.writes(),
        vec![(1, b"typed".to_vec()), (1, b"paste-secret".to_vec())]
    );
    let log = probe.log();
    let engine_resize = position(&log, "engine:resize");
    let transport_resize = position(&log, "transport:resize:1");
    assert!(engine_resize < transport_resize, "resize log: {log:?}");

    tokio::time::timeout(WAIT, manager.shutdown_all())
        .await
        .expect("shutdown timeout")
        .expect("shutdown");
    let terminal_states = collect_until_closed(&mut client.events).await;
    assert_eq!(
        terminal_states,
        [SessionState::Closing, SessionState::Exited]
    );
    assert_eq!(count(&probe.log(), "shutdown:1"), 1);
    assert_eq!(
        client.try_command(SessionCommand::Shutdown),
        Err(SessionError::Closed)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transport_shutdown_error_propagates_through_manager_shutdown() {
    let (factory, probe) = FakeFactory::new([TransportScript::events([TransportEvent::Connected])
        .with_shutdown_failure(SessionFailure::Subprocess)]);
    let manager = SessionManager::new(factory);
    let (launch, _engine) = support::launch(&probe);
    let mut client = manager.launch(launch).expect("launch");
    collect_through_state(&mut client.events, SessionState::Connected).await;

    assert_eq!(
        manager.shutdown_all().await,
        Err(SessionError::TransportShutdown(SessionFailure::Subprocess))
    );
    assert_eq!(manager.active_session_count(), 0);
    assert_eq!(manager.active_child_process_count(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mouse_command_is_encoded_by_the_engine_before_transport_write() {
    let (factory, probe) = FakeFactory::new([TransportScript::events([TransportEvent::Connected])]);
    let manager = SessionManager::new(factory);
    let (launch, _engine) = support::launch(&probe);
    let mut client = manager.launch(launch).expect("launch");
    collect_through_state(&mut client.events, SessionState::Connected).await;

    let event = TerminalMouseEvent {
        kind: MouseEventKind::Press,
        button: Some(MouseButton::Left),
        cell: CellPosition {
            stable_row: 101,
            column: 4,
        },
        viewport_row: 1,
        pixel_x: 36,
        pixel_y: 54,
        modifiers: KeyModifiers::default(),
    };
    client
        .try_command(SessionCommand::Mouse(event))
        .expect("mouse accepted");
    wait_until(|| !probe.writes().is_empty()).await;
    assert_eq!(probe.writes(), vec![(1, b"mouse:press:4:101:1".to_vec())]);

    manager.shutdown_all().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bounded_commands_report_backpressure_without_losing_accepted_input() {
    let blocker = WriteBlocker::new();
    let (factory, probe) =
        FakeFactory::new([TransportScript::events([TransportEvent::Connected])
            .with_write_blocker(blocker.clone())]);
    let manager = SessionManager::new(factory);
    let (launch, _engine) = support::launch(&probe);
    let mut client = manager.launch(launch).expect("launch");
    collect_through_state(&mut client.events, SessionState::Connected).await;

    client
        .try_command(SessionCommand::Input(TerminalInput::CommittedText(
            "blocked".to_owned(),
        )))
        .expect("blocking command accepted");
    blocker.wait_started().await;

    for index in 0..128 {
        client
            .try_command(SessionCommand::Input(TerminalInput::CommittedText(
                format!("queued-{index}"),
            )))
            .expect("bounded command accepted");
    }
    assert_eq!(
        manager.command(
            client.id,
            SessionCommand::Input(TerminalInput::CommittedText("overflow".to_owned()))
        ),
        Err(SessionError::Backpressure)
    );

    blocker.release();
    wait_until(|| probe.writes().len() == 129).await;
    let writes = probe.writes();
    assert_eq!(writes[0].1, b"blocked");
    assert_eq!(writes[128].1, b"queued-127");
    assert!(!writes.iter().any(|(_, bytes)| bytes == b"overflow"));

    tokio::time::timeout(WAIT, manager.shutdown_all())
        .await
        .expect("shutdown timeout")
        .expect("shutdown");
}

#[tokio::test(start_paused = true)]
async fn ten_thousand_output_burst_is_latest_only_and_rate_limited() {
    let started = tokio::time::Instant::now();
    let (factory, probe) = FakeFactory::new([TransportScript::burst(10_000)]);
    let manager = SessionManager::new(factory);
    let (launch, engine) = support::launch(&probe);
    let mut client = manager.launch(launch).expect("launch");
    collect_through_state(&mut client.events, SessionState::Connected).await;
    manager
        .command(
            client.id,
            SessionCommand::Input(TerminalInput::CommittedText("priority".to_owned())),
        )
        .expect("command accepted during output burst");

    tokio::time::timeout(WAIT, async {
        loop {
            if client
                .frames
                .borrow_and_update()
                .as_ref()
                .is_some_and(|frame| frame.rows[0].cells[0].text == "line10000")
            {
                break;
            }
            client.frames.changed().await.expect("frame channel closed");
        }
    })
    .await
    .expect("final frame timeout");
    let elapsed = started.elapsed();
    let renders = engine.render_count();
    assert!(renders > 0, "burst produced no frame");
    assert!(
        renders <= 16,
        "burst produced {renders} frames over {elapsed:?}"
    );
    assert!(
        (Duration::from_millis(250)..Duration::from_millis(267)).contains(&elapsed),
        "burst completed outside the deterministic observation window: {elapsed:?}"
    );
    assert_eq!(probe.writes(), vec![(1, b"priority".to_vec())]);
    let mut frame_events = 0;
    loop {
        match tokio::time::timeout(Duration::from_millis(5), client.events.recv()).await {
            Ok(Ok(SessionEvent::FrameReady(_))) => frame_events += 1,
            Ok(Ok(_)) => {}
            Ok(Err(broadcast::error::RecvError::Lagged(skipped))) => {
                panic!("frame updates flooded broadcast by {skipped} events")
            }
            Ok(Err(broadcast::error::RecvError::Closed)) | Err(_) => break,
        }
    }
    assert_eq!(frame_events, 1, "only the first frame is broadcast");

    tokio::time::timeout(WAIT, manager.shutdown_all())
        .await
        .expect("shutdown timeout")
        .expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconnects_are_serial_and_duplicate_shutdown_is_idempotent() {
    let scripts = (0..3).map(|_| TransportScript::events([TransportEvent::Connected]));
    let (factory, probe) = FakeFactory::new(scripts);
    let manager = SessionManager::new(factory);
    let (launch, _engine) = support::launch(&probe);
    let mut client = manager.launch(launch).expect("launch");
    collect_through_state(&mut client.events, SessionState::Connected).await;

    client
        .try_command(SessionCommand::Reconnect)
        .expect("first reconnect");
    client
        .try_command(SessionCommand::Reconnect)
        .expect("second reconnect");
    wait_until(|| probe.log().iter().any(|entry| entry == "connect:3")).await;

    let log = probe.log();
    assert!(position(&log, "shutdown:1") < position(&log, "create:2"));
    assert!(position(&log, "create:2") < position(&log, "connect:2"));
    assert!(position(&log, "connect:2") < position(&log, "shutdown:2"));
    assert!(position(&log, "shutdown:2") < position(&log, "create:3"));
    assert!(position(&log, "create:3") < position(&log, "connect:3"));

    client
        .try_command(SessionCommand::Shutdown)
        .expect("first shutdown");
    let _ = client.try_command(SessionCommand::Shutdown);
    tokio::time::timeout(WAIT, manager.shutdown_all())
        .await
        .expect("shutdown timeout")
        .expect("shutdown");
    manager.shutdown_all().await.expect("repeated shutdown_all");

    let states = collect_until_closed(&mut client.events).await;
    assert_eq!(
        states
            .iter()
            .filter(|state| **state == SessionState::Closing)
            .count(),
        1
    );
    assert_eq!(count(&probe.log(), "shutdown:3"), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn terminal_outcomes_clean_registry_and_manager_remains_usable() {
    let (factory, probe) = FakeFactory::new([
        TransportScript::events([TransportEvent::Eof]),
        TransportScript::events([TransportEvent::Exit(ExitStatus {
            code: Some(17),
            success: false,
        })]),
        TransportScript::events([TransportEvent::Failure(TransportError::new(
            SessionFailure::Network,
        ))]),
        TransportScript::events([TransportEvent::Connected]),
    ]);
    let manager = SessionManager::new(factory);

    let (launch, _) = support::launch(&probe);
    let mut eof = manager.launch(launch).expect("eof launch");
    assert_eq!(
        recv_exit(&mut eof.events).await,
        ExitStatus {
            code: None,
            success: true
        }
    );
    wait_for_event_channel_close(&mut eof.events).await;
    assert_unknown_session(&manager, eof.id);

    let (launch, _) = support::launch(&probe);
    let mut nonzero = manager.launch(launch).expect("exit launch");
    assert_eq!(
        recv_exit(&mut nonzero.events).await,
        ExitStatus {
            code: Some(17),
            success: false
        }
    );
    wait_for_event_channel_close(&mut nonzero.events).await;
    assert_unknown_session(&manager, nonzero.id);

    let (launch, _) = support::launch(&probe);
    let mut failed = manager.launch(launch).expect("failure launch");
    assert_eq!(
        recv_failure(&mut failed.events).await,
        SessionFailure::Network
    );
    wait_for_event_channel_close(&mut failed.events).await;
    assert_unknown_session(&manager, failed.id);

    let (launch, _) = support::launch(&probe);
    let mut healthy = manager.launch(launch).expect("post-cleanup launch");
    collect_through_state(&mut healthy.events, SessionState::Connected).await;

    tokio::time::timeout(WAIT, manager.shutdown_all())
        .await
        .expect("shutdown timeout")
        .expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn awaiting_states_preserve_clean_and_nonzero_exits() {
    let nonzero_status = ExitStatus {
        code: Some(23),
        success: false,
    };
    let (factory, probe) = FakeFactory::new([
        TransportScript::events([TransportEvent::AwaitingHostKey, TransportEvent::Eof]),
        TransportScript::events([
            TransportEvent::AwaitingAuthentication,
            TransportEvent::Exit(nonzero_status),
        ]),
    ]);
    let manager = SessionManager::new(factory);

    let (launch, _) = support::launch(&probe);
    let mut host_key = manager.launch(launch).expect("host-key launch");
    let (states, status) = recv_exit_with_states(&mut host_key.events).await;
    assert_eq!(
        states,
        [
            SessionState::Created,
            SessionState::Connecting,
            SessionState::Connected,
            SessionState::AwaitingHostKey,
            SessionState::Exited,
        ]
    );
    assert_eq!(
        status,
        ExitStatus {
            code: None,
            success: true,
        }
    );
    wait_for_event_channel_close(&mut host_key.events).await;
    assert_unknown_session(&manager, host_key.id);

    let (launch, _) = support::launch(&probe);
    let mut authentication = manager.launch(launch).expect("authentication launch");
    let (states, status) = recv_exit_with_states(&mut authentication.events).await;
    assert_eq!(
        states,
        [
            SessionState::Created,
            SessionState::Connecting,
            SessionState::Connected,
            SessionState::AwaitingAuthentication,
            SessionState::Exited,
        ]
    );
    assert_eq!(status, nonzero_status);
    wait_for_event_channel_close(&mut authentication.events).await;
    assert_unknown_session(&manager, authentication.id);
    manager.shutdown_all().await.expect("empty shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn illegal_lifecycle_transition_fails_deterministically() {
    let (factory, probe) = FakeFactory::new([TransportScript::events([
        TransportEvent::AwaitingHostKey,
        TransportEvent::AwaitingHostKey,
    ])]);
    let manager = SessionManager::new(factory);
    let (launch, _) = support::launch(&probe);
    let mut client = manager.launch(launch).expect("launch");

    assert_eq!(
        recv_failure(&mut client.events).await,
        SessionFailure::Crashed
    );
    tokio::time::timeout(WAIT, manager.shutdown_all())
        .await
        .expect("shutdown timeout")
        .expect("shutdown");
    while let Ok(Ok(event)) =
        tokio::time::timeout(Duration::from_millis(20), client.events.recv()).await
    {
        assert!(
            !matches!(event, SessionEvent::Crashed(_)),
            "illegal transitions are classified failures, not actor panics"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interaction_round_trip_is_routed_without_secret_debug_output() {
    let interaction_id = InteractionId::new();
    let host_key = InteractionRequest::HostKey(HostKeyPrompt {
        id: interaction_id,
        host: "example.test".to_owned(),
        port: 22,
        algorithm: "ssh-ed25519".to_owned(),
        sha256: "SHA256:fixture".to_owned(),
        changed: false,
    });
    let auth_id = InteractionId::new();
    let authentication = InteractionRequest::Password(AuthPrompt {
        id: auth_id,
        label: "Password".to_owned(),
        echo: false,
    });
    let (factory, probe) = FakeFactory::new([TransportScript::interacting_many([
        host_key,
        authentication,
    ])]);
    let manager = SessionManager::new(factory);
    let (launch, _) = support::launch(&probe);
    let mut client = manager.launch(launch).expect("launch");

    let mut states = Vec::new();
    loop {
        match recv(&mut client.events).await {
            SessionEvent::StateChanged(state) => states.push(state),
            SessionEvent::InteractionRequired(InteractionRequest::HostKey(prompt)) => {
                assert_eq!(prompt.id, interaction_id);
                manager
                    .command(
                        client.id,
                        SessionCommand::Respond(
                            interaction_id,
                            InteractionResponse::HostKey(HostKeyDecision::AcceptAndStore),
                        ),
                    )
                    .expect("interaction response");
            }
            SessionEvent::InteractionRequired(InteractionRequest::Password(prompt)) => {
                assert_eq!(prompt.id, auth_id);
                manager
                    .command(
                        client.id,
                        SessionCommand::Respond(
                            auth_id,
                            InteractionResponse::Secret(SecretString::from(
                                "authentication-secret".to_owned(),
                            )),
                        ),
                    )
                    .expect("authentication response");
            }
            _ => {}
        }
        if states.last() == Some(&SessionState::Connected) {
            break;
        }
    }
    assert_eq!(
        states,
        [
            SessionState::Created,
            SessionState::Connecting,
            SessionState::AwaitingHostKey,
            SessionState::AwaitingAuthentication,
            SessionState::Connected
        ]
    );

    let secret = "must-not-appear";
    let paste_debug = format!(
        "{:?}",
        SessionCommand::Paste(SecretString::from(secret.to_owned()))
    );
    let response_debug = format!(
        "{:?}",
        InteractionResponse::Secret(SecretString::from(secret.to_owned()))
    );
    assert!(!paste_debug.contains(secret));
    assert!(!response_debug.contains(secret));

    manager.shutdown_all().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actor_panic_is_generic_once_and_manager_launches_again() {
    let (factory, probe) = FakeFactory::new([
        TransportScript::panic(),
        TransportScript {
            next: NextBehavior::Pending,
            interactions: std::collections::VecDeque::new(),
            write_blocker: None,
            shutdown_failure: None,
        },
    ]);
    let manager = SessionManager::new(factory);
    let (launch, _) = support::launch(&probe);
    let mut crashed = manager.launch(launch).expect("panic launch");

    let mut crash_events = 0;
    let mut crash_states = 0;
    loop {
        match recv(&mut crashed.events).await {
            SessionEvent::StateChanged(SessionState::Crashed) => crash_states += 1,
            SessionEvent::Crashed(message) => {
                crash_events += 1;
                assert_eq!(message, "session actor crashed");
                break;
            }
            _ => {}
        }
    }
    for event in recv_until_closed(&mut crashed.events).await {
        if matches!(event, SessionEvent::Crashed(_)) {
            crash_events += 1;
        }
        if matches!(event, SessionEvent::StateChanged(SessionState::Crashed)) {
            crash_states += 1;
        }
    }
    assert_eq!(crash_events, 1);
    assert_eq!(crash_states, 1);
    assert_unknown_session(&manager, crashed.id);

    let (launch, _) = support::launch(&probe);
    let mut healthy = manager.launch(launch).expect("second launch");
    collect_through_state(&mut healthy.events, SessionState::Connected).await;
    tokio::time::timeout(WAIT, manager.shutdown_all())
        .await
        .expect("manager hung after panic")
        .expect("shutdown");
}

async fn collect_through_state(
    receiver: &mut broadcast::Receiver<SessionEvent>,
    target: SessionState,
) -> Vec<SessionState> {
    let mut states = Vec::new();
    loop {
        if let SessionEvent::StateChanged(state) = recv(receiver).await {
            states.push(state);
            if state == target {
                return states;
            }
        }
    }
}

async fn collect_until_closed(
    receiver: &mut broadcast::Receiver<SessionEvent>,
) -> Vec<SessionState> {
    recv_until_closed(receiver)
        .await
        .into_iter()
        .filter_map(|event| match event {
            SessionEvent::StateChanged(state) => Some(state),
            _ => None,
        })
        .collect()
}

async fn recv_until_closed(receiver: &mut broadcast::Receiver<SessionEvent>) -> Vec<SessionEvent> {
    tokio::time::timeout(WAIT, async {
        let mut events = Vec::new();
        loop {
            match receiver.recv().await {
                Ok(event) => events.push(event),
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    panic!("event receiver lagged by {skipped}")
                }
                Err(broadcast::error::RecvError::Closed) => return events,
            }
        }
    })
    .await
    .expect("event channel did not close")
}

async fn wait_for_event_channel_close(receiver: &mut broadcast::Receiver<SessionEvent>) {
    let _ = recv_until_closed(receiver).await;
}

async fn recv(receiver: &mut broadcast::Receiver<SessionEvent>) -> SessionEvent {
    match tokio::time::timeout(WAIT, receiver.recv())
        .await
        .expect("event timeout")
    {
        Ok(event) => event,
        Err(broadcast::error::RecvError::Lagged(skipped)) => {
            panic!("event receiver lagged by {skipped}")
        }
        Err(broadcast::error::RecvError::Closed) => panic!("event channel closed"),
    }
}

async fn recv_exit(receiver: &mut broadcast::Receiver<SessionEvent>) -> ExitStatus {
    loop {
        if let SessionEvent::Exited(status) = recv(receiver).await {
            return status;
        }
    }
}

async fn recv_exit_with_states(
    receiver: &mut broadcast::Receiver<SessionEvent>,
) -> (Vec<SessionState>, ExitStatus) {
    let mut states = Vec::new();
    loop {
        match recv(receiver).await {
            SessionEvent::StateChanged(state) => states.push(state),
            SessionEvent::Exited(status) => return (states, status),
            _ => {}
        }
    }
}

async fn recv_failure(receiver: &mut broadcast::Receiver<SessionEvent>) -> SessionFailure {
    loop {
        if let SessionEvent::Failed(failure) = recv(receiver).await {
            return failure;
        }
    }
}

async fn wait_until(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(WAIT, async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("condition timeout");
}

fn assert_unknown_session(manager: &SessionManager, id: rshell_core::SessionId) {
    assert_eq!(
        manager.command(id, SessionCommand::Shutdown),
        Err(SessionError::UnknownSession)
    );
}

fn position(log: &[String], expected: &str) -> usize {
    log.iter()
        .position(|entry| entry == expected)
        .unwrap_or_else(|| panic!("missing {expected} in {log:?}"))
}

fn count(log: &[String], expected: &str) -> usize {
    log.iter()
        .filter(|entry| entry.as_str() == expected)
        .count()
}

#[test]
fn factory_is_reusable_and_publicly_object_safe() {
    fn accepts_factory(_: Arc<dyn rshell_session::TransportFactory>) {}
    let (factory, _) = FakeFactory::new([TransportScript::pending()]);
    accepts_factory(factory);
}
