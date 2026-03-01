//! Convenience helpers to unwrap `Result`/`Option` with process exit on error.

#[allow(dead_code)]
/// Extension trait that unwraps values or terminates the process with code 1.
pub trait UnwrapOrExit<T, E>: Sized {
    /// Unwraps the value or exits without extra error handling.
    fn unwrap_or_exit(self) -> T {
        Self::unwrap_or_exit_with(self, |_| ())
    }
    /// Unwraps the value, invoking `f` with the error payload before exiting.
    fn unwrap_or_exit_with<F: FnOnce(E)>(self, f: F) -> T;
}

impl<T, E> UnwrapOrExit<T, E> for Result<T, E> {
    fn unwrap_or_exit_with<F: FnOnce(E)>(self, f: F) -> T {
        match self {
            Ok(value) => value,
            Err(e) => {
                f(e);
                std::process::exit(1);
            }
        }
    }
}

impl<T> UnwrapOrExit<T, ()> for Option<T> {
    fn unwrap_or_exit_with<F: FnOnce(())>(self, f: F) -> T {
        match self {
            Some(value) => value,
            None => {
                f(());
                std::process::exit(1);
            }
        }
    }
}
