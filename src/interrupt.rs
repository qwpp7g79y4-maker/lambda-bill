//! SIGINT (Ctrl-C) ends the program with exit code 130, a one-line message and
//! no output of its own.
//!
//! The handler does only what a signal handler may do: it writes the message
//! and calls `_exit(130)`. While the report itself is being written the handler
//! only records the signal, so the report is never cut in half; the program
//! exits 130 right after that write. On targets that are not Unix the default
//! disposition stays.
//!
//! This is the only module with `unsafe` code besides `stdout_probe`.

/// Exit status for SIGINT.
pub const EXIT_INTERRUPTED: u8 = 130;

/// The line written to standard error.
pub const MESSAGE: &str = "lambda-bill: interrupted by SIGINT\n";

#[cfg(unix)]
mod imp {
    use std::sync::atomic::{AtomicBool, Ordering};

    extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
        fn write(fd: i32, buf: *const std::ffi::c_void, count: usize) -> isize;
        fn _exit(status: i32) -> !;
    }

    const SIGINT: i32 = 2;

    static WRITING: AtomicBool = AtomicBool::new(false);
    static PENDING: AtomicBool = AtomicBool::new(false);

    extern "C" fn handler(_signum: i32) {
        if WRITING.load(Ordering::SeqCst) {
            PENDING.store(true, Ordering::SeqCst);
            return;
        }
        let message = super::MESSAGE.as_bytes();
        // SAFETY: `write` and `_exit` are async-signal-safe; the buffer is a
        // static string and the length is its own.
        unsafe {
            write(2, message.as_ptr().cast(), message.len());
            _exit(i32::from(super::EXIT_INTERRUPTED));
        }
    }

    pub fn install() {
        // SAFETY: `signal` installs `handler`, an `extern "C" fn(i32)`, for
        // SIGINT. The handler only uses async-signal-safe calls and atomics.
        unsafe { signal(SIGINT, handler as extern "C" fn(i32) as usize) };
    }

    pub fn begin_write() {
        WRITING.store(true, Ordering::SeqCst);
    }

    pub fn end_write() -> bool {
        WRITING.store(false, Ordering::SeqCst);
        PENDING.load(Ordering::SeqCst)
    }
}

#[cfg(not(unix))]
mod imp {
    pub fn install() {}
    pub fn begin_write() {}
    pub fn end_write() -> bool {
        false
    }
}

/// Install the SIGINT handler. Call once, first thing in `main`.
pub fn install() {
    imp::install();
}

/// Mark the start of the write of the report: a SIGINT now waits for it.
pub fn begin_write() {
    imp::begin_write();
}

/// Mark the end of the write. True when a SIGINT arrived during it, so the
/// caller must exit with [`EXIT_INTERRUPTED`] now.
pub fn end_write() -> bool {
    imp::end_write()
}
