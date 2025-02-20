use serde::{Deserialize, Serialize};

/// - <https://musicbrainz.org/doc/Aliases>
/// - <https://musicbrainz.org/doc/MusicBrainz_API/Search>
#[derive(Serialize, Deserialize)]
pub struct Alias {
    locale: Option<String>, // TODO: Enumerate?
    sort_name: String,
    name: String,
    #[serde(default)]
    primary: bool,
    r#type: Option<String>, // TODO: Find an alternative property name.
    // TODO: Date range.
}
