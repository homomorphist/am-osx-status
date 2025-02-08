use std::{ffi::OsString, str::FromStr};
use service_manager::*;
use sysinfo::{ProcessRefreshKind, RefreshKind, System};

pub mod ipc;


#[derive(thiserror::Error, Debug)]
pub enum ServiceStartFailure {
    #[error("process is already running")]
    ProcessAlreadyRunning,
    #[error("could not start the service")]
    ServiceFailure(#[from] std::io::Error),
}


#[derive(thiserror::Error, Debug)]
pub enum ServiceStopFailure {
    #[error("could not kill process")]
    CannotKill,
    #[error("could not stop the service")]
    ServiceFailure(#[from] std::io::Error),
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

    pub fn start(&self, force: bool) -> Result<(), ServiceStartFailure> {
        if !force && self.get_processes().next().is_some() {
            return Err(ServiceStartFailure::ProcessAlreadyRunning)
        }

        self.manager.install(ServiceInstallCtx {
            label: self.label.clone(),
            program: std::env::current_exe().expect("cannot get own executable path"),
            args: vec![OsString::from("start")],
            contents: None,
            username: None,
            working_directory: None,
            environment: None,
            autostart: true, 
        })?;

        Ok(())
    }
    /// Returns the amount of processes killed.
    pub fn stop(&self) -> Result<u16, ServiceStopFailure> {
        self.manager.uninstall(ServiceUninstallCtx { label: self.label.clone() })?;

        let mut amount_killed: u16 = 0;
        for process in self.get_processes() {
            if !process.kill() {
                return Err(ServiceStopFailure::CannotKill)
            }
            amount_killed += 1;
        }

        Ok(amount_killed)
    }
    pub fn restart(&self) -> Result<(), ServiceRestartFailure> {
        self.stop().map_err(ServiceRestartFailure::Stop).unwrap();
        self.start(false).map_err(ServiceRestartFailure::Start).unwrap();
        Ok(())
    }
}
