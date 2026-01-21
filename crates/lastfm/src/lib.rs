#![allow(unused)]

use auth::AccountCredentials;
use maybe_owned_string::MaybeOwnedString;
use serde::Deserialize;
pub mod auth;
pub mod scrobble;
pub mod error;
mod parameters;


use crate::error::code::ErrorCode;
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

    async fn dispatch_unauthorized<'a>(&'a self, mut request: ApiRequest<'a>) -> ::core::result::Result<reqwest::Response, reqwest::Error> {
        request.parameters.add("method".to_string(), MaybeOwnedString::Borrowed(request.endpoint));
        request.parameters.add("api_key".to_string(), MaybeOwnedString::Borrowed(self.identity.get_key()));
        request.parameters.add("format".to_string(), MaybeOwnedString::Borrowed("json"));
        let request = self.net.request(request.method, crate::API_URL)
            .header("User-Agent", &self.identity.user_agent)
            .form(&request.parameters)
            .build()?;
        self.net.execute(request).await
    }
}
impl Client<auth::state::Unauthorized> {
    pub fn new(identity: auth::ClientIdentity) -> Client<auth::state::Unauthorized> {
        Self::new_with_reqwest_client(identity, reqwest::Client::new())
    }

    pub fn new_with_reqwest_client(identity: auth::ClientIdentity, net: reqwest::Client) -> Client<auth::state::Unauthorized> {
        Client::<auth::state::Unauthorized> {
            net,
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
        Self::authorized_with_reqwest_client(identity, session_key, reqwest::Client::new())
    }
    pub fn authorized_with_reqwest_client(identity: auth::ClientIdentity, session_key: auth::SessionKey, net: reqwest::Client) -> Self {
        Self {
            net,
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
        request.parameters.add("api_sig".to_string(), MaybeOwnedString::Owned(request.parameters.sign(self.session_key(), &self.identity).to_string()));
        self.dispatch_unauthorized(request).await
    }

    pub async fn scrobble(&self, scrobbles: &[scrobble::Scrobble<'_>]) -> Result<scrobble::response::ScrobbleServerResponse<'_>> {
        let response = self.dispatch_authorized(ApiRequest {
            endpoint: "track.scrobble",
            method: reqwest::Method::POST,
            parameters: scrobbles.into(),
        }).await?.text().await?;

        if let Some(error) = error::GeneralErrorCode::try_from_api_response_body(&response) {
            return Err(error.into());
        }

        scrobble::response::ScrobbleServerResponse::new(response, scrobbles.len()).map_err(Error::from)
    }

    pub async fn set_now_listening(&self, track: &scrobble::HeardTrackInfo<'_>) -> Result<scrobble::response::ServerUpdateNowPlayingResponse<'_>> {
        let response = self.dispatch_authorized(ApiRequest {
            endpoint: "track.updateNowPlaying",
            method: reqwest::Method::POST,
            parameters: track.into(),
        }).await?.text().await?;

        if let Some(error) = error::GeneralErrorCode::try_from_api_response_body(&response) {
            return Err(error.into());
        }

        scrobble::response::ServerUpdateNowPlayingResponse::new(response).map_err(Error::from)
    }
}

struct ApiRequest<'a> {
    /// Called the "method" (as in method of a service) by Last.fm
    endpoint: &'static str,
    method: reqwest::Method,
    parameters: parameters::Map<'a>
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore = "requires network access"]
    async fn unknown_method() {
        use crate::error::code::{self, ErrorCode, GeneralErrorCode};

        let client = super::Client::new(
            super::auth::ClientIdentity::new(
                "am-osx-status-lfm-test".to_string(),
                "d591a37a79ec4c3d4efe55379029b5b3",
                "20a069921b30039bd2601d955e3bce46"
            ).unwrap()
        );

        let response = client.dispatch_unauthorized(crate::ApiRequest {
            endpoint: "non-existent-endpoint",
            method: reqwest::Method::POST,
            parameters: ::core::default::Default::default()
        }).await.unwrap().text().await.unwrap();

        assert_eq!(GeneralErrorCode::try_from_api_response_body(&response), Some(Ok(code::general::InvalidUsage::InvalidMethod.into())));
    }
}

