use std::mem::size_of;
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle};
use std::ptr;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    CreateJobObjectW, IsProcessInJob, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

use crate::PlatformError;

/// Owns one kill-on-close Windows Job used to contain a PTY process tree.
pub struct WindowsProcessJob {
    handle: Option<OwnedHandle>,
}

impl WindowsProcessJob {
    pub fn new() -> Result<Self, PlatformError> {
        let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if raw.is_null() {
            return Err(PlatformError::last_os_error());
        }
        let handle = unsafe { OwnedHandle::from_raw_handle(raw.cast()) };
        let mut information = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                handle.as_raw_handle().cast(),
                JobObjectExtendedLimitInformation,
                (&raw const information).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(PlatformError::last_os_error());
        }
        Ok(Self {
            handle: Some(handle),
        })
    }

    pub fn as_borrowed_handle(&self) -> BorrowedHandle<'_> {
        self.handle
            .as_ref()
            .expect("a terminated Job cannot be borrowed")
            .as_handle()
    }

    pub fn contains_process(&self, process_id: u32) -> Result<bool, PlatformError> {
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
        if process.is_null() {
            return Err(PlatformError::last_os_error());
        }
        let process = ProcessHandle(process);
        let mut contained = 0;
        let result = unsafe { IsProcessInJob(process.0, self.raw_handle()?, &mut contained) };
        if result == 0 {
            Err(PlatformError::last_os_error())
        } else {
            Ok(contained != 0)
        }
    }

    pub fn terminate(&mut self) -> Result<(), PlatformError> {
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };
        let result = unsafe { TerminateJobObject(handle.as_raw_handle().cast(), 1) };
        drop(handle);
        if result == 0 {
            Err(PlatformError::last_os_error())
        } else {
            Ok(())
        }
    }

    fn raw_handle(&self) -> Result<HANDLE, PlatformError> {
        self.handle
            .as_ref()
            .map(|handle| handle.as_raw_handle().cast())
            .ok_or_else(PlatformError::last_os_error)
    }
}

impl Drop for WindowsProcessJob {
    fn drop(&mut self) {
        self.handle.take();
    }
}

struct ProcessHandle(HANDLE);

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
