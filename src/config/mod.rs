use serde::{Deserialize, Serialize};

use crate::{util::{ferror, HOME}};

mod wizard;
mod file;
pub use file::ConfigPathChoice;

fn is_true(v: bool) {
    v
}

enum PathChoice<'a> {
    Explicit(&'a str),
    Environmental
    

}


#[derive(Serialize, Deserialize)]
pub struct Config<'a> {
    #[serde(skip)]
    pub path: ConfigPathChoice<'a>,
    pub backends: ConfigurableBackends,


    #[serde(skip)]
    file_watcher: Option<kqueue::Watcher>,
    watch_config_file: bool,
    #[serde(skip_serializing_if = "crate::service::ipc::socket_path::is_default", default = "crate::service::ipc::socket_path::clone_default")]
    pub ipc_socket_path: std::path::PathBuf,
}
impl Default for Config<'_> {
    fn default() -> Self {
        Self {
            ipc_socket_path: crate::service::ipc::socket_path::DEFAULT.clone(),
            backends: ConfigurableBackends::default(),
            path: ConfigPathChoice::default(),
            watch_config_file: false,
            file_watcher: None,
        }
    }
}
impl<'a> Config<'a> {
    pub async fn get(args: &'a crate::cli::Cli) -> Self {
        let path_override = args.config_file_path.as_deref();
        let path = ConfigPathChoice::new(path_override);
    
        match std::fs::read(&path) {
            Err(error) => {
                use std::io::ErrorKind;
                match error.kind() {
                    ErrorKind::PermissionDenied => ferror!("cannot read configuration file: permission denied accessing path {}", path.to_string_lossy()),
                    ErrorKind::NotFound => {
                        if path_override.is_some() {
                            ferror!("configuration file not found")
                        } else if !wizard::io::prompt_bool("Would you like to use the interactive configuration helper?\nAll settings can be changed at a later time.") {
                            Self::default()
                        } else {
                            let instance = Self::create_with_wizard(path).await;
                            instance.save_to_disk().await;
                            println!("Saved configuration file to {}", instance.path.as_path().to_string_lossy());
                            instance
                        }
                    }
                    _ => ferror!("cannot read configuration file: {}", error)
                }
            },
            Ok(data) => {
                let data = String::from_utf8_lossy(&data[..]);
                toml::from_str(&data).expect("cannot deserialize config")
            }
       }
    }

    // pub async fn get_fd(&self) -> Option<std::os::fd> {
    //     let file = tokio::fs::File::options().open(self.path).await.unwrap();
    //     // file.
    // }

    pub async fn update_on_file_change(&mut self, enable: bool) {
        if self.watch_config_file == enable { return }
        self.watch_config_file = enable;
        if enable {
            unimplemented!()
        } else {
            drop(self.file_watcher.take())
        }
    }

    pub async fn edit_with_wizard(&mut self)  {
        self.backends.discord = wizard::io::prompt_bool("Enable Discord Rich Presence?");
        self.backends.lastfm =  wizard::io::prompt_lastfm().await;
        self.backends.listenbrainz = wizard::io::prompt_listenbrainz().await;
    }

    /// NOTE: Will not write to the provided path unless [`Self::save_to_disk`] is called.
    pub async fn create_with_wizard(path: ConfigPathChoice<'a>) -> Self {
        let mut config: Self = Default::default();
        config.edit_with_wizard();
        config
    }
    //     };

    //     let contents = toml::ser::to_string(&instance).expect("could not serialize constructed configuration");
    //     let path = instance.path.as_path();
    //     std::fs::create_dir_all(path.parent().expect("cannot write to root...?")).expect("could not create configuration directory");
    //     std::fs::write(&instance.path, contents.as_bytes()).expect("could not write configuration");
    //     println!(r#"Saved configuration file to {}"#, path.to_string_lossy());

    //     instance
    // }
    // }

    pub fn serialize(&self) -> String {
        toml::ser::to_string(self).expect("could not serialize constructed configuration")
    }

    pub async fn save_to_disk(&self) {
        let path = self.path.as_path();
        tokio::fs::create_dir_all(path.parent().expect("cannot write to root...?")).await.expect("could not create configuration directory");
        tokio::fs::write(&path, self.serialize().as_bytes()).await.expect("could not write configuration");
    }
}


#[derive(Serialize, Deserialize)]
pub struct ConfigurableBackends {
    #[cfg(feature = "discord")]
    pub discord: bool,
    #[cfg(feature = "lastfm")]
    pub lastfm: Option<crate::status_backend::lastfm::Config>,
    #[cfg(feature = "listenbrainz")]
    pub listenbrainz: Option<crate::status_backend::listenbrainz::Config>
}
impl Default for ConfigurableBackends {
    fn default() -> Self {
        Self {
            #[cfg(feature = "discord")]
            discord: true,
            #[cfg(feature = "lastfm")]
            lastfm: None,
            #[cfg(feature = "listenbrainz")]
            listenbrainz: None,
        }
    }
}
