use maybe_owned_string::MaybeOwnedStringDeserializeToOwned;
use serde::{Deserialize, Serialize};

pub mod submit_listens;
pub mod error;

pub const API_ROOT: &str = "https://api.listenbrainz.org/1/";


#[repr(transparent)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserToken(shared::HyphenatedUuidString);
impl UserToken {
    pub async fn new(token: impl AsRef<str>) -> Result<Self, ValidTokenInstantiationError> {
        let token = token.as_ref();
        let token = shared::HyphenatedUuidString::new(token).ok_or(error::InvalidTokenError)?;
        
        match Self::check_validity(token).await? {
            TokenValidity::Valid { .. } => Ok(Self(token)),
            TokenValidity::Invalid => Err(error::InvalidTokenError)?
        }
    }

    pub async fn check_validity(token: impl core::fmt::Display) -> Result<TokenValidity, reqwest::Error> {
        let url = &format!("{API_ROOT}/validate-token?token={token}");
        let response = reqwest::get(url).await?;

        #[derive(serde::Deserialize)]
        struct RawTokenValidityResponse<'a> {
            code: u16,
            message: &'a str,
            #[serde(rename = "user_name")]
            username: Option<&'a str>, // only present on valid tokens
            valid: bool,
        }

        let response = response.text().await?;
        let response = serde_json::from_str::<RawTokenValidityResponse>(&response).expect("cannot deserialize output");
        // ^ Do I really want to panic here? What if it returns an NGINX 500 HTML error page, or something? (TODO)

        if let Some(username) = response.username.map(str::to_owned) {
            Ok(TokenValidity::Valid { username })
        } else {
            Ok(TokenValidity::Invalid)
        }
    }
}
impl core::fmt::Display for UserToken {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.0.as_str())
    }
}
use token_validity::*;
pub mod token_validity {
    pub enum TokenValidity {
        Invalid,
        Valid { username: String }
    }
    impl TokenValidity {
        pub const fn is_valid(&self) -> bool {
            matches!(self, TokenValidity::Valid { .. })
        }
        pub const fn is_invalid(&self) -> bool {
            matches!(self, TokenValidity::Invalid)
        }
    }
    
    #[derive(Debug, thiserror::Error)]
    pub enum ValidTokenInstantiationError {
        #[error("token invalid")]
        Invalid(#[from] super::error::InvalidTokenError),
        #[error("cannot perform validity check: network failure: {0}")]
        ValidityCheckFailure(#[from] reqwest::Error),
    }
}





struct InternalSharedListenPack<'a> {
    track: submit_listens::BasicTrackMetadata<'a>,
    timestamp: Option<chrono::DateTime<chrono::Utc>>,
    extra: Option<submit_listens::additional_info::AdditionalInfo<'a>>
}


// TODO: add authorization type-state like lastfm
// TODO: ratelimit middleware?
pub struct Client<PS: AsRef<str>> {
    net: reqwest::Client,
    program: musicbrainz::request_client::ProgramInfo<PS>,
    token: Option<UserToken>,
}
impl<PS: AsRef<str>> Client<PS> {
    fn mk_net(program: &musicbrainz::request_client::ProgramInfo<PS>, token: Option<&UserToken>) -> reqwest::Client {
        let mut client = reqwest::ClientBuilder::new()
            .https_only(true)
            .user_agent(program.to_user_agent());

        if let Some(token) = token {
            use reqwest::header::*;
            let mut headers = HeaderMap::with_capacity(1);
            let mut header = HeaderValue::from_str(&format!("Token {token}")).expect("bad token"); header.set_sensitive(true);
            let header = headers.insert(HeaderName::from_static("authorization"), header);
            client = client.default_headers(headers)
        }

        client.build().expect("could not build network client")
    }

    pub fn get_program_info(&self) -> &musicbrainz::request_client::ProgramInfo<PS> {
        &self.program
    }

    pub fn new(program: musicbrainz::request_client::ProgramInfo<PS>, token: Option<UserToken>) -> Self {
        Self {
            net: Self::mk_net(&program, token.as_ref()),
            program,
            token
        }
    }

    async fn submit_listen_payloads(&self, variant: submit_listens::ListenType, payloads: &[submit_listens::ListeningPayload<'_>]) -> Result<(reqwest::StatusCode, String), reqwest::Error> {
        let body = submit_listens::RawBody {
            listen_type: variant,
            payload: payloads
        }.to_json();

        // TODO: Make use of the defined payload limits in the constants file.
        
        let response = self.net.post(format!("{API_ROOT}/submit-listens")).body(body).send().await?;
        Ok((response.status(), response.text().await?))
    }

    pub async fn submit_playing_now(&self, track: submit_listens::BasicTrackMetadata<'_>, extra: Option<submit_listens::additional_info::AdditionalInfo<'_>>) -> Result<(), submit_listens::CurrentlyPlayingSubmissionError> {
        let (code, body) = self.submit_listen_payloads(submit_listens::ListenType::PlayingNow, &[submit_listens::ListeningPayload {
            listened_at: None,
            metadata: submit_listens::ListeningPayloadTrackMetadata {
                basic: track,
                additional_info: extra.map(|info| info.into_raw())
            }
        }]).await?;

        use reqwest::StatusCode;
        use submit_listens::CurrentlyPlayingSubmissionError;
        match code {
            StatusCode::OK => Ok(()),
            StatusCode::TOO_MANY_REQUESTS => Err(CurrentlyPlayingSubmissionError::Ratelimited),
            StatusCode::UNAUTHORIZED => Err(error::InvalidTokenError)?,
            code => Err(CurrentlyPlayingSubmissionError::Other(code, body))
        }
    }

    pub async fn submit_listen(&self, track: submit_listens::BasicTrackMetadata<'_>, time: chrono::DateTime<chrono::Utc>, extra: Option<submit_listens::additional_info::AdditionalInfo<'_>>) -> Result<(), submit_listens::ListenSubmissionError> {
        if time < super::constants::LISTEN_MINIMUM_DATE {
            return Err(error::ListenDateTooHistoric)?;
        }


        let (code, body) = self.submit_listen_payloads(submit_listens::ListenType::Single, &[submit_listens::ListeningPayload {
            listened_at: Some(time.timestamp() as u32),
            metadata: submit_listens::ListeningPayloadTrackMetadata {
                basic: track,
                additional_info: extra.map(|info| info.into_raw())
            }
        }]).await?;

        use reqwest::StatusCode;
        use submit_listens::ListenSubmissionError;
        match code {
            StatusCode::OK => Ok(()),
            StatusCode::TOO_MANY_REQUESTS => Err(ListenSubmissionError::Ratelimited),
            StatusCode::UNAUTHORIZED => Err(error::InvalidTokenError)?,
            code => Err(ListenSubmissionError::Other(code, body))
        }
    }
}


