pub mod id;
pub use id::Id;
use id::*;


pub mod request_client;

#[derive(serde::Serialize)]
pub struct Tag<'a>(maybe_owned_string::MaybeOwnedString<'a>);


pub struct Artist;
impl IdPossessor for Artist { const VARIANT: IdSubject = IdSubject::Artist; }

pub struct Release;
impl IdPossessor for Release { const VARIANT: IdSubject = IdSubject::Release; }

pub struct ReleaseGroup;
impl IdPossessor for ReleaseGroup { const VARIANT: IdSubject = IdSubject::ReleaseGroup; }

pub struct Recording;
impl IdPossessor for Recording { const VARIANT: IdSubject = IdSubject::Recording; }

pub struct Track;
impl IdPossessor for Track { const VARIANT: IdSubject = IdSubject::Track; }

pub struct Work;
impl IdPossessor for Work { const VARIANT: IdSubject = IdSubject::Work; }

