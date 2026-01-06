pub const REPOSITORY_URL: &str = "https://github.com/homomorphist/am-osx-status";
pub const REVERSE_DNS_IDENTIFIER: &str = get_reverse_dns_identifier!();

#[macro_export]
macro_rules! get_reverse_dns_identifier { () => { "network.goop.am-osx-status" }; }
pub use get_reverse_dns_identifier;

use std::sync::LazyLock;

/// User home directory.
pub static HOME: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
    #[allow(deprecated)] // This binary is MacOS-exclusive; this function only has unexpected behavior on Windows.
    std::env::home_dir().expect("no home directory env detected")
});

pub static APPLICATION_SUPPORT_FOLDER: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
    crate::util::HOME.join("Library/Application Support/am-osx-status")
});

pub static OWN_PID: LazyLock<libc::pid_t> = LazyLock::new(|| {
    unsafe { libc::getpid() }
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

pub fn get_installed_physical_memory() -> Option<u64> {
    unsafe {
        let mut size: u64 = 0;
        let mut size_len = std::mem::size_of::<u64>();
        let ret = libc::sysctlbyname(
            c"hw.memsize".as_ptr().cast(),
            (&raw mut size).cast(),
            &raw mut size_len,
            std::ptr::null_mut(),
            0,
        );
        if ret == 0 {
            Some(size)
        } else {
            let error = std::io::Error::last_os_error();
            tracing::error!(%error, "failed to get installed physical memory via sysctlbyname");
            None
        }
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
