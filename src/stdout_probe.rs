//! One of the two modules with `unsafe` code (the other is `interrupt`).
//!
//! Rust gives a closed file descriptor 0, 1 or 2 back as `/dev/null` (read and
//! write) before `main`, and its standard output ignores the error of a write
//! to a descriptor that is not open for writing. Afterwards `>&-` and `1<file`
//! look like a working output. A constructor in `.init_array` runs earlier than
//! that replacement and records whether descriptor 1 was closed, or open for
//! reading only, at the start of the process.

use std::sync::atomic::{AtomicBool, Ordering};

static FD1_UNWRITABLE: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "linux")]
mod probe {
    use super::{AtomicBool, Ordering, FD1_UNWRITABLE};

    extern "C" {
        fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    }
    const F_GETFD: i32 = 1;
    const F_GETFL: i32 = 3;
    /// Mask of the access mode in the status flags. `O_RDONLY` is 0.
    const O_ACCMODE: i32 = 3;

    extern "C" fn record() {
        // SAFETY: `fcntl` with `F_GETFD` and `F_GETFL` only reads flags of
        // descriptor 1 and changes nothing.
        let closed = unsafe { fcntl(1, F_GETFD) } == -1;
        let read_only = !closed && (unsafe { fcntl(1, F_GETFL) } & O_ACCMODE) == 0;
        let flag: &AtomicBool = &FD1_UNWRITABLE;
        flag.store(closed || read_only, Ordering::Relaxed);
    }

    #[used]
    #[link_section = ".init_array"]
    static INIT: extern "C" fn() = record;
}

/// Was descriptor 1 closed or read-only when the process started? Only
/// reliable on Linux. Elsewhere it says "no" and the write itself decides.
pub fn stdout_unwritable_at_start() -> bool {
    FD1_UNWRITABLE.load(Ordering::Relaxed)
}
