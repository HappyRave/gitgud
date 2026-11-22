use std::io;
use std::process::{Child, Command};

/// A child process and every process it starts, ended together by [`ProcessTree::kill`].
///
/// On Windows the child starts suspended, joins a Job Object, then resumes, so
/// nothing it spawns can escape the job (spike 4.5: `taskkill /T` misses
/// grandchildren whose parent already exited, e.g. a shell's background job).
pub(crate) struct ProcessTree {
    pub child: Child,
    #[cfg(windows)]
    job: windows_sys::Win32::Foundation::HANDLE,
}

// The job handle is only used through this owner.
#[cfg(windows)]
unsafe impl Send for ProcessTree {}

#[cfg(windows)]
impl ProcessTree {
    pub fn spawn(cmd: &mut Command) -> io::Result<Self> {
        use std::os::windows::io::AsRawHandle;
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
        use windows_sys::Win32::System::JobObjects::{AssignProcessToJobObject, CreateJobObjectW};
        use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED};

        cmd.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
        let mut child = cmd.spawn()?;
        let fail = |child: &mut Child, job: HANDLE| {
            let err = io::Error::last_os_error();
            let _ = child.kill();
            if !job.is_null() {
                // SAFETY: `job` came from CreateJobObjectW and is closed once.
                unsafe { CloseHandle(job) };
            }
            err
        };
        // SAFETY: plain Win32 calls; null attributes and name are allowed.
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return Err(fail(&mut child, job));
        }
        // SAFETY: both handles are valid; the process is suspended, so it has
        // started nothing yet.
        if unsafe { AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) } == 0 {
            return Err(fail(&mut child, job));
        }
        if let Err(e) = resume(child.id()) {
            let _ = fail(&mut child, job);
            return Err(e);
        }
        Ok(Self { child, job })
    }

    /// Ends the child and every process in its job.
    pub fn kill(&mut self) {
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;
        // SAFETY: `self.job` is a live job handle owned by `self`.
        if unsafe { TerminateJobObject(self.job, 1) } == 0 {
            let _ = self.child.kill();
        }
    }
}

#[cfg(windows)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        // SAFETY: closed exactly once, here. Without KILL_ON_JOB_CLOSE this leaves
        // background work git started (an auto gc) running.
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.job) };
    }
}

/// Resumes every thread of a process started with `CREATE_SUSPENDED`.
#[cfg(windows)]
fn resume(pid: u32) -> io::Result<()> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    // SAFETY: snapshot of all threads; the handle is closed below.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(std::mem::size_of::<THREADENTRY32>()).expect("struct size fits u32"),
        cntUsage: 0,
        th32ThreadID: 0,
        th32OwnerProcessID: 0,
        tpBasePri: 0,
        tpDeltaPri: 0,
        dwFlags: 0,
    };
    let mut resumed = 0;
    // SAFETY: `entry` is a correctly sized THREADENTRY32 for the whole walk.
    let mut more = unsafe { Thread32First(snapshot, &raw mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: the thread belongs to our suspended child; handle closed below.
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if !thread.is_null() {
                // SAFETY: valid thread handle with resume rights.
                if unsafe { ResumeThread(thread) } != u32::MAX {
                    resumed += 1;
                }
                // SAFETY: closed once.
                unsafe { CloseHandle(thread) };
            }
        }
        // SAFETY: as above.
        more = unsafe { Thread32Next(snapshot, &raw mut entry) } != 0;
    }
    // SAFETY: closed once.
    unsafe { CloseHandle(snapshot) };
    if resumed == 0 {
        return Err(io::Error::other(format!("could not resume process {pid}")));
    }
    Ok(())
}

#[cfg(not(windows))]
impl ProcessTree {
    pub fn spawn(cmd: &mut Command) -> io::Result<Self> {
        Ok(Self {
            child: cmd.spawn()?,
        })
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}
