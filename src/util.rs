macro_rules! fallback_to_default_and_log_error {
    ($result: expr) => {
        {
            // needed to get T::default
            fn fallback_to_default_and_log_error<T: Default, E: core::fmt::Debug>(result: Result<T, E>) -> T {
                match result {
                    Ok(value) => value,
                    Err(error) => {
                        tracing::error!(?error, "failed to get value; using default");
                        T::default()
                    }
                }
            }
            fallback_to_default_and_log_error($result)
        }
    };
}

pub(crate) use fallback_to_default_and_log_error;

pub const REPOSITORY_URL: &str = "https://github.com/Agapurnis/am-osx-status";

use std::sync::LazyLock;
/// User home directory.
pub static HOME: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
    #[allow(deprecated)] // This binary is MacOS-exclusive; this function only has unexpected behavior on Windows.
    std::env::home_dir().expect("no home directory env detected")
});

pub static OWN_PID: LazyLock<libc::pid_t> = LazyLock::new(|| unsafe { libc::getpid() });

macro_rules! ferror {
    ($($t: tt)*) => {
        {
            eprintln!($($t)*);
            std::process::exit(1)
        }
    }
}

pub(crate) use ferror;
