use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use rshell_platform::{create_private_file, durable_replace_user_file, harden_private_file};
use russh::keys::{PublicKey, known_hosts};

use super::{HostKeyError, HostKeyStorageStep};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) fn store(
    destination: &Path,
    host: &str,
    port: u16,
    key: &PublicKey,
) -> Result<(), HostKeyError> {
    let Some(parent) = destination.parent() else {
        return Err(storage_error(host, port, HostKeyStorageStep::CreateParent));
    };
    fs::create_dir_all(parent)
        .map_err(|_| storage_error(host, port, HostKeyStorageStep::CreateParent))?;

    let (temporary_path, temporary_file) = create_private_temporary_file(parent)
        .map_err(|_| storage_error(host, port, HostKeyStorageStep::CreateTemporary))?;
    let temporary = TemporaryKnownHostsFile(temporary_path);
    {
        let mut temporary_file = temporary_file;
        copy_existing_file(destination, &mut temporary_file)
            .map_err(|_| storage_error(host, port, HostKeyStorageStep::CopyExisting))?;
        temporary_file
            .flush()
            .and_then(|()| temporary_file.sync_all())
            .map_err(|_| storage_error(host, port, HostKeyStorageStep::SyncTemporary))?;
    }

    known_hosts::learn_known_hosts_path(host, port, key, temporary.path())
        .map_err(|_| storage_error(host, port, HostKeyStorageStep::Learn))?;
    OpenOptions::new()
        .write(true)
        .open(temporary.path())
        .and_then(|file| file.sync_all())
        .map_err(|_| storage_error(host, port, HostKeyStorageStep::SyncLearned))?;
    harden_private_file(temporary.path())
        .map_err(|_| storage_error(host, port, HostKeyStorageStep::HardenTemporary))?;
    durable_replace_user_file(temporary.path(), destination)
        .map_err(|_| storage_error(host, port, HostKeyStorageStep::Replace))?;
    Ok(())
}

fn storage_error(host: &str, port: u16, step: HostKeyStorageStep) -> HostKeyError {
    HostKeyError::storage(host, port, step)
}

fn copy_existing_file(path: &Path, target: &mut File) -> io::Result<()> {
    match File::open(path) {
        Ok(mut source) => {
            io::copy(&mut source, target)?;
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn create_private_temporary_file(
    parent: &Path,
) -> Result<(std::path::PathBuf, File), rshell_platform::PlatformError> {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = parent.join(format!(
        ".rshell-known-hosts-{}-{sequence}.tmp",
        std::process::id()
    ));
    create_private_file(&path).map(|file| (path, file))
}

struct TemporaryKnownHostsFile(std::path::PathBuf);

impl TemporaryKnownHostsFile {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryKnownHostsFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
