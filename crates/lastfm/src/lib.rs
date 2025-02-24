#![allow(unused)]

use auth::AccountCredentials;
use maybe_owned_string::MaybeOwnedString;
use serde::Deserialize;
pub mod auth;
pub mod scrobble;
pub mod error;
mod parameters;


pub use error::Error;
pub type Result<T, E = error::GeneralErrorCode> = ::core::result::Result<T, Error<E>>;

pub(crate) const API_URL: &str = "https://ws.audioscrobbler.com/2.0/";

pub struct Client<A: auth::state::AuthorizationStatus> {
    pub identity: auth::ClientIdentity,
    pub net: reqwest::Client, // exposed for re-use if dev'd like to only have one
    session_key: Option<auth::SessionKey>,
    _authorized: core::marker::PhantomData<A>
}
impl<A: auth::state::AuthorizationStatus> Client<A> {
    pub const fn is_authorized(&self) -> bool {
        self.session_key.is_some()
    }
}
impl Client<auth::state::Unauthorized> {
    pub fn new(identity: auth::ClientIdentity) -> Client<auth::state::Unauthorized> {
        Client::<auth::state::Unauthorized> {
            net: reqwest::Client::builder().user_agent(&identity.user_agent).build().expect("cannot construct reqwest client"),
            identity,
            session_key: None,
            _authorized: core::marker::PhantomData
        }
    }

    pub fn into_authorized(self, session_key: auth::SessionKey) -> Client<auth::state::Authorized> {
        Client::<auth::state::Authorized> {
            net: self.net,
            identity: self.identity,
            session_key: Some(session_key),
            _authorized: core::marker::PhantomData,
        }
    }
}
impl<'a> Client<auth::state::Authorized> {
    pub fn authorized(identity: auth::ClientIdentity, session_key: auth::SessionKey) -> Self {
        Self {
            net: reqwest::Client::builder().user_agent(&identity.user_agent).build().expect("cannot construct reqwest client"),
            identity,
            session_key: Some(session_key),
            _authorized: core::marker::PhantomData,
        }
    }

    pub const fn session_key(&self) -> &auth::SessionKey {
        self.session_key.as_ref().expect("no session key on client with authenticated type-state")
    }

    async fn dispatch_authorized<'b: 'a>(&'b self, mut request: ApiRequest<'a>) -> ::core::result::Result<reqwest::Response, reqwest::Error> {
        request.parameters.add("sk".to_string(), MaybeOwnedString::Borrowed(self.session_key().as_ref()));
        request.parameters.add("method".to_string(), MaybeOwnedString::Borrowed(request.endpoint));
        request.parameters.add("api_key".to_string(), MaybeOwnedString::Borrowed(self.identity.get_key()));
        request.parameters.add("api_sig".to_string(), MaybeOwnedString::Owned(request.parameters.sign(self.session_key(), &self.identity).to_string()));
        request.parameters.add("format".to_string(), MaybeOwnedString::Borrowed("json"));
        let request = self.net.request(request.method, crate::API_URL)
            .header("Content-Length", "0")
            .header("User-Agent", &self.identity.user_agent)
            .query(&request.parameters)
            .build()?;
        self.net.execute(request).await
    }


    pub async fn scrobble(&self, scrobbles: &[scrobble::Scrobble<'a>]) -> Result<scrobble::response::ScrobbleServerResponse> {
        let response = self.dispatch_authorized(ApiRequest {
            endpoint: "track.scrobble",
            method: reqwest::Method::POST,
            parameters: scrobbles.into(),
        }).await?;

        let response = response.text().await?;
        let response = scrobble::response::ScrobbleServerResponse::new(response, scrobbles.len())?;
        
        Ok(response)
    }

    pub async fn set_now_listening(&self, track: &scrobble::HeardTrackInfo<'_>) -> Result<scrobble::response::ServerUpdateNowPlayingResponse> {
        let response = self.dispatch_authorized(ApiRequest {
            endpoint: "track.updateNowPlaying",
            method: reqwest::Method::POST,
            parameters: track.into(),
        }).await?;

        let response = response.text().await?;
        let response = scrobble::response::ServerUpdateNowPlayingResponse::new(response)?;
        
        Ok(response)
    }
}

struct ApiRequest<'a> {
    /// Called the "method" (as in method of a service) by Last.fm
    endpoint: &'static str,
    method: reqwest::Method,
    parameters: parameters::Map<'a>
}

