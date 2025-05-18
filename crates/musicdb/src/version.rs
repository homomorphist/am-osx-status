#[derive(thiserror::Error, Debug)]
pub enum VersionParseError {
    #[error("too many version components")]
    TooManyComponents,
    #[error("could not parse component: {0}")]
    ParseFailure(#[from] core::num::ParseIntError),
    #[error("not enough components")]
    NotEnoughComponents
}


/// A specific version of a MacOS Apple Music installation.
/// It possesses four components: major, minor, patch and revision.
/// Exact semantics are unknown; assumptions of SemVer were made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppleMusicVersion {
    pub major: u8,
    pub minor: u16,
    pub patch: u16,
    pub revision: u32
}
impl core::str::FromStr for AppleMusicVersion {
    type Err = VersionParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut major: Option<u8> = None;
        let mut minor: Option<u16> = None;
        let mut patch: Option<u16> = None;
        let mut revision: Option<u32> = None;
        for (i, component) in s.split_terminator('.').enumerate() {
            match i {
                0 => major = Some(component.parse()?),
                1 => minor = Some(component.parse()?),
                2 => patch = Some(component.parse()?),
                3 => revision = Some(component.parse()?),
                _ => return Err(VersionParseError::TooManyComponents)
            }
        };
        let major = major.ok_or(VersionParseError::NotEnoughComponents)?;
        let minor = minor.ok_or(VersionParseError::NotEnoughComponents)?;
        let patch = patch.ok_or(VersionParseError::NotEnoughComponents)?;
        let revision = revision.ok_or(VersionParseError::NotEnoughComponents)?;
        Ok(Self {
            major,
            minor,
            patch,
            revision
        })
    }
}
impl core::fmt::Display for AppleMusicVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}.{}", 
            self.major,
            self.minor,
            self.patch,
            self.revision
        )
    }
}
impl core::cmp::PartialOrd for AppleMusicVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl core::cmp::Ord for AppleMusicVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major.cmp(&other.major)
            .then_with(|| self.minor.cmp(&self.minor))
            .then_with(|| self.patch.cmp(&self.patch))
            .then_with(|| self.revision.cmp(&self.revision))
    }
}
