//! Logger initialization with custom record formatting.
//!
//! The formatter includes timestamp, level, thread/task identifier, module
//! path, and message body to make async scraping logs easier to inspect.

use std::{
    io::Write,
    mem,
    num::NonZero,
    thread::{self, ThreadId},
};

use chrono::Local;
use env_logger::Env;

/// Returns the current thread's ID as a numeric `u64` for use in log output.
///
/// # Safety and implementation notes
///
/// This function uses `unsafe` and `mem::transmute` to interpret `std::thread::ThreadId`
/// as a `NonZero<u64>`. It relies on the current internal layout of `ThreadId` (as of
/// the Rust version in use), which is **not** guaranteed by the standard library and
/// may change in future compiler/std releases. If the layout changes, this code could
/// produce wrong values or, in theory, undefined behavior.
///
/// We use this approach because `ThreadId::as_u64()` is not yet stable on the stable
/// channel, and we do not want to depend on a nightly toolchain.
///
/// **Future replacement:** When `ThreadId::as_u64()` is stabilized, replace this
/// implementation with a direct call to `thread::current().id().as_u64()` and remove
/// the `unsafe` block and the `MyThreadId` helper. Track stabilization at
/// <https://github.com/rust-lang/rust/issues/67939>.
fn current_thread_id() -> u64 {
    struct MyThreadId(NonZero<u64>);

    // ThreadId::as_u64 is not stable yet, and we do not want to use nightly
    // toolchains, so we use some unsafe tricks to get the numeric thread ID.
    debug_assert_eq!(
        mem::size_of::<MyThreadId>(),
        mem::size_of::<thread::ThreadId>(),
        "ThreadId and MyThreadId must have the same size"
    );

    unsafe {
        mem::transmute::<ThreadId, MyThreadId>(thread::current().id())
            .0
            .into()
    }
}

/// Initializes the global logger with a default `info` filter.
///
/// The output format includes local timestamp, log level, thread/task IDs,
/// module path, and the formatted log arguments.
pub fn init_logger() {
    let env = Env::new().default_filter_or("info");

    env_logger::Builder::from_env(env)
        .format(|buf, record| {
            let style = buf.default_level_style(record.level());

            let time = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let level = record.level();
            let thread_id = current_thread_id();
            let module = record.module_path().unwrap_or("unknown");
            let args = record.args();

            match tokio::task::try_id() {
                Some(task_id) => writeln!(
                    buf,
                    "{time} [{style}{level:5}{style:#} {thread_id}:{task_id} {module}] {style}{args}{style:#}"
                ),
                None =>  writeln!(
                    buf,
                    "{time} [{style}{level:5}{style:#} {thread_id}:s {module}] {style}{args}{style:#}"
                )
            }
        })
        .init();
}
