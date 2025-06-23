use std::sync::Arc;
use maybe_owned_string::MaybeOwnedString;
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};

use crate::data_fetching::components::ComponentSolicitation;

use error::dispatch::DispatchError;
pub mod error {
    pub use dispatch::DispatchError;
    pub mod dispatch {
        /// How the program should respond to an error being encountered.
        #[derive(Debug)]
        pub enum Recovery {
            /// Will cause the program (not just the backend) to exit with a fatal error.
            /// This will always be logged at the `ERROR` level.
            CriticallyFail,
            /// Don't interrupt program behavior, log the error and continue, potentially recording the input data for retrial at a later time.
            /// If this method is called multiple times per track, it may be wise to use [`Recovery::Skip`] with a [`SkipPredicate`] instead.
            Continue(RecoveryAttributes),
            /// Don't use this method again until the predicate is met.
            /// If this method isn't one which is called multiple times per track, it is equivalent to [`Recovery::Continue`].
            /// Predicate is not tracked across sessions. If the program is restarted, the method will be called again as normal.
            Skip {
                /// Shared attributes for this recovery method.
                attributes: RecoveryAttributes, 
                /// The condition which must be met before the method can be called again.
                /// If `defer` is true, attempts to call the method will be stored and attempted in bulk once the predicate is met (or the program is restarted).
                until: SkipPredicate 
            },
        }
        impl Recovery {
            /// Returns the associated attributes, if present.
            pub const fn attributes(&self) -> Option<&RecoveryAttributes> {
                match self {
                    Recovery::Continue(attributes) => Some(attributes),
                    Recovery::Skip { attributes, .. } => Some(attributes),
                    _ => None
                }
            }

            /// Returns the log level, if present.
            pub fn log_level(&self) -> Option<tracing::Level> {
                self.attributes().and_then(|a| a.log).or({
                    if matches!(self, Recovery::CriticallyFail) {
                        Some(tracing::Level::ERROR)
                    } else { None }
                })
            }
            
            /// Returns whether or not more attempt(s) should be deferred.
            pub fn defer(&self) -> bool {
                self.attributes().map(|a| a.defer).unwrap_or_default()
            }
        }

        /// Attributes which can be applied to a recovery method.
        #[derive(Debug)]
        pub struct RecoveryAttributes {
            /// The level, if any, at which to log the error.
            /// If `None`, the error will not be logged.
            pub log: Option<tracing::Level>,
            /// Whether or not to attempt to store the data for the call(s) so that they can be tried again later.
            /// ## Example
            /// If you're [skipping](Recovery::Skip) until an authentication issue is fixed, you'd defer `listened` data to be submitted in bulk later once the issue is resolved.
            pub defer: bool,
        }


        /// A condition which must be met before the method can be called again.
        #[derive(Debug)]
        pub enum SkipPredicate {
            /// Skip the method until a new song is played.
            NextSong,
            /// Skip this method until the program is restarted.
            Restart,
        }

        use maybe_owned_string::MaybeOwnedString;

        pub use cause::Cause;

        use crate::{status_backend::DispatchableTrack, store::{entities::{DeferredTrack, Key}, MaybeStaticSqlError}};
        pub mod cause {
            use super::MaybeOwnedString;

            /// The request-related cause of a dispatch error.
            /// This occurs if the request (or its response) wasn't successfully processed because a [non-data](DataError) error was encountered.
            #[derive(thiserror::Error, Debug)]
            pub enum RequestError {
                /// The remote backend refused the request because of a lack of authorization.
                /// Contains an optional message with an elaboration as to why.
                #[error("unauthorized: {cause}", cause = .0.as_deref().unwrap_or("no reason given"))]
                Unauthorized(Option<MaybeOwnedString<'static>>),
                /// A response was received, but it indicated that the backend is currently unavailable.
                #[error("service unavailable")]
                Unavailable,
                /// Couldn't connect to the backend; likely because the user's network is offline.
                #[error("connection failure")]
                ConnectionFailure,
                /// The user's network is presumably online, but the backend is unreachable for one reason or another.
                #[error("network error: {0}")]
                NetworkError(reqwest::Error),
                /// Unable to deserialize the response from the backend.
                #[error("deserialization error: {0}")]
                DeserializationError(#[from] serde_json::Error),
            }
            impl From<reqwest::Error> for RequestError {
                fn from(error: reqwest::Error) -> Self {
                    if error.is_connect() {
                        RequestError::ConnectionFailure
                    } else {
                        RequestError::NetworkError(error)
                    }
                }
            }

            /// The data-related cause of a dispatch error.
            /// This occurs if the dispatch wasn't successfully processed because of an issue with the data being submitted.
            #[derive(thiserror::Error, Debug)]
            pub enum DataError {
                /// The current track is missing required data (i.e. a title, the artist name).
                /// Contains an elaboration on what data is missing.
                #[error("missing required data: {0}")]
                MissingRequired(MaybeOwnedString<'static>),
                /// Attempted to submit data which is invalid or out of range.
                /// Contains an elaboration on what data is invalid.
                #[error("invalid data: {0}")]
                Invalid(MaybeOwnedString<'static>),
            }

            /// The cause of a dispatch error.
            #[derive(thiserror::Error, Debug)]
            pub enum Cause {
                #[error("{0}")]
                Request(#[from] RequestError),
                #[error("{0}")]
                Data(#[from] DataError),
                /// Something went wrong concerning the [`Subscriber`](crate::status_backend::Subscriber) implementation itself.
                /// Contains an elaboration on what went wrong.
                #[error("internal error: {0}")]
                Internal(Box<dyn std::error::Error + Send + Sync>),
            }
            impl Cause {
                /// Add a recovery method to the cause and convert it into a full [`DispatchError`](super::DispatchError).
                pub fn with_recovery(self, recovery: super::Recovery) -> super::DispatchError {
                    super::DispatchError {
                        cause: self,
                        recovery
                    }
                }

                /// Create a new internal error with the specified message.
                pub fn internal(msg: impl Into<MaybeOwnedString<'static>>) -> Self {
                    #[derive(Debug)]
                    struct InternalError(MaybeOwnedString<'static>);
                    impl core::fmt::Display for InternalError {
                        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                            write!(f, "internal error: {}", self.0)
                        }
                    }
                    impl core::error::Error for InternalError {}

                    Cause::Internal(Box::new(InternalError(msg.into())))
                }
            }

            impl From<reqwest::Error> for Cause {
                fn from(error: reqwest::Error) -> Self {
                    Cause::Request(error.into())
                }
            }
            impl From<serde_json::Error> for Cause {
                fn from(error: serde_json::Error) -> Self {
                    Cause::Request(error.into())
                }
            }
        }

        /// An error that occurred as a result of a dispatch to a backend.
        #[derive(Debug)]
        pub struct DispatchError {
            /// The cause of the error.
            pub cause: Cause,
            /// How the program should respond to the error.
            pub recovery: Recovery,
        }
        impl std::error::Error for DispatchError {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match &self.cause {
                    Cause::Request(cause::RequestError::NetworkError(err)) => Some(err),
                    _ => None
                }
            }
        }
        impl core::fmt::Display for DispatchError {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, "{}", self.cause)
            }
        }
        impl DispatchError {
            /// Log the error with the specified context, if applicable.
            pub fn log(&self, backend: &'static str, event: impl crate::subscription::TypeIdentity) {
                if let Some(level) = self.recovery.log_level() {
                    // Uhm, so, we can't using `tracing::event` with a non-constant level, so...
                    macro_rules! bind {
                        ($(($level: ident, $macro: ident) $(,)?)*) => {
                            match level {
                                $(tracing::Level::$level => tracing::$macro!(backend, ?event, error = ?self, "dispatch error"),)*
                            }
                        };
                    }
                    bind! {
                        (ERROR, error),
                        (WARN, warn),
                        (INFO, info),
                        (DEBUG, debug),
                        (TRACE, trace),
                    }
                }
            }

            /// Panic if the error is fatal.
            fn handle_fatal(&self) {
                if matches!(self.recovery, Recovery::CriticallyFail) {
                    // more info would've been logged already by `log`
                    panic!("dispatch resulted in fatal error");
                }
            }

            /// Returns a tuple of the track ID and whether it was this operation which added the track was added to the database.
            /// (If the second element is false, the track was already in the database.)
            async fn add_to_deferred(&self, backend: &'static str, event: impl crate::subscription::TypeIdentity, track: &DispatchableTrack) -> Result<(Key<DeferredTrack>, bool), MaybeStaticSqlError> {
                use crate::store::entities::FromKey;
                Ok(match DeferredTrack::get_with_persistent_id(&track.persistent_id).await? {
                    Some(track) => (track.id, false),
                    None => (DeferredTrack::insert(track).await?, true)
                })
            }
            
            /// Log the error and panic if it is fatal.
            pub fn handle(&self, backend: &'static str, event: impl crate::subscription::TypeIdentity) {
                self.log(backend, event);
                self.handle_fatal();
            }

        }
        impl DispatchError { // constructors
            pub fn internal(error: Box<dyn std::error::Error + Send + Sync>, recovery: Recovery) -> Self {
                Self {
                    cause: Cause::Internal(error),
                    recovery
                }
            }

            pub fn internal_msg(msg: &'static str, skip: bool) -> Self {
                Self {
                    cause: Cause::internal(msg),
                    recovery: if skip {
                        Recovery::Skip {
                            until: SkipPredicate::Restart,
                            attributes: RecoveryAttributes {
                                log: Some(tracing::Level::ERROR),
                                defer: true,
                            }
                        }
                    } else {
                        Recovery::Continue(RecoveryAttributes {
                            log: Some(tracing::Level::ERROR),
                            defer: true
                        })
                    }
                }
            }

            pub const fn missing_required_data(data: &'static str) -> Self {
                Self {
                    cause: Cause::Data(cause::DataError::MissingRequired(MaybeOwnedString::Borrowed(data))),
                    recovery: Recovery::Skip {
                        until: SkipPredicate::NextSong,
                        attributes: RecoveryAttributes {
                            log: Some(tracing::Level::ERROR),
                            defer: false
                        }
                    }
                }
            }

            pub const fn invalid_data(data: &'static str) -> Self {
                Self {
                    cause: Cause::Data(cause::DataError::Invalid(MaybeOwnedString::Borrowed(data))),
                    recovery: Recovery::Continue(RecoveryAttributes {
                        log: Some(tracing::Level::ERROR),
                        defer: false
                    })
                }
            }

            pub const fn unauthorized(reason: Option<&'static str>) -> Self {
                Self {
                    cause: Cause::Request(cause::RequestError::Unauthorized({
                        if let Some(reason) = reason {
                            Some(MaybeOwnedString::Borrowed(reason))
                        } else { None }
                    })),
                    recovery: Recovery::Skip {
                        until: SkipPredicate::Restart,
                        attributes: RecoveryAttributes {
                            log: Some(tracing::Level::ERROR),
                            defer: true,
                        },
                    }
                }
            }
        }
        impl From<reqwest::Error> for DispatchError {
            fn from(error: reqwest::Error) -> Self {
                Self {
                    cause: error.into(),
                    recovery: Recovery::Continue(RecoveryAttributes {
                        log: Some(tracing::Level::ERROR),
                        defer: true
                    })
                }
            }
        }
        impl From<serde_json::Error> for DispatchError {
            fn from(error: serde_json::Error) -> Self {
                Self {
                    cause: error.into(),
                    recovery: Recovery::Continue(RecoveryAttributes {
                        log: Some(tracing::Level::ERROR),
                        defer: true
                    })
                }
            }
        }
    }
}


macro_rules! use_backends {
    ([ $(($name: ident, $ident: ident, $feature: literal, $id: literal)$(,)?)* ]) => {
        pub const MAX_ENABLED_BACKEND_COUNT: usize = {
            $(
                ({
                    #[cfg(feature = $feature)]
                    { 1 }
                    #[cfg(not(feature = $feature))]
                    { 0 }
                }) +
            )* 0
        };

        $(
            #[cfg(feature = $feature)]
            pub mod $name;
        )*

        #[derive(Debug, PartialEq, Eq, Clone, Copy)]
        pub enum BackendIdentity {
            $(
                #[cfg(feature = $feature)]
                $ident,
            )*
        }
        impl BackendIdentity {
            pub const fn get_name(&self) -> &'static str {
                match self {
                    $(
                        #[cfg(feature = $feature)]
                        Self::$ident => stringify!($ident),
                    )*
                }
            }
            pub const fn get_holey_index(&self) -> u16 {
                match self {
                    $(
                        #[cfg(feature = $feature)]
                        Self::$ident => $id,
                    )*
                }
            }
            pub const fn from_holey_index(index: u16) -> Option<Self> {
                match index {
                    $(
                        #[cfg(feature = $feature)]
                        $id => Some(Self::$ident),
                    )*
                    _ => None
                }
            }
        }

        
        #[derive(Debug, Default)]
        pub struct BackendMap<T> {
            $(
                #[cfg(feature = $feature)]
                pub $name: Option<T>,
            )*
        }
        impl<'a, T> BackendMap<T> {
            pub fn new() -> Self {
                Self {
                    $(
                        #[cfg(feature = $feature)]
                        $name: None,
                    )*
                }
            }

            pub fn iter(&'a self) -> iter::BackendMapIterator<'a, T> {
                self.into_iter()
            }

            pub fn take(&mut self, identity: BackendIdentity) -> Option<T> {
                match identity {
                    $(
                        #[cfg(feature = $feature)]
                        BackendIdentity::$ident => self.$name.take(),
                    )*
                }
            }
        }
        impl<T> core::ops::Index<BackendIdentity> for BackendMap<T> {
            type Output = Option<T>;
            fn index(&self, index: BackendIdentity) -> &Self::Output {
                match index {
                    $(
                        #[cfg(feature = $feature)]
                        BackendIdentity::$ident => &self.$name,
                    )*
                }
            }
        }
        impl<T> core::ops::IndexMut<BackendIdentity> for BackendMap<T> {
            fn index_mut(&mut self, index: BackendIdentity) -> &mut Self::Output {
                match index {
                    $(
                        #[cfg(feature = $feature)]
                        BackendIdentity::$ident => &mut self.$name,
                    )*
                }
            }
        }

        pub mod iter {
            use super::*;

            pub struct BackendMapIterator<'a, T> {
                inner: &'a BackendMap<T>,
                index: usize,
            }
            impl<'a, T> Iterator for BackendMapIterator<'a, T> {
                type Item = (BackendIdentity, &'a Option<T>);
                fn next(&mut self) -> Option<Self::Item> {                
                    while self.index < MAX_ENABLED_BACKEND_COUNT {
                        let index = self.index;
                        let identity = BackendIdentity::from_holey_index(index as u16);
                        self.index += 1;
                        if let Some(identity) = identity {
                            return Some((identity, &self.inner[identity]));
                        }
                    }
                    None
                }
            }
            impl<'a, T> IntoIterator for &'a BackendMap<T> {
                type Item = (BackendIdentity, &'a Option<T>);
                type IntoIter = iter::BackendMapIterator<'a, T>;
                fn into_iter(self) -> Self::IntoIter {
                    self.iter()
                }
            }
            
            pub struct BackendMapIntoIterator<T> {
                inner: BackendMap<T>,
                index: usize,
            }
            impl<T> IntoIterator for BackendMap<T> {
                type Item = (BackendIdentity, Option<T>);
                type IntoIter = BackendMapIntoIterator<T>;
                fn into_iter(self) -> Self::IntoIter {
                    BackendMapIntoIterator {
                        inner: self,
                        index: 0,
                    }
                }
            }
            impl<T> Iterator for BackendMapIntoIterator<T> {
            type Item = (BackendIdentity, Option<T>);
            fn next(&mut self) -> Option<Self::Item> {
                while self.index < MAX_ENABLED_BACKEND_COUNT {
                    let index = self.index;
                    let identity = BackendIdentity::from_holey_index(index as u16);
                    self.index += 1;
                    if let Some(identity) = identity {
                        return Some((identity, self.inner.take(identity)));
                    }
                }
                None
            }
        }
        }

        pub struct Backends {
            $(
                #[cfg(feature = $feature)]
                pub $name: Option<Arc<Mutex<$name::$ident>>>,
            )*
        }
        impl Backends {
            pub fn all(&self) -> Vec<Arc<Mutex<dyn Subscriber>>> {
                let mut backends: Vec<Arc<Mutex<dyn Subscriber>>> = Vec::with_capacity(MAX_ENABLED_BACKEND_COUNT);
        
                $(
                    #[cfg(feature = $feature)]
                    if let Some(backend) = self.$name.as_ref() {
                        backends.push(backend.clone());
                    }
                )*
        
                backends
            }
        }
        impl core::fmt::Debug for Backends {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                let mut set = f.debug_set();
                $(
                    #[cfg(feature = $feature)]
                    if let Some(backend) = &self.$name {
                        set.entry(backend);
                    }
                )*
                set.finish()
            }
        }
    };
}
use_backends!([
    (discord, DiscordPresence, "discord", 0),
    (lastfm, LastFM, "lastfm", 1),
    (listenbrainz, ListenBrainz, "listenbrainz", 2)
]);

impl<T, E> BackendMap<Result<T, E>> {
    fn into_errors_iter(self) -> impl Iterator<Item = (BackendIdentity, E)> {
        self.into_iter().filter_map(|(i, r)| r.and_then(|r| r.err()).map(|e| (i, e)))
    }
}

/// The minimum data required to dispatch a track to a backend.
/// This can be serialized and deserialized for bulk dispatches at later dates.
#[derive(Debug, Serialize, Deserialize)]
pub struct DispatchableTrack {
    pub name: String,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub artist: Option<String>,
    /// Uppercase hexadecimal representation of the track's persistent ID.
    pub persistent_id: String,
    pub duration: Option<core::time::Duration>,
    pub media_kind: osa_apple_music::track::MediaKind,
    pub track_number: Option<core::num::NonZero<u16>>,
}
impl From<osa_apple_music::Track> for DispatchableTrack {
    fn from(track: osa_apple_music::Track) -> Self {
        let track: osa_apple_music::track::BasicTrack = track.into();
        Self {
            name: track.name,
            album: track.album.name,
            album_artist: track.album.artist,
            artist: track.artist,
            persistent_id: track.persistent_id.clone(),
            media_kind: track.media_kind,
            duration: track.duration,
            track_number: track.track_number,
        }
    }
}
impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for DispatchableTrack {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            name: row.try_get("name")?,
            album: row.try_get("album")?,
            album_artist: row.try_get("album_artist")?,
            artist: row.try_get("artist")?,
            persistent_id: row.try_get("persistent_id")?,
            media_kind: row.try_get("media_kind")?,
            duration: row.try_get::<Option<f32>, _>("duration")?.map(core::time::Duration::from_secs_f32),
            track_number: row.try_get("track_number")?,
        })
    }
}

pub mod uncensor {
    use super::*;

    /// Attempt to uncensor a title utilizing a combination of the display name and the sorting name.
    /// 
    /// This takes advantage of the fact that Apple does not censor words within the sorting name.
    /// 
    /// However, care must be taken regarding the fact that a sorting name strips out certain
    /// prefixes, such as "The", which will need to be re-added like they are within the display name.
    pub fn heuristically_uncensor_name<'a>(display: &str, sorting: &'a str) -> Option<MaybeOwnedString<'a>> {
        fn do_names_match_lhs_wildcarded(display: &str, sorting: &str) -> bool {
            if display == sorting {
                return true;
            }
    
            if display.len() != sorting.len() {
                return false;
            }
    
            for (canon, censored) in sorting.chars().zip(display.chars()) {
                match (canon, censored) {
                    (_, '*') => continue, // true value unknown; treat as wildcard
                    (l, r) if l == r => continue,
                    _ => return false
                }
            }
    
            true
        }
    
        const NO_PREFIX: &str = "";
    
        [NO_PREFIX, "The ", "A ", "An "]
            .iter()
            .flat_map(|prefix| display.strip_prefix(prefix).map(|stripped| (prefix, stripped)))
            .filter(|(prefix, stripped)| do_names_match_lhs_wildcarded(stripped, sorting))
            .map(|(prefix, _)| match prefix.len() {
                0 => MaybeOwnedString::Borrowed(sorting),
                _ => MaybeOwnedString::Owned(format!("{prefix}{sorting}"))
            }).next()
    }
    pub use heuristically_uncensor_name as heuristically;

    pub async fn uncensor_name_itunes(track: &DispatchableTrack) -> Option<String> {
        crate::data_fetching::services::itunes::find_track(track)
            .await
            .inspect_err(|err| {
                tracing::error!(error = ?err, "failed to fetch track info from iTunes");
            }).ok().flatten().map(|track| track.name)
    }
    pub use uncensor_name_itunes as with_itunes;

    pub async fn uncensor_track<'a>(track: &'a DispatchableTrack) -> Option<MaybeOwnedString<'a>> {
        if let Some(name) = heuristically_uncensor_name(&track.name, &track.persistent_id) {
            Some(name)
        } else {
            uncensor_name_itunes(track).await.map(MaybeOwnedString::Owned)
        }
    }
    pub use uncensor_track as track;

    #[cfg(test)]
    mod tests {
        use super::heuristically_uncensor_name;

        #[test]
        fn heuristically() {
            assert!(heuristically_uncensor_name(    "f**k", "fuck") == Some(    "fuck".into()));
            assert!(heuristically_uncensor_name("The f**k", "fuck") == Some("The fuck".into()));
            assert!(heuristically_uncensor_name("The foo",  "foo" ) == Some("The foo" .into()));
            assert!(heuristically_uncensor_name(  "A foo",  "foo" ) == Some(  "A foo" .into()));
        }
    }
}


#[derive(Debug)]
pub struct BackendContext<A> {
    pub track: Arc<DispatchableTrack>,
    pub app: Arc<osa_apple_music::ApplicationData>,
    pub data: Arc<A>,
    pub listened: Arc<Mutex<crate::listened::Listened>>,
    pub musicdb: Arc<Option<musicdb::MusicDB>>,
}
impl<A> Clone for BackendContext<A> {
    fn clone(&self) -> Self {
        Self {
            track: self.track.clone(),
            app: self.app.clone(),
            data: self.data.clone(),
            listened: self.listened.clone(),
            musicdb: self.musicdb.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DispatchedApplicationStatus {
    Playing,
    /// The music stopped and there is no more music that will start playing soon.
    // TODO: uhh fact-check this it's been so long
    Stopped,
    /// A temporary pause in track playback; the boolean indicating whether this has started or ended.
    /// This may not be a result of user action— this may also be dispatched when encountering playback buffering issues.
    Paused,
}
impl From<osa_apple_music::application::PlayerState> for DispatchedApplicationStatus {
    fn from(value: osa_apple_music::application::PlayerState) -> Self {
        use osa_apple_music::application::PlayerState;
        match value {
            PlayerState::Playing => Self::Playing,
            PlayerState::Paused => Self::Paused,
            PlayerState::Stopped => Self::Stopped,
            _ => unimplemented!("unforeseen player state")
        }
    }
}

struct TransientSendableUntypedRawBoxPointer(*mut u8); // are we so fr
unsafe impl Send for TransientSendableUntypedRawBoxPointer {}

pub use subscription::{Subscriber, subscribe};
pub mod subscription {
    use crate::data_fetching::components::ComponentSolicitation;

    use super::{error::DispatchError, BackendContext, TransientSendableUntypedRawBoxPointer};

    type DefaultContext = BackendContext<()>;
    type DefaultReturn = ();

    macro_rules! define {
        (
            $dollar: tt, // nested macro hack until $$ is stabilized
            [$({
                $(#[$meta:meta])* $name:ident $($extra: tt)*
            }$($comma: tt)?)*],
            { $($subscriber: tt)* }
        ) => {
            $(
                define!(@trait@ $(#[$meta])* $name $($extra)*);
            )*


            #[derive(Debug, PartialEq, Eq, Clone, Copy)]
            pub enum Identity { $($name,)* }

            pub use type_identity::TypeIdentity;
            pub mod type_identity {
                pub mod context {
                    $(
                        define!(@context@ $name, $($extra)*);
                    )*
                }
                pub mod returns {
                    $(
                        define!(@returns@ $name, $($extra)*);
                    )*
                }
                
                pub trait TypeIdentity: core::fmt::Debug {
                    const IDENTITY: super::Identity;
                    type DispatchContext: Send + Clone;
                    type DispatchReturn: Send;
                }
                $(
                    #[derive(Debug)]
                    pub struct $name;
                    impl TypeIdentity for $name {
                        const IDENTITY: super::Identity = super::Identity::$name;
                        type DispatchContext = super::type_identity::context::$name;
                        type DispatchReturn = super::type_identity::returns::$name;
                    }
                )*
            }

            use cast_trait_object::{create_dyn_cast_config, DynCast};

            pub mod cast_configs {
                $(
                    super::create_dyn_cast_config!(pub $name = super::Subscriber => super::$name<Identity = super::type_identity::$name>);
                )*
            }

            #[async_trait::async_trait]
            pub trait Subscriber: $(DynCast<cast_configs::$name> +)* core::fmt::Debug + Sync + Send {
                $($subscriber)*
            }

            #[macro_export]
            macro_rules! define_subscriber {
                (
                    $dollar(#[$sub_meta:meta])*
                    $vis:vis
                    $sub_name:ident,
                    $dollar($def:tt)*
                ) => {
                    cast_trait_object::impl_dyn_cast!($sub_name => $($crate::status_backend::subscription::cast_configs::$name),*);
                    $dollar(#[$sub_meta])*
                    $vis struct $sub_name $dollar($def)*
                    impl $sub_name {
                        pub const NAME: &'static str = stringify!($sub_name);
                    }
                    #[async_trait::async_trait]
                    impl $crate::status_backend::subscription::Subscriber for $sub_name {
                        async fn get_solicitation(&self, event: $crate::status_backend::subscription::Identity) -> Option<$crate::data_fetching::components::ComponentSolicitation> {
                            match event {
                                $(
                                    $crate::status_backend::subscription::Identity::$name => {
                                        let typed = <dyn $crate::status_backend::subscription::Subscriber as cast_trait_object::DynCast<$crate::status_backend::subscription::cast_configs::$name>>::dyn_cast_ref(self).ok()?;
                                        Some($crate::status_backend::subscription::$name::get_solicitation(typed).await)
                                    }
                                )*,
                            }
                        }

                        #[allow(private_interfaces)]
                        async unsafe fn dispatch_untyped(
                            &mut self,
                            event: $crate::status_backend::subscription::Identity,
                            context: $crate::status_backend::TransientSendableUntypedRawBoxPointer
                        ) -> Option<
                            Result<
                                $crate::status_backend::TransientSendableUntypedRawBoxPointer,
                                $crate::status_backend::error::DispatchError
                            >
                        > {
                            match event {
                                $(
                                    $crate::status_backend::subscription::Identity::$name => {
                                        let typed = <dyn $crate::status_backend::subscription::Subscriber as cast_trait_object::DynCast<$crate::status_backend::subscription::cast_configs::$name>>::dyn_cast_mut(self).ok()?;
                                        type Context = $crate::status_backend::subscription::type_identity::context::$name;
                                        let context = context.0 as *mut Context;
                                        let context = unsafe { Box::from_raw(context) };
                                        let output = typed.dispatch(*context).await;
                                        let output = output.map(Box::new).map(Box::into_raw).map(|ptr| $crate::status_backend::TransientSendableUntypedRawBoxPointer(ptr as *mut u8));
                                        Some(output)
                                    }
                                )*,
                            }
                        }

                        fn get_identity(&self) -> $crate::status_backend::BackendIdentity {
                            $crate::status_backend::BackendIdentity::$sub_name
                        }
                    }
                };
                (
                    $dollar(#[$dollar sub_meta:meta])*
                    $vis:vis
                    $sub_name:ident
                ) => {
                    define_subscriber! {
                        $dollar(#[$dollar sub_meta])*
                        $vis
                        $sub_name,
                    }
                }
            }

            pub use define_subscriber;
        };
        (@trait@ $(#[$meta:meta])* $name:ident<$context: ty>) => {
            define!(@trait@ $(#[$meta])* $name<$context, $crate::status_backend::subscription::DefaultReturn>);
        };
        (@trait@ $(#[$meta:meta])* $name:ident<_, $return: ty>) => {
            define!(@trait@ $(#[$meta])* $name<$crate::status_backend::subscription::DefaultContext, $return>);
        };
        (@trait@ $(#[$meta:meta])* $name:ident) => {
            define!(@trait@ $(#[$meta])* $name<$crate::status_backend::subscription::DefaultContext, $crate::status_backend::subscription::DefaultReturn>);
        };
        (@trait@ $(#[$meta:meta])* $name:ident<$context: ty, $return: ty>) => {
            $(#[$meta])*
            #[async_trait::async_trait]
            pub trait $name: Subscriber {
                type Identity: $crate::status_backend::subscription::TypeIdentity;

                async fn dispatch(&mut self, context: $context) -> Result<$return, super::error::DispatchError>;

                async fn get_solicitation(&self) -> super::ComponentSolicitation {
                    super::ComponentSolicitation::default()
                }
            }
        };
        (@make_meta@ [$({ $($subscription: tt)* }$($comma: tt)?)*]) => {
            $(
                define!(@name@ $($subscription)*)
                $($comma)?
            )*
        };
        (@context@ $name: ident, <$context: ty, $(tt)*) => {
            pub type $name = $context;
        };
        (@context@ $name: ident, <$context: ty>) => {
            pub type $name = $context;
        };
        (@context@ $name: ident, <_, $return: ty>) => {
            define!(@context@ $name, <$crate::status_backend::subscription::DefaultContext,);
        };
        (@context@ $name: ident,) => {
            define!(@context@ $name, <$crate::status_backend::subscription::DefaultContext,);
        };
        (@returns@ $name: ident, <$context: ty, $return: ty>) => {
            pub type $name = $return;
        };
        (@returns@ $name: ident, <$context: ty>) => {
            define!(@returns@ $name, <$context, $crate::status_backend::subscription::DefaultReturn>);
        };
        (@returns@ $name: ident, <_, $return: ty>) => {
            define!(@returns@ $name, <(), $return>);
        };
        (@returns@ $name: ident,) => {
            define!(@returns@ $name, <(), $crate::status_backend::subscription::DefaultReturn>);
        };
    }
    
    define!($, [
        { TrackStarted<crate::status_backend::BackendContext<crate::data_fetching::AdditionalTrackData>> },
        { TrackEnded },
        { ProgressJolt },
        { ApplicationStatusUpdate<crate::status_backend::DispatchedApplicationStatus> },
    ], {
        async fn get_solicitation(&self, event: self::Identity) -> Option<ComponentSolicitation>;
        #[allow(private_interfaces)]
        async unsafe fn dispatch_untyped(&mut self, event: self::Identity, value: TransientSendableUntypedRawBoxPointer) -> Option<Result<TransientSendableUntypedRawBoxPointer, DispatchError>>;
        fn get_identity(&self) -> crate::status_backend::BackendIdentity;
    });

    #[macro_export]
    macro_rules! subscribe {
        ($struct: ident, $ident: ident, { $($t: tt)* }) => {
            #[async_trait::async_trait]
            impl $crate::status_backend::subscription::$ident for $struct {
                type Identity = $crate::status_backend::subscription::type_identity::$ident;
    
                $($t)*
            }
        }
    }

    pub use subscribe;
}


impl Backends {
    #[tracing::instrument(level = "debug")]
    pub async fn get_solicitations(&self, event: subscription::Identity) -> ComponentSolicitation {
        let backends = self.all();
        let mut solicitation = ComponentSolicitation::default();
        let mut jobs = Vec::with_capacity(backends.len());
        for backend in backends {
            jobs.push(tokio::spawn(async move {
                backend.lock().await.get_solicitation(event).await
            }));
        }
        for (i, job) in jobs.into_iter().enumerate() {
            match job.await {
                Ok(Some(got)) => solicitation += got,
                Ok(None) => (),
                Err(err) => {
                    let backend = self.all()[i].lock().await.get_identity().get_name();
                    tracing::error!(?err, backend, "error getting solicitation; skipping")
                },
            };
        }
        solicitation
    }

    #[tracing::instrument(skip(context), level = "debug")]
    async fn dispatch<T: subscription::TypeIdentity>(&self, context: T::DispatchContext) -> BackendMap<Result<T::DispatchReturn, DispatchError>> {
        let backends = self.all();
        let mut outputs = BackendMap::new();
        let mut jobs = Vec::with_capacity(backends.len());

        for backend in backends {
            let context = context.clone();
            let context = Box::into_raw(Box::new(context));
            let context = TransientSendableUntypedRawBoxPointer(context as *mut u8);
            jobs.push(tokio::spawn(async move {
                let mut backend = backend.lock().await;
                let backend = &mut *backend;
                unsafe { backend.dispatch_untyped(T::IDENTITY, context).await }
                    .map(|result| (backend.get_identity(), result))
            }));
        }

        for (i, job) in jobs.into_iter().enumerate() {
            match job.await {
                Ok(None) => {},
                Ok(Some((identity, result))) => {
                    outputs[identity] = Some(result.map(|ptr| {
                        let ptr = ptr.0 as *mut T::DispatchReturn;
                        let ptr = unsafe { Box::from_raw(ptr) };
                        *ptr
                    }));
                },
                Err(err) => {
                    let backend = self.all()[i].lock().await.get_identity().get_name();
                    tracing::error!(?err, backend, "error dispatching track completion")
                }
            }
        };

        outputs
    }

    #[tracing::instrument(skip(context), level = "debug", fields(track = &context.track.persistent_id))]
    pub async fn dispatch_track_started(&self, context: BackendContext<crate::data_fetching::AdditionalTrackData>) {
        type Variant = subscription::type_identity::TrackStarted;
        for (identity, error) in self.dispatch::<Variant>(context).await.into_errors_iter() {
            error.handle(identity.get_name(), Variant {});
        }
    }

    #[tracing::instrument(skip(context), level = "debug", fields(track = &context.track.persistent_id))]
    pub async fn dispatch_track_ended(&self, context: BackendContext<()>) {
        type Variant = subscription::type_identity::TrackEnded;
        for (identity, error) in self.dispatch::<Variant>(context).await.into_errors_iter() {
            error.handle(identity.get_name(), Variant {});
        }
    }

    #[tracing::instrument(skip(context), level = "debug", fields(track = &context.track.persistent_id))]
    pub async fn dispatch_current_progress(&self, context: BackendContext<()>) {
        type Variant = subscription::type_identity::ProgressJolt;
        for (identity, error) in self.dispatch::<Variant>(context).await.into_errors_iter() {
            error.handle(identity.get_name(), Variant {});
        }
    }

    #[tracing::instrument(level = "debug")]
    pub async fn dispatch_status(&self, status: DispatchedApplicationStatus) {
        type Variant = subscription::type_identity::ApplicationStatusUpdate;
        for (identity, error) in self.dispatch::<Variant>(status).await.into_errors_iter() {
            error.handle(identity.get_name(), Variant {});
        }
    }


    pub async fn new(config: &crate::config::Config<'_>) -> Backends {        
        #[cfg(feature = "lastfm")]
        use crate::status_backend::lastfm::*;

        #[cfg(feature = "discord")]
        use crate::status_backend::discord::*;

        #[cfg(feature = "listenbrainz")]
        use crate::status_backend::listenbrainz::*;

        #[cfg(feature = "lastfm")]
        let lastfm = config.backends.lastfm.as_ref().and_then(|config| {
            if config.enabled {
                Some(Arc::new(Mutex::new(LastFM::new(
                    config.identity.clone(),
                    config.session_key.clone().expect("no session keys")
                ))))
            } else { None }
        });
        
        #[cfg(feature = "listenbrainz")]
        let listenbrainz = config.backends.listenbrainz.as_ref().and_then(|config| {
            if config.enabled {
                Some(Arc::new(Mutex::new(ListenBrainz::new(
                    config.program_info.clone(),
                    config.user_token.clone().expect("no token")
                ))))
            } else { None }
        });

        #[cfg(feature = "discord")]
        let discord = match config.backends.discord.as_ref().copied() {
            Some(config) if config.enabled => Some(DiscordPresence::new(config).await),
            _ => None
        };

        Backends {
            #[cfg(feature = "lastfm")] lastfm,
            #[cfg(feature = "discord")] discord,
            #[cfg(feature = "listenbrainz")] listenbrainz
        }
    }
}
