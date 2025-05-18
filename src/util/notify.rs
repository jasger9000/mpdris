use std::{ffi::CString, time::Duration};

use libc::{CLOCK_MONOTONIC, c_char, c_int, clock_gettime, timespec};
use libloading::{Library, library_filename};

#[derive(Debug)]
pub struct Systemd {
    _library: Library,
    sd_notify: unsafe extern "C" fn(unset_environment: c_int, state: *const c_char) -> c_int,
}

impl Systemd {
    pub fn new() -> Result<Self, libloading::Error> {
        unsafe {
            let lib = Library::new(library_filename("systemd"))?;
            let sd_notify = *lib.get(b"sd_notify")?;

            Ok(Self {
                _library: lib,
                sd_notify,
            })
        }
    }

    /// Notifies the service manager of state changes.
    ///
    /// Panics when failing to turn state into a Cstring
    pub fn notify(&self, state: &str) {
        let state = CString::new(state).expect("Could not turn string into CString");
        unsafe {
            (self.sd_notify)(0, state.as_ptr());
        }
    }
}

pub fn monotonic_time() -> Duration {
    let mut ts = timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe {
        clock_gettime(CLOCK_MONOTONIC, &mut ts);
    }

    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}
