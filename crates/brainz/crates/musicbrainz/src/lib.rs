pub mod id;
pub mod entities;
pub use id::Id;

pub mod request_client;

#[derive(serde::Serialize)]
pub struct Tag<'a>(maybe_owned_string::MaybeOwnedString<'a>);

