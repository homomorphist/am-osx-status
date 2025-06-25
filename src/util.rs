pub const REPOSITORY_URL: &str = "https://github.com/homomorphist/am-osx-status";

use std::sync::LazyLock;
/// User home directory.
pub static HOME: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
    #[allow(deprecated)] // This binary is MacOS-exclusive; this function only has unexpected behavior on Windows.
    std::env::home_dir().expect("no home directory env detected")
});

pub static OWN_PID: LazyLock<sysinfo::Pid> = LazyLock::new(|| {
    sysinfo::get_current_pid().expect("unsupported platform")
});

pub async fn get_macos_version() -> Option<String> {
    use tokio::process::Command;

    let output = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .await
        .expect("failed to execute sw_vers command");

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        tracing::error!("Failed to get macOS version: {}", String::from_utf8_lossy(&output.stderr));
        None
    }
}

macro_rules! ferror {
    ($($t: tt)*) => {
        {
            eprintln!($($t)*);
            std::process::exit(1)
        }
    }
}

pub(crate) use ferror;
