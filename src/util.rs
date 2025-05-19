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
