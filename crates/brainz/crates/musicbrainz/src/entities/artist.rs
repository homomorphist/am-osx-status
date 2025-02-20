use serde::{Deserialize, Serialize};
use crate::id::{IdPossessor, IdSubject};

// Incomplete.
#[derive(Serialize, Deserialize)]
pub struct Artist {
    /// The MusicBrainz ID of the artist.
    pub id: crate::Id<Self>,

    /// The official name of the artist.
    pub name: String,

    /// The name of the artist in a format meant for sorting.
    pub sort_name: String,
    
    /// The gender that the artist (if singular) identifies with.
    /// Not present for groups.
    pub gender: Option<Gender>,
}
impl IdPossessor for Artist {
    const VARIANT: IdSubject = IdSubject::Artist;
}

/// The type of artist.
pub enum Type {
    /// An individual person.
    Person,
    /// A group of people that may or may not have a distinct name.
    Group,
    /// A large instrumental ensemble.
    Orchestra,
    /// A large vocal ensemble.
    Choir,
    /// A fictitious individual character.
    Character,
    /// Anything which does not fit into the above categories.
    Other,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Gender {
    Male,
    Female,
    NonBinary
}
impl From<&str> for Gender {
    fn from(s: &str) -> Self {
        if s.eq_ignore_ascii_case("male") { return Self::Male };
        if s.eq_ignore_ascii_case("female") { return Self::Female };
        Self::NonBinary
    }
}

pub mod credit {
    use super::*;

    /// <https://musicbrainz.org/doc/Artist_Credits>
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct Individual {
        pub id: crate::Id<Artist>,
        pub name: String,
        pub sort_name: String,
    }
    
    /// <https://musicbrainz.org/doc/Artist_Credits>
    #[derive(Serialize, Deserialize)]
    pub struct Credited {
        pub name: Option<String>,
        pub artist: Individual,
        #[serde(rename = "joinphrase")]
        pub join_phrase: Option<String>,
    }

    pub type List = Vec<Credited>;
}


