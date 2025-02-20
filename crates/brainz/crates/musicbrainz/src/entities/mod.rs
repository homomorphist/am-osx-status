use super::id::{IdPossessor, IdSubject};

pub mod artist;
pub use artist::Artist;

pub mod alias;
pub use alias::Alias;

pub struct Release;
impl IdPossessor for Release { const VARIANT: IdSubject = IdSubject::Release; }

pub struct ReleaseGroup;
impl IdPossessor for ReleaseGroup { const VARIANT: IdSubject = IdSubject::ReleaseGroup; }

pub mod recording;
pub use recording::Recording;

pub struct Track;
impl IdPossessor for Track { const VARIANT: IdSubject = IdSubject::Track; }

pub struct Work;
impl IdPossessor for Work { const VARIANT: IdSubject = IdSubject::Work; }
