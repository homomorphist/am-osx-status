use serde::{Deserialize, Serialize};

/// Details about the program utilizing this library.
#[derive(Debug, Clone, PartialEq,  Serialize, Deserialize)]
pub struct ProgramInfo<S: AsRef<str>> {
    pub name: S,
    pub version: Option<S>,
    /// Contact information for placement in the User-Agent for requests.
    /// - <https://wiki.musicbrainz.org/MusicBrainz_API/Rate_Limiting#Provide_meaningful_User-Agent_strings>
    pub contact: S,
}

impl<S: AsRef<str>> ProgramInfo<S> {
    pub fn to_user_agent(&self) -> String {
        let capacity = self.name.as_ref().len()
            + self.version.as_ref().map(|v| v.as_ref().len() + "/".len()).unwrap_or(0)
            + " (".len() + self.contact.as_ref().len() + ")".len();
        let mut out = String::with_capacity(capacity);
        out += self.name.as_ref();
        if let Some(version) = &self.version {
            out += version.as_ref();
        }
        out += " (";
        out += self.contact.as_ref();
        out += ")";
        out
    }
}


