use std::{
    sync::{Arc, mpsc},
    time::Duration,
};

use rshell_core::AppError;
use rshell_session::SessionManager;
use rshell_storage::SqliteRepository;

use crate::bootstrap::{
    BootstrapError, BootstrappedApplication, SHUTDOWN_TIMEOUT, needs_emergency_session_cleanup,
};
use crate::p0_smoke_cleanup::{
    P0CleanupEvidence, delete_temporary_credentials, scan_temporary_state,
};

pub(crate) struct P0Shutdown {
    pub(crate) evidence: P0CleanupEvidence,
    pub(crate) error: Option<BootstrapError>,
}

pub(crate) async fn shutdown(application: BootstrappedApplication) -> Result<(), BootstrapError> {
    let BootstrappedApplication {
        application,
        repository,
        sessions,
        credentials: _,
        import_cleanup,
        observer,
    } = application;
    let import_cleanup_error = import_cleanup
        .shutdown()
        .await
        .err()
        .map(|_| BootstrapError::ImportCleanup);
    observer.record("import.cleanup.shutdown");
    let lifecycle_error = shutdown_application_and_sessions(application, &sessions).await;
    observer.record("application.sessions.shutdown");
    let storage_error =
        (!shutdown_repository(repository)).then_some(BootstrapError::StorageShutdown);
    observer.record("storage.shutdown");

    import_cleanup_error
        .or(lifecycle_error)
        .or(storage_error)
        .map_or(Ok(()), Err)
}

pub(crate) async fn shutdown_p0(
    application: BootstrappedApplication,
    temporary_root: &std::path::Path,
    secret_environment: &[String],
) -> P0Shutdown {
    let BootstrappedApplication {
        application,
        repository,
        sessions,
        credentials,
        import_cleanup,
        observer,
    } = application;
    let import_cleanup_error = import_cleanup
        .shutdown()
        .await
        .err()
        .map(|_| BootstrapError::ImportCleanup);
    observer.record("import.cleanup.shutdown");
    let lifecycle_error = shutdown_application_and_sessions(application, &sessions).await;
    observer.record("application.sessions.shutdown");
    let mut evidence = P0CleanupEvidence::new();
    evidence.application_shutdown_clean = Some(lifecycle_error.is_none());
    evidence.actor_count = Some(sessions.active_session_count());
    evidence.direct_session_child_count = Some(sessions.active_child_process_count());
    let credentials_clean = delete_temporary_credentials(&credentials, &repository, &mut evidence);
    let storage_clean = shutdown_repository(repository);
    observer.record("storage.shutdown");
    evidence.repository_shutdown_clean = Some(storage_clean);
    let state_clean = scan_temporary_state(temporary_root, secret_environment, &mut evidence);
    let cleanup_error = (!credentials_clean || !state_clean).then_some(BootstrapError::P0Cleanup);
    P0Shutdown {
        evidence,
        error: import_cleanup_error
            .or(cleanup_error)
            .or((!storage_clean).then_some(BootstrapError::StorageShutdown))
            .or(lifecycle_error),
    }
}

pub(crate) fn shutdown_repository(repository: Arc<SqliteRepository>) -> bool {
    let (result_tx, result_rx) = mpsc::sync_channel(1);
    if std::thread::Builder::new()
        .name("rshell-storage-shutdown".into())
        .spawn(move || {
            let _ = result_tx.send(repository.shutdown().is_ok());
        })
        .is_err()
    {
        return false;
    }
    result_rx
        .recv_timeout(Duration::from_secs(5))
        .is_ok_and(|result| result)
}

async fn emergency_shutdown_sessions(sessions: &SessionManager) -> Result<(), BootstrapError> {
    relm4::tokio::time::timeout(SHUTDOWN_TIMEOUT, sessions.shutdown_all())
        .await
        .ok()
        .and_then(Result::ok)
        .filter(|()| sessions.active_session_count() == 0)
        .ok_or(BootstrapError::SessionShutdown)
}

async fn shutdown_application_and_sessions(
    application: rshell_core::ApplicationHandle,
    sessions: &SessionManager,
) -> Option<BootstrapError> {
    let application_error = relm4::tokio::time::timeout(SHUTDOWN_TIMEOUT, application.shutdown())
        .await
        .map_or(Some(BootstrapError::ApplicationShutdown), |result| {
            result.err().map(application_error)
        });
    let session_error = if needs_emergency_session_cleanup(
        application_error.is_none(),
        sessions.active_session_count(),
    ) {
        emergency_shutdown_sessions(sessions).await.err()
    } else {
        None
    };
    application_error.or(session_error)
}

fn application_error(error: AppError) -> BootstrapError {
    match error {
        AppError::SessionShutdown(_) => BootstrapError::SessionShutdown,
        _ => BootstrapError::ApplicationShutdown,
    }
}
