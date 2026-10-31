//! Closing the job terminates descendants, including when a request is cancelled.
#[cfg(windows)]
pub struct ProcessTree(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for ProcessTree {}
#[cfg(windows)]
impl ProcessTree {
    pub fn attach(child: &tokio::process::Child) -> Result<Self, String> {
        unsafe {
            use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err("Could not create the command lifecycle boundary.".into());
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const std::ffi::c_void,
                std::mem::size_of_val(&info) as u32,
            ) != 0;
            let assigned = configured
                && child.raw_handle().is_some_and(|handle| {
                    AssignProcessToJobObject(job, handle as windows_sys::Win32::Foundation::HANDLE)
                        != 0
                });
            if !assigned {
                CloseHandle(job);
                return Err("Could not apply command cleanup. The command was stopped.".into());
            }
            Ok(Self(job))
        }
    }
}
#[cfg(windows)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(unix)]
pub struct ProcessTree(Option<i32>);
#[cfg(unix)]
impl ProcessTree {
    pub fn attach(child: &tokio::process::Child) -> Result<Self, String> {
        Ok(Self(child.id().and_then(|id| i32::try_from(id).ok())))
    }
}
#[cfg(unix)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        if let Some(group) = self.0 {
            let _ = unsafe { libc::killpg(group, libc::SIGKILL) };
        }
    }
}
