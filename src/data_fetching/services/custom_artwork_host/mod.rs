use crate::subscribers::DispatchableTrack;
use tokio::sync::Mutex;
use std::sync::Arc;


#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderedHostList(pub Vec<HostIdentity>);
impl Default for OrderedHostList {
    fn default() -> Self {
        Self(vec![
            #[cfg(feature = "catbox")] HostIdentity::Litterbox,
            #[cfg(feature = "catbox")] HostIdentity::Catbox,
        ])
    }
}

macro_rules! define_hosts {
    (
        (
            $enum_vis: vis enum $enum: ident,
            $instances_vis: vis struct $instances: ident,
            $configs_vis: vis struct $configs: ident
        ), [$(
            ($([?$feature: literal])? $variant: ident @ $mod: ident ($repr: literal) || $aliases: expr$(, $config: ident)?)
        ),*]
    ) => {
        $(
            $(#[cfg(feature = $feature)])?
            pub mod $mod;
        )*

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $enum_vis enum $enum {
            $(
                $(#[cfg(feature = $feature)])?
                $variant,
            )*
        }
        impl $enum {
            pub fn all() -> &'static [Self] {
                &[
                    $(
                        $(#[cfg(feature = $feature)])?
                        Self::$variant,
                    )*
                ]
            }
            pub fn aliases(&self) -> &'static [&'static str] {
                match self {
                    $(
                        $(#[cfg(feature = $feature)])?
                        Self::$variant => &$aliases,
                    )*
                    _ => unreachable!("unrecognized custom artwork host")
                }
            }
            pub fn to_str(self) -> &'static str {
                match self {
                    $(
                        $(#[cfg(feature = $feature)])?
                        Self::$variant => $repr,
                    )*
                    _ => unreachable!("unrecognized custom artwork host")
                }
            }
            pub fn from_str(input: &str) -> Option<Self> {
                let input = input.trim();
                for host in Self::all() {
                    if host.to_str().eq_ignore_ascii_case(input) {
                        return Some(*host);
                    }
                    for alias in host.aliases() {
                        if alias.eq_ignore_ascii_case(input) {
                            return Some(*host);
                        }
                    }
                }
                None
            }
        }
        impl serde::Serialize for $enum {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(self.to_str())
            }
        }
        impl<'de> serde::Deserialize<'de> for $enum {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                Self::from_str(&s).ok_or_else(|| serde::de::Error::custom
                    (format!("invalid host identity: {}", s)))
            }
        }

        #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
        $configs_vis struct $configs {
            pub order: OrderedHostList,

            $(
                #[serde(default, skip_serializing_if = "Option::is_none")]
                $(#[cfg(feature = $feature)])?
                $mod: Option<Arc<<$mod::Host as CustomArtworkHostMetadata>::Config>>
            ),*
        }

        #[derive(Debug)]
        $instances_vis struct $instances {
            $(
                $(#[cfg(feature = $feature)])?
                $mod: Option<Mutex<Box<dyn CustomArtworkHost>>>
            ),*
        }
        impl $instances {
            pub fn none() -> Self {
                Self {
                    $(
                        $(#[cfg(feature = $feature)])?
                        $mod: None,
                    )*
                }
            }
            pub async fn new(configs: &$configs) -> Self {
                type Entry<'a> = (&'a HostIdentity, tokio::task::JoinHandle<Box<dyn CustomArtworkHost>>);
                let order = &configs.order.0;
                let mut handles = Vec::<Entry<'_>>::with_capacity(order.len());

                for identity in order { 
                    match identity {
                        $(
                            $(#[cfg(feature = $feature)])?
                            $enum::$variant => {
                                let config = configs.$mod.clone().unwrap_or_else(|| Arc::new({
                                    <$mod::Host as CustomArtworkHostMetadata>::Config::default()
                                }));
                                handles.push((identity, tokio::spawn(async move {
                                    Box::new($mod::Host::new(&config).await) as Box<dyn CustomArtworkHost>
                                })))
                            },
                        )*
                        _ => unreachable!("unrecognized custom artwork host")
                    }
                }

                let mut instances = Self::none();
                for (identity, handle) in handles {
                    match identity {
                        $(
                            $(#[cfg(feature = $feature)])?
                            $enum::$variant => {
                                let host = handle.await.expect("failed to initialize custom artwork host");
                                instances.$mod = Some(Mutex::new(host));
                            },
                        )*
                        _ => unreachable!("unrecognized custom artwork host")
                    }
                }
                instances
            }
            pub async fn get(&self, identity: $enum) -> Option<tokio::sync::MutexGuard<'_, Box<dyn CustomArtworkHost>>> {
                match identity {
                    $(
                        $(#[cfg(feature = $feature)])?
                        $enum::$variant => if let Some(host) = &self.$mod {
                            Some(host.lock().await)
                        } else {
                            None
                        },
                    )*
                }
            }
        }
    };
}

define_hosts!(
    (
        pub enum HostIdentity,
        pub struct Hosts,
        pub struct HostConfigurations
    ), [
        ([?"catbox"] Litterbox @ litterbox ("litterbox") || []),
        ([?"catbox"] Catbox @ catbox ("catbox") || [])
    ]
);

#[derive(thiserror::Error, Debug)]
pub enum UploadError {
    #[error("an unknown error occurred while uploading the custom track artwork")]
    UnknownError,
    #[error("sqlx error: {0}")]
    SqlxError(#[from] sqlx::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum RetrievalError {
    #[error("an unknown error occurred while retrieving the custom track artwork url")]
    UnknownError,
}


#[derive(thiserror::Error, Debug)]
pub enum CustomArtworkHostError {
    #[error("{0}")]
    UploadError(#[from] UploadError),
    #[error("{0}")]
    RetrievalError(#[from] RetrievalError)
}

#[async_trait::async_trait]
pub trait CustomArtworkHost: core::fmt::Debug + Send {
    async fn new(config: &<Self as CustomArtworkHostMetadata>::Config) -> Self where Self: Sized + CustomArtworkHostMetadata;
    async fn upload(&mut self, pool: &sqlx::SqlitePool, track: &DispatchableTrack, path: &str) -> Result<crate::store::entities::CustomArtworkUrl, UploadError>;
}
pub trait CustomArtworkHostMetadata {
    const IDENTITY: HostIdentity;
    type Config: serde::Serialize + serde::de::DeserializeOwned + Default;
}
