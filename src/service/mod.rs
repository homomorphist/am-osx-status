use std::{ffi::OsString, str::FromStr};
use service_manager::*;
use sysinfo::{ProcessRefreshKind, RefreshKind, System};

pub mod ipc;


#[derive(thiserror::Error, Debug)]
pub enum ServiceStartFailure {
    #[error("process is already running")]
    ProcessAlreadyRunning,
    #[error("unknown io error ({0})")]
    IoFailure(#[from] std::io::Error),
}


#[derive(thiserror::Error, Debug)]
pub enum ServiceStopFailure {
    #[error("service not enabled")]
    NotEnabled,
    #[error("unknown io error ({0})")]
    IoFailure(#[from] std::io::Error),
}


#[derive(thiserror::Error, Debug)]
pub enum ServiceRestartFailure {
    #[error("{0}")]
    Start(#[from] ServiceStartFailure),
    #[error("{0}")]
    Stop(#[from] ServiceStopFailure)
}


pub struct ServiceController {
    label: ServiceLabel,
    system: sysinfo::System,
    manager: LaunchdServiceManager
}
impl ServiceController {
    pub fn new() -> Self {
        Self {
            label: ServiceLabel::from_str(clap::crate_name!()).unwrap(),
            system: System::new_with_specifics(RefreshKind::new().with_processes(ProcessRefreshKind::new())),
            manager: LaunchdServiceManager::user(),
        }
    }

    fn get_processes(&self) -> impl Iterator<Item = &sysinfo::Process> {
        self.system
            .processes_by_exact_name(std::ffi::OsStr::new(clap::crate_name!()))
            .filter(|process| process.pid().as_u32() != *crate::util::OWN_PID as u32)
    }

    pub fn is_program_active(&self) -> bool {
        self.get_processes().next().is_some()
    }

    pub fn start(&self, config: impl Into<OsString>, force: bool) -> Result<(), ServiceStartFailure> {
        if !force && self.get_processes().next().is_some() {
            return Err(ServiceStartFailure::ProcessAlreadyRunning)
        }

        self.manager.install(ServiceInstallCtx {
            label: self.label.clone(),
            program: std::env::current_exe().expect("cannot get own executable path"),
            args: vec![
                OsString::from("--ran-as-service"),
                OsString::from("--config"),
                config.into(),
                OsString::from("start"),
            ],
            contents: None,
            username: None,
            working_directory: None,
            environment: None,
            autostart: true, 
        })?;

        self.manager.start(ServiceStartCtx {
            label: self.label.clone(),
        })?;

        Ok(())
    }
    pub fn stop(&self) -> Result<(), ServiceStopFailure> {
        if let Err(error) = self.manager.uninstall(ServiceUninstallCtx { label: self.label.clone() }) {
            if error.kind() == std::io::ErrorKind::NotFound {
                return Err(ServiceStopFailure::NotEnabled);
            }
            return Err(ServiceStopFailure::IoFailure(error))
        };
        Ok(())
    }
    pub fn restart(&self, config: impl Into<OsString>) -> Result<(), ServiceRestartFailure> {
        self.stop().map_err(ServiceRestartFailure::Stop)?;
        self.start(config, false).map_err(ServiceRestartFailure::Start)?;
        Ok(())
    }
}
