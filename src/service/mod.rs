use std::{ffi::OsString, iter::Product, sync::LazyLock};

use lockfile::ActiveProcessLockfile;
use crate::util::{ferror, REVERSE_DNS_IDENTIFIER};

pub mod ipc;
pub mod lockfile;

const JOB_DEFINITION_TEMPLATE: &str = include_str!("definition.plist.template");

static JOB_DEFINITION_LOCATION: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
    crate::util::HOME.join(concat!("Library/LaunchAgents/", crate::util::get_reverse_dns_identifier!(), ".plist"))
});

static USER_ID: LazyLock<libc::uid_t> = LazyLock::new(|| unsafe { libc::getuid() });
static DOMAIN_TARGET: LazyLock<String> = LazyLock::new(|| format!("gui/{}", *USER_ID));

/// User IO-based service interface
pub struct ServiceController;
impl ServiceController {
    fn agent() -> LaunchAgent<'static> {
        LaunchAgent::new(&JOB_DEFINITION_LOCATION)
    }

    fn render_job_definition(config_path: impl AsRef<std::path::Path>) -> String {
        JOB_DEFINITION_TEMPLATE
            .replace("{{ reverse_dns_identifier }}", REVERSE_DNS_IDENTIFIER)
            .replace("{{ app_path }}", std::env::current_exe().expect("cannot get own executable path").to_string_lossy().as_ref())
            .replace("{{ config_path }}", config_path.as_ref().to_string_lossy().as_ref())
    }

    pub fn get_definition_path() -> &'static std::path::Path {
        &JOB_DEFINITION_LOCATION
    }

    async fn write_job_definition(config_path: impl AsRef<std::path::Path>) -> Result<(), std::io::Error> {
        LaunchAgent::write_definition(Self::get_definition_path(), Self::render_job_definition(config_path)).await
    }

    async fn delete_job_definition() -> Result<bool, std::io::Error> {
        Self::agent().remove_definition().await
    }

    pub async fn start(config_path: impl AsRef<std::path::Path>, log: bool) {
        if let Err(err) = Self::write_job_definition(&config_path).await {
            ferror!("Failed to write job definition file: {}", err);
        }

        match Self::agent().register().await {
            Err(err) => ferror!("Failed to register service: {}", err),
            Ok(was_registered) => {
                if log {
                    if was_registered {
                        println!("Service registered and started!");
                    } else {
                        println!("Service already registered; not registering again.");
                    }
                }
            }

        }
    }

    pub async fn restart(config_path: impl AsRef<std::path::Path>) {
        Self::stop(false).await;
        Self::start(config_path, false).await;
        println!("Service restarted!");
    }

    pub async fn stop(log: bool) {
        let pid =  Self::agent().get_pid().await;
        match Self::agent().unregister(false).await {
            Ok(was_registered) => {
                if log {
                    if was_registered {
                        print!("Service");
                        if let Some(pid) = pid {
                            print!(" (pid {pid}) stopped and");
                        }
                        println!(" temporarily unregistered! It will start again on the next login, or when started again manually.");
                        println!("(If you want to fully remove the service, use `am-osx-status service remove`.)");
                    } else {
                        println!("Service wasn't registered to begin with.");
                    }
                }
            },
            Err(err) => {
                ferror!("Failed to unregister service: {}", err);
            }
        }
    }

    pub async fn remove() {
        let pid = Self::agent().get_pid().await;
        match Self::agent().unregister(true).await {
            Ok(was_registered) => {
                if was_registered {
                    print!("Service");
                    if let Some(pid) = pid {
                        print!(" (pid {pid}) stopped and");
                    }
                    println!(" unregistered! To add it again, use `am-osx-status service start`.");
                } else {
                    println!("Service wasn't registered to begin with.");
                }
            },
            Err(err) => {
                ferror!("Failed to unregister service: {}", err);
            }
        }
    }

    pub async fn is_running() -> bool {
        Self::agent().is_running().await.unwrap_or(false)
    }

    pub async fn is_loaded() -> bool {
        Self::agent().is_loaded().await.unwrap_or(false)
    }

    pub async fn is_defined() -> Result<bool, std::io::Error> {
        match tokio::fs::metadata(&*JOB_DEFINITION_LOCATION).await {
            Ok(meta) => Ok(meta.is_file()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err),
        }
    }

    pub async fn pid() -> Option<libc::pid_t> {
        Self::agent().get_pid().await
    }
}

#[derive(Debug)]
struct LaunchctlErrorOutput {
    status: std::process::ExitStatus,
    stderr: String,
}
impl std::fmt::Display for LaunchctlErrorOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "launchctl exited with status {}: {}", self.status, self.stderr)
    }
}
impl core::error::Error for LaunchctlErrorOutput {}


#[derive(thiserror::Error, Debug)]
enum UnregistrationFailure {
    #[error("couldn't delete definition file: {0}")]
    DefinitionRemoval(#[from] std::io::Error),
    #[error("{0}")]
    Launchctl(#[from] LaunchctlErrorOutput),    
}

struct LaunchAgent<'a> {
    path: &'a std::path::Path,
}
impl<'a> LaunchAgent<'a> {
    pub fn new(path: &'a std::path::Path) -> Self {
        Self { path }
    }

    /// Create a new launch agent definition at the given path with the given body.
    /// This agent will be automatically be registered on login, which may start it immediately depending on its configuration.
    /// To manually register it immediately, call `register()` on the returned object.
    pub async fn new_with_definition(path: &'a std::path::Path, body: impl AsRef<[u8]>) -> Result<Self, std::io::Error> {
        Self::write_definition(path, body).await?;
        Ok(Self { path })
    }

    async fn write_definition(path: impl AsRef<std::path::Path>, body: impl AsRef<[u8]>) -> Result<(), std::io::Error> {
        use tokio::io::{AsyncWrite, AsyncWriteExt};
        use tokio::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;
        let mut opts = OpenOptions::new();
        opts.create(true).write(true).truncate(true); // will overwrite if exists
        opts.mode(0o644); // rw-r--r--
        let mut file = opts.open(path.as_ref()).await?;
        file.write_all(body.as_ref()).await?;
        file.sync_all().await
    }

    async fn execute_launchctl_command<I, S>(&self, args: I) -> Result<String, LaunchctlErrorOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let output = tokio::process::Command::new("launchctl")
            .args(args)
            .output()
            .await
            .expect("failed to execute launchctl");
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(LaunchctlErrorOutput {
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            })
        }
    }

    /// Returns the PID of the running service.
    pub async fn get_pid(&self) -> Option<libc::pid_t> {
        let output = self.execute_launchctl_command(&["list", REVERSE_DNS_IDENTIFIER]).await.ok()?;
        for line in output.lines() {
            const PREFIX: &str = "\"PID\" = ";
            if let Some(pid) = line.strip_prefix(PREFIX) {
                let pid = pid
                    .trim_end_matches(';')
                    .trim()
                    .parse::<libc::pid_t>()
                    .expect("cannot parse pid");
            
                return Some(pid);
            }
        }
        None
    }

    /// Whether the launch agent is currently running.
    pub async fn is_running(&self) -> Result<bool, LaunchctlErrorOutput> {
        match self.execute_launchctl_command(&["list", REVERSE_DNS_IDENTIFIER]).await {
            Ok(_) => Ok(true),
            Err(err) if err.status.code() == Some(113) => Ok(false),
            Err(err) => Err(err)
        }
    }

    /// Whether the launch agent is loaded with launchd– this is distinct from whether the process is running.
    pub async fn is_loaded(&self) -> Result<bool, LaunchctlErrorOutput> {
        match self.execute_launchctl_command(&["list", REVERSE_DNS_IDENTIFIER]).await {
            Ok(_) => Ok(true),
            Err(err) if err.status.code() == Some(113) => Ok(false),
            Err(err) => Err(err)
        }
    }

    /// Register the launch agent with launchd and returns whether it was not already registered.
    /// This automatically happens on login, but this can be used to manually register it until the next logout.
    /// This will automatically start the service if `RunAtLoad` or `KeepAlive` conditions are met and the service isn't disabled.
    pub async fn register(&self) -> Result<bool, LaunchctlErrorOutput> {
        if self.is_loaded().await? {
            return Ok(false);
        }

        self.execute_launchctl_command(&[
            "bootstrap",
            DOMAIN_TARGET.as_str(),
            self.path.to_string_lossy().as_ref()
        ]).await?;

        Ok(true)
    }

    /// Unregister the launch agent from launchd and returns whether it was registered to begin with.
    /// This will stop the service if it is running.
    /// The service will be re-registered on next login if the plist file is kept.
    pub async fn unregister(&self, remove_definition: bool) -> Result<bool, UnregistrationFailure> {
        if !self.is_loaded().await? {
            return Ok(false);
        }

        self.execute_launchctl_command(&[
            "bootout",
            DOMAIN_TARGET.as_str(),
            self.path.to_string_lossy().as_ref()
        ]).await?;

        if remove_definition {
            self.remove_definition().await?;
        }

        Ok(true)
    }

    async fn remove_definition(&self) -> Result<bool, std::io::Error> {
        match tokio::fs::remove_file(self.path).await {
            Ok(()) => Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err),
        }
    }

    /// Stop the service if it is running.
    /// Note that if the service is configured to `KeepAlive`, it will be restarted immediately.
    pub async fn stop(&self) -> Result<(), LaunchctlErrorOutput> {
        self.execute_launchctl_command(&[
            "stop",
            REVERSE_DNS_IDENTIFIER
        ]).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigPathChoice;

    static CONFIG_PATH: LazyLock<ConfigPathChoice> = LazyLock::new(|| ConfigPathChoice::new(None));

    // ideally we'd use temp but i'll do later ig idk
    fn get_config_path() -> &'static std::path::Path {
        CONFIG_PATH.as_path()
    }
   
    #[tokio::test]
    #[ignore = "has race condition with other tests; will override real service too"]
    async fn double_register() {
        let agent = ServiceController::agent();
        assert!(agent.unregister(true).await.is_ok());
        ServiceController::write_job_definition(get_config_path()).await.expect("failed to write service definition");
        assert!( agent.register().await.expect("failed to register service")); //  true if it was not already registered
        assert!(!agent.register().await.expect("failed to register service")); // false if it was     already registered
        assert!(agent.unregister(true).await.is_ok());
    }

    #[tokio::test]
    #[ignore = "has race condition with other tests; will override real service too"]
    async fn double_unregister() {
        let agent = ServiceController::agent();
        ServiceController::write_job_definition(get_config_path()).await.expect("failed to write service definition");
        agent.register().await.expect("failed to register service");
        assert!( agent.unregister(true).await.expect("failed to unregister service")); //  true if it was     registered
        assert!(!agent.unregister(true).await.expect("failed to unregister service")); // false if it was not registered
    }
}
