#[derive(Debug, thiserror::Error)]
pub enum Error<T: code::ErrorCode = GeneralErrorCode> {
    /// Error codes returned by the Last.fm API.
    #[error("{0}")]
    ApiError(#[from] T),
    /// An error occurred while sending the request.
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    /// An error occurred while deserializing the response.
    /// This is an internal error and should be reported as a bug.
    #[error("deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
}
impl<T: code::ErrorCode> TryInto<u8> for &Error<T> {
    type Error = ();
    fn try_into(self) -> Result<u8, Self::Error> {
        match self {
            Error::ApiError(code) => Ok((*code).into()),
            _ => Err(()),
        }
    }
}
impl<T: code::ErrorCode> Error<T> {
    /// Return an error from a general last.fm API error code if it is recognized.
    fn try_from_code(code: u8) -> Result<Self, code::UnmappedErrorCode> {
        T::try_from(code).map(Error::ApiError)
    }
    /// Return an error from a general last.fm API error code, or an error representing that the code itself is unrecognized.
    fn from_code(code: u8) -> Self {
        Self::try_from_code(code).unwrap_or_else(|err| Error::Deserialization(err.into()))
    }
}
impl<T: code::ErrorCode> From<u8> for Error<T> {
    fn from(code: u8) -> Self {
        Self::from_code(code)
    }
}


pub use code::GeneralErrorCode;
/// Error codes returned by the Last.fm API.
/// Station-related error codes are omitted, since stations themselves have been deleted.
/// <https://www.last.fm/api/errorcodes>
pub mod code {
    /// Implemented for Last.fm API error codes.
    /// Allows for [`core::convert::Into`] conversions to [`Error`] without overlapping implements on generics.
    pub trait ErrorCode:
        std::error::Error
        + core::fmt::Debug
        + Clone
        + Copy
        + Into<u8>
        + TryFrom<u8, Error = UnmappedErrorCode>
    {}


    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct UnmappedErrorCode {
        /// The error code that wasn't mapped to a variant.
        pub code: u8
    }
    impl From<u8> for UnmappedErrorCode {
        fn from(code: u8) -> Self {
            UnmappedErrorCode { code }
        }
    }
    impl From<UnmappedErrorCode> for u8 {
        fn from(unmapped: UnmappedErrorCode) -> u8 {
            unmapped.code
        }
    }
    impl std::error::Error for UnmappedErrorCode {}
    impl core::fmt::Display for UnmappedErrorCode {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "unmapped response error code: {}", self.code)
        }
    }
    impl From<UnmappedErrorCode> for serde_json::Error {
        fn from(val: UnmappedErrorCode) -> Self {
            serde::de::Error::custom(val)
        }
    }
    impl<T: ErrorCode> From<UnmappedErrorCode> for crate::Error<T> {
        fn from(code: UnmappedErrorCode) -> Self {
            crate::Error::Deserialization(code.into())
        }
    }


    /// General error codes returned by the Last.fm API.
    /// This doesn't encompass every possible error; some variants which are only possible  on specific endpoints are omitted.
    #[derive(Debug, thiserror::Error, PartialEq, Eq, Clone, Copy)]
    #[repr(u8)]
    pub enum GeneralErrorCode {
        #[error("{0}")]
        ServiceAvailability(#[from] general::ServiceAvailability),
        #[error("{0}")]
        Authentication(#[from] general::Authentication),
        #[error("{0} (library error; please report)")]
        InvalidUsage(#[from] general::InvalidUsage),
        /// Your IP has made too many requests in a short period.
        #[error("rate limit exceeded")]
        RateLimitExceeded = 29,
    }

    impl ErrorCode for GeneralErrorCode {}
    impl TryFrom<u8> for GeneralErrorCode {
        type Error = UnmappedErrorCode;
        fn try_from(code: u8) -> Result<Self, Self::Error> {
            general::ServiceAvailability::try_from(code).map(GeneralErrorCode::ServiceAvailability)
                .or_else(|_| general::Authentication::try_from(code).map(GeneralErrorCode::Authentication))
                .or_else(|_| general::InvalidUsage::try_from(code).map(GeneralErrorCode::InvalidUsage))
                .or(match code { 29 => Ok(GeneralErrorCode::RateLimitExceeded), _ => Err(UnmappedErrorCode { code} ), })
        }
    }
    impl From<GeneralErrorCode> for u8 {
        fn from(code: GeneralErrorCode) -> u8 {
            match code {
                GeneralErrorCode::ServiceAvailability(code) => code as u8,
                GeneralErrorCode::Authentication(code) => code as u8,
                GeneralErrorCode::InvalidUsage(code) => code as u8,
                GeneralErrorCode::RateLimitExceeded => 29,
            }
        }
    }

    #[macro_export]
    macro_rules! using_destructure {
        (_, $($tt:tt)*) => { _ };
        ($name: ident, $($tt:tt)*) => { $name };
    }

    #[macro_export]
    macro_rules! def {
        (
            $(
                $(#[$meta:meta])* $vis:vis enum $name:ident {
                    $(
                        $(#[$variant_meta:meta])*
                        $variant:ident
                            $(= $lit:literal)?
                            $(($(#[from] $__:vis)? $flatten:ty))?
                            $(,)?
                    )*
                }
            )*
        ) => {
            $(
                $(#[$meta])*
                #[derive(Debug, thiserror::Error, PartialEq, Eq, Clone, Copy)]
                #[repr(u8)]
                $vis enum $name {
                    $(
                        $(#[$variant_meta])*
                        $variant$(($(#[from] $__)? $flatten))?$(= $lit)?
                    ),*
                }
                impl $crate::error::code::ErrorCode for $name {}
                impl TryFrom<u8> for $name {
                    type Error = $crate::error::code::UnmappedErrorCode;
                    fn try_from(value: u8) -> Result<Self, Self::Error> {
                        // Can't make this a singular `match` expression until `if_let_guard` feature gets stabilized.
                        match value {
                            $(
                                $($lit => return Ok($name::$variant),)?
                            )*
                            _ => ()
                        };

                        $(
                            $(if let Ok(code) = <$flatten>::try_from(value) {
                                return Ok($name::$variant(code));
                            })?
                        )*


                        Err($crate::error::code::UnmappedErrorCode { code: value })
                    }
                }
                impl From<$name> for u8 {
                    fn from(code: $name) -> u8 {
                        match code {
                            $(
                                $name::$variant$(($crate::error::code::using_destructure!(v, $flatten)))? => 
                                    $($lit)?
                                    $(u8::from($crate::error::code::using_destructure!(v, $flatten)))?,
                            )*
                        }
                    }
                }
                impl PartialEq<u8> for $name {
                    fn eq(&self, other: &u8) -> bool {
                        u8::from(*self) == *other
                    }
                }
                impl PartialEq<$crate::Error> for $name {
                    fn eq(&self, other: &$crate::Error) -> bool {
                        Ok(u8::from(*self)) == TryInto::<u8>::try_into(other)
                    }
                }
                impl PartialEq<$crate::error::GeneralErrorCode> for $name {
                    fn eq(&self, other: &$crate::error::GeneralErrorCode) -> bool {
                        u8::from(*self) == u8::from(*other)
                    }
                }
                impl PartialEq<$name> for $crate::Error {
                    fn eq(&self, other: &$name) -> bool {
                        TryInto::<u8>::try_into(self) == Ok(u8::from(*other))
                    }
                }
                impl PartialEq<$name> for $crate::error::GeneralErrorCode {
                    fn eq(&self, other: &$name) -> bool {
                        u8::from(*self) == u8::from(*other)
                    }
                }
            )*
        };
    }

    pub(crate) use using_destructure;
    pub(crate) use def;

    macro_rules! into_superficial {
        ($($name:ident),*) => {
            $(
                impl From<$name> for $crate::Error {
                    fn from(code: $name) -> Self {
                        $crate::Error::ApiError(super::GeneralErrorCode::$name(code))
                    }
                }
            )*
        };
    }

    /// General error codes returned by the Last.fm API that could be applicable to many endpoints.
    /// This doesn't encompass every possible error; some variants which are only possible  on specific endpoints are omitted.
    pub mod general {
        def! {
            /// Errors that indicate the Last.fm service is unavailable.
            pub enum ServiceAvailability {
                /// Most likely the backend service failed. Please try again.
                #[error("operation failed; please try again")]
                OperationFailed = 8,
                
                /// This service is temporarily offline. Try again later.
                #[error("service offline")]
                ServiceOffline = 11,

                /// There was a temporary error processing your request. Please try again.
                // "The service is temporarily unavailable, please try again."
                #[error("temporarily unavailable")]
                TemporaryError = 16  
            }
            
            /// Errors that indicate a problem with authentication or authorization.
            pub enum Authentication {
                /// You do not have permissions to access the service.
                #[error("authentication failed; lacking permissions")]
                AuthenticationFailed = 4,
        
                /// Please re-authenticate.
                #[error("invalid session key")]
                InvalidSessionKey = 9,
        
                /// You must be granted a valid key by last.fm.
                #[error("invalid API key")]
                InvalidApiKey = 10,
        
                /// Access for your account has been suspended, please contact Last.fm.
                #[error("suspended API key")]
                SuspendedApiKey = 26,
            }

            /// Errors that indicate this library isn't correctly interacting with the Last.fm API.
            /// If encountered, please report them as bugs.
            pub enum InvalidUsage {
                /// The service does not exist.
                #[error("invalid service")]
                InvalidService = 2,

                /// No method with that name in this package.
                #[error("invalid method")]
                InvalidMethod = 3,

                /// This service doesn't exist in that format.
                #[error("invalid format")]
                InvalidFormat = 5,

                /// Your request is missing a required parameter.
                #[error("invalid parameters")]
                InvalidParameters = 6,

                /// Invalid resource specified.
                #[error("invalid resource")]
                InvalidResource = 7,

                /// Invalid method signature supplied.
                #[error("invalid method signature")]
                InvalidMethodSignature = 13,

                /// This type of request is no longer supported.
                #[error("deprecated")]
                Deprecated = 27,
            }
        }

        into_superficial! {
            ServiceAvailability,
            Authentication,
            InvalidUsage
        }
    }
}