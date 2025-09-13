use std::sync::LazyLock;

use crate::util::OWN_PID;

pub static LOCKFILE_PATH: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
    crate::util::APPLICATION_SUPPORT_FOLDER.join("last-active.pid")
});

fn is_process_running(pid: libc::pid_t) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

pub struct ActiveProcessLockfile;
impl ActiveProcessLockfile {
    /// Returns the stored PID, which may not necessarily still be running.
    async fn read() -> Option<libc::pid_t>  {
        match tokio::fs::read_to_string(&*LOCKFILE_PATH).await {
            Ok(contents) => {
                match contents.trim().parse::<libc::pid_t>() {
                    Ok(pid) => Some(pid),
                    Err(err) => {
                        tracing::error!("failed to parse pid from lockfile: {}", err);
                        None
                    },
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => {
                tracing::error!("failed to read lockfile: {}", err);
                None
            },
        }
    }

    /// Returns the stored PID if it is still running.
    pub async fn get() -> Option<libc::pid_t> {
        Self::read().await.filter(|&pid| is_process_running(pid))
    }

    pub async fn write() -> Result<(), std::io::Error> {
        tokio::fs::write(&*LOCKFILE_PATH, OWN_PID.to_string()).await
    }

    pub async fn clear() -> Result<(), std::io::Error> {
        tokio::fs::remove_file(&*LOCKFILE_PATH).await
    }
}
