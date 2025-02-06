use std::str::FromStr;

use serde::{Serialize, Deserialize};


use crate::GeneralError;

pub mod state {
    pub trait AuthorizationStatus {}

    pub struct Authorized;
    pub struct Unauthorized;

    impl AuthorizationStatus for Authorized {}
    impl AuthorizationStatus for Unauthorized {}
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct ClientIdentity {
    key: internal::ThirtyTwoCharactersLowercaseHexAsciiString,
    secret: internal::ThirtyTwoCharactersLowercaseHexAsciiString,
    pub user_agent: String,
}
impl ClientIdentity {
    pub fn new(user_agent: String, key: &str, secret: &str) -> Result<Self, internal::InvalidThirtyTwoCharactersLowercaseHexAsciiStringError> {
        match internal::ThirtyTwoCharactersLowercaseHexAsciiString::new(key) {
            Err(err) => Err(err),
            Ok(key) => match internal::ThirtyTwoCharactersLowercaseHexAsciiString::new(secret) {
                Err(err) => Err(err),
                Ok(secret) => Ok(Self { user_agent, key, secret })
            },
        }
    }

    pub async fn generate_authorization_token(&self) -> Result<AuthorizationToken, AuthorizationTokenGenerationError> {
        AuthorizationToken::generate(self).await
    }

    pub const fn get_key(&self) -> &str {
        self.key.as_str()
    }
    pub const fn get_secret(&self) -> &str {
        self.secret.as_str()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthorizationToken(internal::ThirtyTwoCharacterAsciiString);
impl AuthorizationToken {
    pub const unsafe fn new_unchecked(str: &str) -> Self {
        Self(internal::ThirtyTwoCharacterAsciiString::new_unchecked(str.as_bytes()))
    }

    /// <https://www.last.fm/api/show/auth.getToken>
    pub async fn generate(client: &ClientIdentity) -> Result<AuthorizationToken, AuthorizationTokenGenerationError> {
        let url = format!("{}?method=auth.gettoken&api_key={}&format=json", crate::API_URL, client.key);
        let response = reqwest::get(url).await?;

        #[derive(serde::Serialize, serde::Deserialize)]
        #[serde(untagged)]
        enum Response {
            Ok { token: AuthorizationToken },
            Fail { #[serde(rename = "error")] code: u8, message: String }
        }
        
        let response = response.text().await?;
        let response = serde_json::from_str(&response).expect("deserialization failure");
        // ^ Do I really want to panic here? What if it returns an NGINX 500 HTML error page, or something? (TODO)


        match response {
            Response::Ok { token } => Ok(token),
            Response::Fail { code,  .. } => panic!("fail w/ code {code}") // FIXME better error handling
        }
    }

    pub fn generate_authorization_url(&self, client: &ClientIdentity) -> String {
        format!("https://www.last.fm/api/auth/?api_key={}&token={self}", client.key)
    }

    /// [`Self::get_authorization_url`] flow must be completed prior to obtaining a session token.
    /// - <https://www.last.fm/api/show/auth.getSession>
    pub async fn generate_session_key(&self, client: &ClientIdentity) -> Result<SessionKey, SessionKeyThroughAuthorizationTokenError> {
        let signature = format!("{:x}", md5::compute(format!("api_key{}methodauth.getSessiontoken{self}{}", client.key, client.secret)));
        let response = reqwest::Client::new().post(crate::API_URL)
            .header("Content-Length", "0")
            .header("User-Agent", &client.user_agent)
            .query(&[
                ("format", "json"),
                ("method", "auth.getSession"),
                ("api_key", client.key.as_ref()),
                ("api_sig", &signature),
                ("token", self.0.as_str()),
            ])
            .send().await?
            .text().await?;

        match serde_json::from_str(&response).expect("output could not be deserialized") {
            SessionKeyGenerationResponse::Ok { session } => Ok(session.key),
            SessionKeyGenerationResponse::Fail { code,  .. } => match code {
                4 => Err(SessionKeyThroughAuthorizationTokenError::Invalid),
                14 => Err(SessionKeyThroughAuthorizationTokenError::Unauthorized),
                15 => Err(SessionKeyThroughAuthorizationTokenError::Expired),
                _ => Err(SessionKeyThroughAuthorizationTokenError::General(GeneralError::try_from(code).expect("unknown error code")))
            }
        }
    }
}
impl AsRef<str> for AuthorizationToken {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}
impl core::fmt::Display for AuthorizationToken {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}


#[derive(Debug, thiserror::Error)]
pub enum AuthorizationTokenGenerationError {
    #[error("network error: {0}")]
    NetworkError(#[from] reqwest::Error)
}

/// Returned by the session generation endpoints upon success.
#[derive(Serialize, Deserialize)]
struct SessionInfo {
    name: String,
    key: SessionKey,
    subscriber: u32
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum SessionKeyGenerationResponse {
    Ok { session: SessionInfo },
    Fail { #[serde(rename = "error")] code: u8, message: String }
}


/// <https://www.last.fm/api/show/auth.getSession#Errors>
#[derive(Debug, thiserror::Error)]
pub enum SessionKeyThroughAuthorizationTokenError {
    #[error("network failure: {0}")]
    NetworkFailure(#[from] reqwest::Error),

    /// The authorization token is not valid. This may mean it has already been used to successfully create a session.
    #[error("authorization token invalid")]
    Invalid, // 4
    /// The authorization token was not authorized by the user.
    #[error("token was not authorized")]
    Unauthorized, // 14
    /// The authorization token has expired. 
    #[error("token expired")]
    Expired, // 15

    #[error("{0}")]
    General(#[from] crate::GeneralError)
}

/// A key authenticating an authorized user session.
/// 
/// Obtainable via 
///  - [`AccountCredentials::generate_session_key`]
///  - [`AuthorizationToken::generate_session_key`] (after user completion of [`AuthorizationToken::get_authorization_url`])
// TODO: Mobile obtainment method.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionKey(internal::ThirtyTwoCharacterAsciiString);
impl SessionKey {
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
impl AsRef<str> for SessionKey {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}
impl core::fmt::Display for SessionKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Serialize)]
pub struct ApiSignature(pub(crate) internal::ThirtyTwoCharactersLowercaseHexAsciiString);
impl ApiSignature {
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
impl AsRef<str> for ApiSignature {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}
impl core::fmt::Display for ApiSignature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// <https://www.last.fm/api/show/auth.getMobileSession#Errors>
#[derive(Debug, thiserror::Error)]

pub enum SessionKeyThroughCredentialsError {
    #[error("network failure: {0}")]
    NetworkFailure(#[from] reqwest::Error),

    #[error("username or password is incorrect")]
    IncorrectCredentials,

    #[error("{0}")]
    General(#[from] crate::GeneralError)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountCredentials<'a> {
    /// The username (or email) of the user.
    pub username: &'a str,
    pub password: &'a str,
}
impl AccountCredentials<'_> {
    pub async fn generate_session_key(&self, client: &ClientIdentity) -> Result<SessionKey, SessionKeyThroughCredentialsError> {
        let signature = format!("{:x}", md5::compute(format!("api_key{}methodauth.getMobileSessionpassword{}username{}{}", client.key, self.password, self.username, client.secret)));
        let url = format!("{}?format=json&method=auth.getMobileSession&api_key={}&api_sig={signature}&username={}&password={}", crate::API_URL, client.key, self.username, self.password);
        let response = reqwest::Client::new().post(crate::API_URL)
            .header("Content-Length", "0")
            .header("User-Agent", &client.user_agent)
            .query(&[
                ("format", "json"),
                ("method", "auth.getMobileSession"),
                ("api_key", client.key.as_ref()),
                ("api_sig", &signature),
                ("username", self.username),
                ("password", self.password),
            ])
            .send().await?
            .text().await?;

        match serde_json::from_str(&response).expect("output could not be deserialized") {
            SessionKeyGenerationResponse::Ok { session } => Ok(session.key),
            SessionKeyGenerationResponse::Fail { code,  .. } => match code {
                4 => Err(SessionKeyThroughCredentialsError::IncorrectCredentials),
                _ =>  Err(SessionKeyThroughCredentialsError::General(GeneralError::try_from(code).expect("unknown error code")))
            }
        }
    }
}

pub(crate) mod internal {
    #[derive(Clone, PartialEq)]
    #[derive(thiserror::Error, Debug)]
    pub enum InvalidThirtyTwoCharacterAsciiStringError {
        #[error("invalid length: expected 32 characters, got {0}")]
        InvalidLength(usize),
        #[error("string was not ascii")]
        NotAscii
    }

    #[derive(Clone, PartialEq)]
    #[repr(transparent)]
    pub struct ThirtyTwoCharacterAsciiString([u8; Self::LENGTH]);
    impl ThirtyTwoCharacterAsciiString {
        pub const LENGTH: usize = 32;

        pub const fn new(str: &str) -> Result<Self, InvalidThirtyTwoCharacterAsciiStringError> {
            let len = str.len();
            if len != Self::LENGTH { return Err(InvalidThirtyTwoCharacterAsciiStringError::InvalidLength(len)) }
            if !str.is_ascii() { return Err(InvalidThirtyTwoCharacterAsciiStringError::NotAscii) }
            Ok(unsafe { Self::new_unchecked(str.as_bytes())} )
        }

        pub const unsafe fn new_unchecked(bytes: &[u8]) -> Self {
            Self(**core::mem::transmute::<&&[u8], &&[u8; Self::LENGTH]>(&bytes))
        }

        pub const fn as_str(&self) -> &str {
            unsafe { core::str::from_utf8_unchecked(&self.0) }
        }
    }
    impl core::fmt::Display for ThirtyTwoCharacterAsciiString {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str(self.as_str())
        }
    }
    impl core::fmt::Debug for ThirtyTwoCharacterAsciiString {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str(self.as_str())
        }
    }
    impl AsRef<str> for ThirtyTwoCharacterAsciiString {
        fn as_ref(&self) -> &str {
            self.as_str()
        }
    }
    impl serde::ser::Serialize for ThirtyTwoCharacterAsciiString {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: serde::ser::Serializer {
            serializer.serialize_str(self.as_str())
        }
    }
    impl<'de> serde::de::Deserialize<'de> for ThirtyTwoCharacterAsciiString {
        fn deserialize<D>(deserializer: D) -> Result<ThirtyTwoCharacterAsciiString, D::Error> where D: serde::de::Deserializer<'de> {
            struct Visitor;
            impl serde::de::Visitor<'_> for Visitor {
                type Value = ThirtyTwoCharacterAsciiString;

                fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                    formatter.write_str("an thirty-two character string")
                }

                fn visit_str<E>(self, str: &str) -> Result<Self::Value, E> where E: serde::de::Error, {
                    match Self::Value::new(str) {
                        Ok(value) => Ok(value),
                        Err(error) => Err(E::custom(error))
                    }
                }
            }

            deserializer.deserialize_str(Visitor)
        }
    }


    #[derive(thiserror::Error, Debug)]
    pub enum InvalidThirtyTwoCharactersLowercaseHexAsciiStringError {
        #[error("invalid length: expected 32 characters, got {0}")]
        InvalidLength(usize),
        #[error("bad byte: expected binary ascii for lowercase hex, got u8 of {0}")]
        BadCharacter(u8)
    }

    #[derive(Clone, PartialEq)]
    #[repr(transparent)]
    pub struct ThirtyTwoCharactersLowercaseHexAsciiString([u8; Self::LENGTH]);
    impl ThirtyTwoCharactersLowercaseHexAsciiString {
        pub const LENGTH: usize = 32;

        pub const fn new(str: &str) -> Result<Self, InvalidThirtyTwoCharactersLowercaseHexAsciiStringError> {
            let len = str.len();
            if len != Self::LENGTH { return Err(InvalidThirtyTwoCharactersLowercaseHexAsciiStringError::InvalidLength(len)) }
            let bytes = str.as_bytes();
            let mut i = 0;
            while i != Self::LENGTH {
                let byte = bytes[i];
                if !matches!(byte, b'0'..=b'9' | b'a'..=b'f') {
                    return Err(InvalidThirtyTwoCharactersLowercaseHexAsciiStringError::BadCharacter(byte));
                }
                i += 1;
            }
            Ok(unsafe { Self::new_unchecked(str.as_bytes())} )
        }

        pub const unsafe fn new_unchecked(bytes: &[u8]) -> Self {
            Self(**core::mem::transmute::<&&[u8], &&[u8; Self::LENGTH]>(&bytes))
        }

        pub const fn as_str(&self) -> &str {
            unsafe { core::str::from_utf8_unchecked(&self.0) }
        }
    }
    impl core::fmt::Display for ThirtyTwoCharactersLowercaseHexAsciiString {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str(self.as_str())
        }
    }
    impl core::fmt::Debug for ThirtyTwoCharactersLowercaseHexAsciiString {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str(self.as_str())
        }
    }
    impl AsRef<str> for ThirtyTwoCharactersLowercaseHexAsciiString {
        fn as_ref(&self) -> &str {
            self.as_str()
        }
    }
    impl serde::ser::Serialize for ThirtyTwoCharactersLowercaseHexAsciiString {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: serde::ser::Serializer {
            serializer.serialize_str(self.as_str())
        }
    }
    impl<'de> serde::de::Deserialize<'de> for ThirtyTwoCharactersLowercaseHexAsciiString {
        fn deserialize<D>(deserializer: D) -> Result<ThirtyTwoCharactersLowercaseHexAsciiString, D::Error> where D: serde::de::Deserializer<'de> {
            struct Visitor;
            impl serde::de::Visitor<'_> for Visitor {
                type Value = ThirtyTwoCharactersLowercaseHexAsciiString;

                fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                    formatter.write_str("an thirty-two character string of lowercase hex characters")
                }

                fn visit_str<E>(self, str: &str) -> Result<Self::Value, E> where E: serde::de::Error, {
                    match Self::Value::new(str) {
                        Ok(value) => Ok(value),
                        Err(error) => Err(E::custom(error))
                    }
                }
            }

            deserializer.deserialize_str(Visitor)
        }
    }
}