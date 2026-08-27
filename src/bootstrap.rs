use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use rshell_core::{AppBootstrapState, AppDependencies, ApplicationHandle, ApplicationService};
use rshell_platform::{PlatformPaths, configure_runtime};
use rshell_session::{KnownHostsVerifier, SessionManager, ports::SessionPortAdapter};
use rshell_storage::{
    CredentialCoordinator, SqliteRepository, SystemCredentialVault,
    ports::{
        CredentialPortAdapter, ImportPortAdapter, ImportPreviewCleanup, RepositoryPortAdapter,
    },
};

pub(crate) const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BootstrapError {
    Runtime,
    PlatformConfigure,
    PlatformPaths,
    StorageOpen,
    StorageMigrate,
    CredentialsReconcile,
    CatalogLoad,
    SettingsLoad,
    ApplicationStart,
    ImportCleanup,
    ApplicationShutdown,
    SessionShutdown,
    StorageShutdown,
    StartupCleanup,
    P0Cleanup,
}

impl BootstrapError {
    pub(crate) const fn category(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::PlatformConfigure | Self::PlatformPaths => "platform",
            Self::StorageOpen
            | Self::StorageMigrate
            | Self::CatalogLoad
            | Self::SettingsLoad
            | Self::StorageShutdown => "storage",
            Self::CredentialsReconcile => "credentials",
            Self::ApplicationStart | Self::ApplicationShutdown => "application",
            Self::ImportCleanup => "cleanup",
            Self::SessionShutdown => "session",
            Self::StartupCleanup | Self::P0Cleanup => "cleanup",
        }
    }

    pub(crate) const fn context(self) -> &'static str {
        match self {
            Self::Runtime => "creating async runtime",
            Self::PlatformConfigure => "configuring platform runtime",
            Self::PlatformPaths => "creating platform paths",
            Self::StorageOpen => "opening state storage",
            Self::StorageMigrate => "migrating state storage",
            Self::CredentialsReconcile => "reconciling credentials",
            Self::CatalogLoad => "loading connection catalog",
            Self::SettingsLoad => "loading terminal settings",
            Self::ApplicationStart => "starting application service",
            Self::ImportCleanup => "stopping import preview cleanup",
            Self::ApplicationShutdown => "stopping application service",
            Self::SessionShutdown => "stopping local sessions",
            Self::StorageShutdown => "stopping state storage",
            Self::StartupCleanup => "cleaning failed bootstrap",
            Self::P0Cleanup => "cleaning P0 smoke state",
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct BootstrapObserver(Arc<Mutex<Vec<&'static str>>>);

impl BootstrapObserver {
    pub(crate) fn record(&self, call: &'static str) {
        lock(&self.0).push(call);
    }

    #[cfg(test)]
    fn calls(&self) -> Vec<&'static str> {
        lock(&self.0).clone()
    }
}

pub(crate) struct BootstrappedApplication {
    pub(crate) application: ApplicationHandle,
    pub(crate) repository: Arc<SqliteRepository>,
    pub(crate) sessions: Arc<SessionManager>,
    pub(crate) credentials: Arc<CredentialCoordinator>,
    pub(crate) import_cleanup: ImportPreviewCleanup,
    pub(crate) observer: BootstrapObserver,
}

pub(crate) fn create_runtime() -> Result<relm4::tokio::runtime::Runtime, BootstrapError> {
    relm4::tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|_| BootstrapError::Runtime)
}

pub(crate) async fn start(
    paths: &PlatformPaths,
    observer: &BootstrapObserver,
) -> Result<BootstrappedApplication, BootstrapError> {
    configure_runtime(paths).map_err(|_| BootstrapError::PlatformConfigure)?;
    observer.record("platform.configure");
    paths
        .ensure_exists()
        .map_err(|_| BootstrapError::PlatformPaths)?;

    let repository = Arc::new(
        SqliteRepository::open(paths.state_dir.join("rshell.sqlite3"))
            .map_err(|_| BootstrapError::StorageOpen)?,
    );
    observer.record("storage.open");

    let result = start_with_repository(paths, Arc::clone(&repository), observer).await;
    match result {
        Ok(application) => Ok(application),
        Err(error) if crate::cleanup::shutdown_repository(Arc::clone(&repository)) => Err(error),
        Err(_) => Err(BootstrapError::StartupCleanup),
    }
}

async fn start_with_repository(
    paths: &PlatformPaths,
    repository: Arc<SqliteRepository>,
    observer: &BootstrapObserver,
) -> Result<BootstrappedApplication, BootstrapError> {
    repository
        .migrate()
        .map_err(|_| BootstrapError::StorageMigrate)?;
    observer.record("storage.migrate");

    let vault = Arc::new(SystemCredentialVault::new());
    let coordinator = Arc::new(CredentialCoordinator::new(Arc::clone(&repository), vault));
    coordinator
        .reconcile()
        .map_err(|_| BootstrapError::CredentialsReconcile)?;
    observer.record("credentials.reconcile");

    let catalog = repository
        .load_catalog()
        .map_err(|_| BootstrapError::CatalogLoad)?;
    observer.record("catalog.load");
    let settings = repository
        .load_settings()
        .map_err(|_| BootstrapError::SettingsLoad)?;
    let terminal_profiles = repository
        .load_terminal_profiles()
        .map_err(|_| BootstrapError::SettingsLoad)?;
    observer.record("settings.load");

    let repository_port = Arc::new(RepositoryPortAdapter::new(Arc::clone(&repository)));
    let credential_port = Arc::new(CredentialPortAdapter::new(Arc::clone(&coordinator)));
    let import_port = Arc::new(ImportPortAdapter::new(
        Arc::clone(&repository),
        Arc::clone(&coordinator),
    ));
    let session_port = Arc::new(SessionPortAdapter::with_local_manager(
        KnownHostsVerifier::for_platform(paths),
    ));
    let sessions = Arc::clone(session_port.manager());
    let application = ApplicationService::start(
        AppDependencies {
            repository: repository_port,
            credentials: credential_port,
            imports: import_port.clone(),
            sessions: session_port,
        },
        AppBootstrapState {
            catalog,
            settings,
            terminal_profiles,
        },
    )
    .await;
    let application = match application {
        Ok(application) => application,
        Err(_) => {
            let sessions_clean =
                relm4::tokio::time::timeout(SHUTDOWN_TIMEOUT, sessions.shutdown_all())
                    .await
                    .is_ok_and(|result| result.is_ok() && sessions.active_session_count() == 0);
            return Err(if sessions_clean {
                BootstrapError::ApplicationStart
            } else {
                BootstrapError::StartupCleanup
            });
        }
    };
    observer.record("application.start");
    let import_cleanup = ImportPreviewCleanup::start(&import_port);
    observer.record("import.cleanup.start");

    Ok(BootstrappedApplication {
        application,
        repository,
        sessions,
        credentials: coordinator,
        import_cleanup,
        observer: observer.clone(),
    })
}

impl BootstrappedApplication {
    pub(crate) async fn shutdown(self) -> Result<(), BootstrapError> {
        crate::cleanup::shutdown(self).await
    }

    pub(crate) async fn shutdown_p0(
        self,
        temporary_root: &std::path::Path,
        secret_environment: &[String],
    ) -> crate::cleanup::P0Shutdown {
        crate::cleanup::shutdown_p0(self, temporary_root, secret_environment).await
    }
}

pub(crate) const fn needs_emergency_session_cleanup(
    application_succeeded: bool,
    active_sessions: usize,
) -> bool {
    !application_succeeded || active_sessions != 0
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, fs, path::PathBuf, time::SystemTime};

    use super::*;

    #[test]
    fn composition_root_bootstraps_each_process_service_exactly_once_in_order() {
        let _shell = ShellOverride::deterministic();
        let root = std::env::temp_dir().join(format!(
            "rshell-bootstrap-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let paths =
            PlatformPaths::from_roots(root.join("config"), root.join("state"), root.join("cache"));
        let runtime = create_runtime().expect("runtime must start");
        let observer = BootstrapObserver::default();
        let application = runtime
            .block_on(start(&paths, &observer))
            .expect("production bootstrap must start");

        assert_eq!(
            observer.calls(),
            [
                "platform.configure",
                "storage.open",
                "storage.migrate",
                "credentials.reconcile",
                "catalog.load",
                "settings.load",
                "application.start",
                "import.cleanup.start",
            ]
        );
        runtime
            .block_on(application.shutdown())
            .expect("production bootstrap must shut down");
        assert_eq!(
            observer.calls(),
            [
                "platform.configure",
                "storage.open",
                "storage.migrate",
                "credentials.reconcile",
                "catalog.load",
                "settings.load",
                "application.start",
                "import.cleanup.start",
                "import.cleanup.shutdown",
                "application.sessions.shutdown",
                "storage.shutdown",
            ]
        );
        fs::remove_dir_all(root).expect("test state must be removed");
    }

    struct ShellOverride(Option<OsString>);

    impl ShellOverride {
        fn deterministic() -> Self {
            let previous = std::env::var_os("RSHELL_SHELL");
            let shell = deterministic_shell();
            assert!(shell.is_file(), "deterministic test shell must exist");
            // SAFETY: this root test is the only test in this binary that reads RSHELL_SHELL.
            unsafe { std::env::set_var("RSHELL_SHELL", shell) };
            Self(previous)
        }
    }

    impl Drop for ShellOverride {
        fn drop(&mut self) {
            // SAFETY: the guard restores the process environment before this test returns.
            unsafe {
                match self.0.take() {
                    Some(previous) => std::env::set_var("RSHELL_SHELL", previous),
                    None => std::env::remove_var("RSHELL_SHELL"),
                }
            }
        }
    }

    fn deterministic_shell() -> PathBuf {
        #[cfg(windows)]
        {
            let windows = std::env::var_os("WINDIR").expect("WINDIR must be defined");
            PathBuf::from(windows).join("System32").join("where.exe")
        }
        #[cfg(not(windows))]
        {
            [PathBuf::from("/usr/bin/true"), PathBuf::from("/bin/true")]
                .into_iter()
                .find(|path| path.is_file())
                .expect("a standard true executable must exist")
        }
    }

    #[test]
    fn completed_application_does_not_schedule_emergency_session_cleanup() {
        assert!(!needs_emergency_session_cleanup(true, 0));
        assert!(needs_emergency_session_cleanup(false, 0));
        assert!(needs_emergency_session_cleanup(true, 1));
    }
}
