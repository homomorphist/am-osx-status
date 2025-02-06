#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    UnknownVariant,
    DidNotTerminate,
    BadNumber(core::num::ParseIntError)
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Pool {
   pub variant: Variant,
   pub number: Option<core::num::NonZeroU8>
}
impl Pool {
    pub(crate) fn read(input: &str) -> Result<super::Read<Self>, ParseError>  {
        let mut no_digit = false;
        let stop = input.chars().enumerate().find(|(_, v)| v.is_ascii_digit() || {
            no_digit = v == &'/';
            no_digit
        }).map(|v| v.0).ok_or(ParseError::DidNotTerminate)?;
        Ok(if no_digit {
            super::Read {
                bytes: unsafe { core::num::NonZeroUsize::new_unchecked(stop + '/'.len_utf8()) },
                value: Self {
                    variant: Variant::from_str(&input[0..stop]).ok_or(ParseError::UnknownVariant)?,
                    number: None
                }
            }
        } else {
            let slash = stop + input[stop..].chars().enumerate().find(|(_, v)| v == &'/').ok_or(ParseError::DidNotTerminate)?.0;
            let number = input[stop..slash].parse().map_err(ParseError::BadNumber)?;
            super::Read {
                bytes: unsafe { core::num::NonZeroUsize::new_unchecked(slash + '/'.len_utf8()) },
                value: Self {
                    variant: Variant::from_str(&input[0..stop]).ok_or(ParseError::UnknownVariant)?,
                    number: Some(number)
                }
            }
        })
    }
}
impl core::fmt::Display for Pool {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.variant.to_str())?;
        if let Some(number) = self.number {
            write!(f, "{number}")?
        };
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum Variant {
    AppStore,
    AppStoreSource,
    Podcasts,
    Features,
    VideoHLS,
    Video,
    MusicArtistImages, // https://mvod.itunes.apple.com/itunes-assets/HLSVideo122/v4/98/d0/92/98d09281-61a6-f026-1363-cddb7a140488/P817585636_default.m3u8
    Music,
    Books,
    FuseSocial, // I saw this on an Apple Music behind the scenes video, I think.
    CobaltPublic, // Educational platform data..? (iTunes U?)
    
}
impl Variant {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(input: &str) -> Option<Self> {
        match input {
            "Purple" => Some(Self::AppStore),
            "PurpleSource" => Some(Self::AppStoreSource),
            "Podcasts" => Some(Self::Podcasts),
            "Features" => Some(Self::Features),
            "HLSVideo" => Some(Self::VideoHLS),
            "Video" => Some(Self::Video),
            "AMCArtistImages" => Some(Self::MusicArtistImages),
            "Music" => Some(Self::Music),
            "Publication" => Some(Self::Books),
            "FuseSocial" => Some(Self::FuseSocial),
            "CobaltPublic" => Some(Self::CobaltPublic),
            _ => None
        }
    }
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::AppStore => "Purple",
            Self::AppStoreSource => "PurpleSource",
            Self::Podcasts => "Podcasts",
            Self::Features => "Features",
            Self::VideoHLS => "HLSVideo",
            Self::Video => "Video",
            Self::MusicArtistImages => "AMCArtistImages",
            Self::Music => "Music",
            Self::Books => "Publication",
            Self::FuseSocial => "FuseSocial",
            Self::CobaltPublic => "CobaltPublic",
        }
    }
}
impl core::fmt::Display for Variant {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.to_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        use core::num::NonZero;
        assert_eq!(Pool::read("Music/"), Ok(crate::Read { bytes: NonZero::new(6).unwrap(), value: Pool { variant: Variant::Music, number: None }}));
        assert_eq!(Pool::read("Music4/"), Ok(crate::Read { bytes: NonZero::new(7).unwrap(), value: Pool { variant: Variant::Music, number: Some(NonZero::new(4).unwrap()) }}));
    }
}
