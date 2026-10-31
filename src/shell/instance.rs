//! Only one copy of the app runs at a time per data folder. The first copy holds a named Windows
//! mutex and waits on a named event; a second copy finds the mutex taken, sets the event and exits,
//! and the first copy shows and focuses its window.

use std::path::Path;

/// What launching found out.
pub enum Launch {
    /// This is the first copy: keep the guard for as long as the app runs.
    First(Guard),
    /// Another copy runs; it has been asked to come forward.
    Second,
}

/// Holds the single-instance mutex, and delivers the other copies' requests to show the window.
pub struct Guard {
    shows: async_channel::Receiver<()>,
    #[allow(dead_code, reason = "owning the handle is what keeps the mutex taken")]
    mutex: imp::Handle,
}

impl Guard {
    /// One message for each time another copy was launched.
    pub fn show_requests(&self) -> async_channel::Receiver<()> {
        self.shows.clone()
    }
}

/// Take the lock for the app whose data folder is `data_dir`, or ask the copy that has it to come
/// forward. Different data folders (a debug build, or a scratch folder) never get in each other's way.
pub fn acquire(data_dir: &Path) -> Launch {
    acquire_named(&name_for(data_dir))
}

/// The name shared by the mutex and the event of the copies using `data_dir`.
fn name_for(data_dir: &Path) -> String {
    // FNV-1a of the folder's name, which is stable between runs (unlike `DefaultHasher`).
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in data_dir.to_string_lossy().to_lowercase().replace('/', "\\").trim_end_matches('\\').bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("BazaarTracker-{hash:016x}")
}

fn acquire_named(name: &str) -> Launch {
    match imp::lock(name) {
        imp::Lock::First(mutex, shows) => Launch::First(Guard { shows, mutex }),
        imp::Lock::Second => {
            imp::signal(name);
            Launch::Second
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    type RawHandle = *mut c_void;

    const ERROR_ALREADY_EXISTS: u32 = 183;
    const INFINITE: u32 = u32::MAX;
    const WAIT_OBJECT_0: u32 = 0;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateMutexW(attributes: *const c_void, initial_owner: i32, name: *const u16) -> RawHandle;
        fn CreateEventW(attributes: *const c_void, manual_reset: i32, initial_state: i32, name: *const u16) -> RawHandle;
        fn SetEvent(handle: RawHandle) -> i32;
        fn WaitForSingleObject(handle: RawHandle, milliseconds: u32) -> u32;
        fn CloseHandle(handle: RawHandle) -> i32;
        fn GetLastError() -> u32;
    }

    /// A Windows handle, closed when dropped.
    pub struct Handle(RawHandle);

    // SAFETY: kernel handles may be used from any thread.
    unsafe impl Send for Handle {}
    unsafe impl Sync for Handle {}

    impl Handle {
        fn raw(&self) -> RawHandle {
            self.0
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: the handle came from a successful Create* call and is closed once.
            unsafe { CloseHandle(self.0) };
        }
    }

    pub enum Lock {
        First(Handle, async_channel::Receiver<()>),
        Second,
    }

    fn wide(text: &str) -> Vec<u16> {
        std::ffi::OsStr::new(text).encode_wide().chain(Some(0)).collect()
    }

    fn event(name: &str) -> Option<Handle> {
        let name = wide(&format!("Local\\{name}-show"));
        // SAFETY: `name` is a NUL-terminated wide string that outlives the call.
        let handle = unsafe { CreateEventW(std::ptr::null(), 0, 0, name.as_ptr()) };
        (!handle.is_null()).then_some(Handle(handle))
    }

    pub fn lock(name: &str) -> Lock {
        let mutex_name = wide(&format!("Local\\{name}"));
        // SAFETY: `mutex_name` is a NUL-terminated wide string that outlives the call.
        let mutex = unsafe { CreateMutexW(std::ptr::null(), 1, mutex_name.as_ptr()) };
        // SAFETY: read straight after the call it reports on.
        let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        if mutex.is_null() {
            // Cannot tell, so do not stop the player from starting the app.
            let (_, shows) = async_channel::unbounded();
            return Lock::First(Handle(std::ptr::null_mut()), shows);
        }
        let mutex = Handle(mutex);
        if exists {
            return Lock::Second;
        }
        let (sender, shows) = async_channel::unbounded();
        if let Some(event) = event(name) {
            std::thread::Builder::new()
                .name("single-instance".into())
                .spawn(move || {
                    // SAFETY: the handle is owned by this thread, which never ends before the process.
                    while unsafe { WaitForSingleObject(event.raw(), INFINITE) } == WAIT_OBJECT_0 {
                        if sender.send_blocking(()).is_err() {
                            break;
                        }
                    }
                })
                .ok();
        }
        Lock::First(mutex, shows)
    }

    pub fn signal(name: &str) {
        if let Some(event) = event(name) {
            // SAFETY: the handle is valid until `event` is dropped.
            unsafe { SetEvent(event.raw()) };
        }
    }

}

#[cfg(not(windows))]
mod imp {
    /// Other platforms have no single-instance lock yet.
    pub struct Handle;

    pub enum Lock {
        First(Handle, async_channel::Receiver<()>),
        Second,
    }

    pub fn lock(_name: &str) -> Lock {
        let (_, shows) = async_channel::unbounded();
        Lock::First(Handle, shows)
    }

    pub fn signal(_name: &str) {}
}

#[cfg(test)]
#[path = "instance_tests.rs"]
mod tests;
