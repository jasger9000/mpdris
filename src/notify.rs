use std::{ffi::CString, time::Duration};

use libc::{c_char, c_int, clock_gettime, timespec, CLOCK_MONOTONIC};

#[link(name = "systemd")]
extern "C" {
    /// int sd_notify(int unset_environment, const char *state);
    fn sd_notify(unset_enviroment: c_int, state: *const c_char) -> c_int;
}

/// Notifies the service manager of state changes.
///
/// Panics when failing to turn state into a Cstring
pub fn notify_systemd(state: &str) {
    let state = CString::new(state).expect("Could not turn string into CString");
    unsafe {
        sd_notify(0, state.as_ptr());
    }
}

pub fn monotonic_time() -> Duration {
    let mut ts = timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe {
        clock_gettime(CLOCK_MONOTONIC, &mut ts);
    }

    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}
