use anyhow::{Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
#[cfg(windows)]
const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

#[cfg(windows)]
#[link(name = "Kernel32")]
unsafe extern "system" {
    #[link_name = "MoveFileExW"]
    fn move_file_ex_w(existing_file_name: *const u16, new_file_name: *const u16, flags: u32)
    -> i32;
}

pub(crate) fn write_file_durable(path: &Path, data: &str) -> Result<()> {
    write_file_durable_inner(path, data, true)
}

pub(crate) fn write_file_durable_without_backup(path: &Path, data: &str) -> Result<()> {
    write_file_durable_inner(path, data, false)
}

fn write_file_durable_inner(path: &Path, data: &str, refresh_existing_backup: bool) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let (tmp_path, mut tmp_file) = create_temp_file(path)?;
    let result = (|| -> Result<()> {
        tmp_file
            .write_all(data.as_bytes())
            .with_context(|| format!("failed to write {}", tmp_path.display()))?;
        tmp_file
            .sync_all()
            .with_context(|| format!("failed to flush {}", tmp_path.display()))?;
        drop(tmp_file);
        if refresh_existing_backup {
            refresh_backup(path)?;
        }
        replace_file(&tmp_path, path)?;
        sync_parent_dir(path)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }

    result
}

pub(crate) fn recover_backup_if_missing(path: &Path) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }

    recover_backup(path)
}

pub(crate) fn recover_backup(path: &Path) -> Result<bool> {
    let backup_path = backup_path(path);
    if !backup_path.exists() {
        return Ok(false);
    }

    fs::copy(&backup_path, path).with_context(|| {
        format!(
            "failed to recover {} from {}",
            path.display(),
            backup_path.display()
        )
    })?;
    fs::remove_file(&backup_path)
        .with_context(|| format!("failed to remove {}", backup_path.display()))?;
    sync_parent_dir(path)?;
    Ok(true)
}

fn refresh_backup(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let backup_path = backup_path(path);
    fs::copy(path, &backup_path).with_context(|| {
        format!(
            "failed to refresh backup {} from {}",
            backup_path.display(),
            path.display()
        )
    })?;
    let backup = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&backup_path)
        .with_context(|| format!("failed to open backup {}", backup_path.display()))?;
    backup
        .sync_all()
        .with_context(|| format!("failed to flush backup {}", backup_path.display()))?;
    Ok(())
}

fn create_temp_file(path: &Path) -> Result<(PathBuf, File)> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "rshell-config".into());
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    for attempt in 0..100u32 {
        let candidate = parent.join(format!(".{file_name}.{pid}.{nanos}.{attempt}.tmp"));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to create {}", candidate.display()));
            }
        }
    }

    anyhow::bail!("failed to allocate a temporary file for {}", path.display());
}

#[cfg(not(windows))]
fn replace_file(tmp_path: &Path, path: &Path) -> Result<()> {
    fs::rename(tmp_path, path).with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

#[cfg(windows)]
fn replace_file(tmp_path: &Path, path: &Path) -> Result<()> {
    let path_label = path.display().to_string();
    let tmp_path = windows_path(tmp_path);
    let target_path = windows_path(path);
    let ok = unsafe {
        move_file_ex_w(
            tmp_path.as_ptr(),
            target_path.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("failed to replace {path_label}"));
    }
    Ok(())
}

fn backup_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "rshell-config".into());
    parent.join(format!("{file_name}.bak"))
}

#[cfg(windows)]
fn windows_path(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(not(windows))]
fn sync_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        let dir = File::open(parent)
            .with_context(|| format!("failed to open directory {}", parent.display()))?;
        dir.sync_all()
            .with_context(|| format!("failed to flush directory {}", parent.display()))?;
    }
    Ok(())
}

#[cfg(windows)]
fn sync_parent_dir(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_write_creates_and_replaces_file() {
        let path = std::env::temp_dir().join(format!(
            "rshell-durable-write-{}.json",
            uuid::Uuid::new_v4()
        ));

        write_file_durable(&path, "{\"value\":1}").unwrap();
        write_file_durable(&path, "{\"value\":2}").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"value\":2}");
        assert_eq!(
            std::fs::read_to_string(backup_path(&path)).unwrap(),
            "{\"value\":1}"
        );

        let _ = std::fs::remove_file(backup_path(&path));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn recovers_backup_when_target_is_missing() {
        let path = std::env::temp_dir().join(format!(
            "rshell-durable-recover-{}.json",
            uuid::Uuid::new_v4()
        ));
        let backup = backup_path(&path);

        std::fs::write(&backup, "{\"value\":3}").unwrap();

        assert!(recover_backup_if_missing(&path).unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"value\":3}");
        assert!(!backup.exists());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn recovers_backup_over_corrupt_target() {
        let path = std::env::temp_dir().join(format!(
            "rshell-durable-recover-corrupt-{}.json",
            uuid::Uuid::new_v4()
        ));
        let backup = backup_path(&path);

        std::fs::write(&path, "{not-json").unwrap();
        std::fs::write(&backup, "{\"value\":4}").unwrap();

        assert!(recover_backup(&path).unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"value\":4}");
        assert!(!backup.exists());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn durable_write_without_backup_replaces_without_copying_secret() {
        let path = std::env::temp_dir().join(format!(
            "rshell-durable-no-backup-{}.json",
            uuid::Uuid::new_v4()
        ));
        let backup = backup_path(&path);

        write_file_durable(&path, "{\"password\":\"secret\"}").unwrap();
        let _ = std::fs::remove_file(&backup);
        write_file_durable_without_backup(&path, "{\"password\":\"\"}").unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\"password\":\"\"}"
        );
        assert!(!backup.exists());

        let _ = std::fs::remove_file(path);
    }
}
