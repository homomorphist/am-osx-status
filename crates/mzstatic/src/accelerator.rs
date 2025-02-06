//! Reverse proxy parameters.


use super::Read;
use crate::read;

/// - [`Directives::r`]
/// - [`Directives::v`]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParseValueVariant {
    #[doc = "- [`Directives::r`]"] R,
    #[doc = "- [`Directives::v`]"] V
}
impl core::fmt::Display for ParseValueVariant {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::R => "r",
            Self::V => "v"
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReadError<'a> {
    BadNumeric {
        inner: core::num::ParseIntError,
        variant: ParseValueVariant
    },
    UnknownRegion(UnknownRegion<'a>),
    ExpectedDelimiterAfterR,
}
impl core::error::Error for ReadError<'_> {}
impl core::fmt::Display for ReadError<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BadNumeric { variant, .. } => write!(f, "parse-int error while reading {variant}-value"),
            Self::ExpectedDelimiterAfterR => write!(f, "expected delimiter after r-value"),
            Self::UnknownRegion(err) => write!(f, "{}", err)
        }
    }
}
impl<'a> From<UnknownRegion<'a>> for ReadError<'a> {
    fn from(value: UnknownRegion<'a>) -> Self {
        Self::UnknownRegion(value)
    }
}

/// ## Accelerator Directives
/// 
/// Presumably some sort of parameters for the "Accelerator" reverse proxy.
/// This can be removed without issue.
/// 
/// ### Accelerator
/// 
/// It seems to be an asset reverse proxy developed by Apple.
/// You can see an error for it by accessing <https://is1-ssl.mzstatic.com/>.
/// 
/// ### Format
/// ```txt
/// 
///         ┌─ Unknown "r" value.
///        ┌┴──┐
/// .../us/r1000/000/...
///     ├┘       └┬┘
///     └─ Region └─ Unknown "v" value.
/// ```
/// 
/// The region and "v" value are both optional, but an r-value will always be present.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Directives<'a>  {
    /// ### Region
    /// 
    /// This can be omitted in certain circumstances. <!-- When? -->
    /// 
    /// **Known values:**
    ///  - `us`
    ///  - `eu`
    ///  - `jp`
    ///  - `au`
    // TODO: Document what happens upon an invalid region.
    pub region: Option<Region>,
    /// ### Unknown "r" value.
    ///
    /// Of varying stability. Consistently present if any other accelerator directives are present; perhaps on the basis of being needed to distinguish the directives between other token path components?
    /// - <https://a3.mzstatic.com/us/r10/Video/v4/a0/d8/84/a0d88405-6a88-dcd7-e162-fb3cbe1aaa77/08E49_MLNA_EndOfWatch_tempart.jpg>
    ///   - Can seemingly only have an "r" value of 10 through 40.
    /// - <https://s1.mzstatic.com/us/r1000/000/Features/atv/AutumnResources/videos/entries.json>
    ///   - No other functional "r" values are known.
    /// - <https://s1.mzstatic.com/us/r1000/0/Music122/v4/c8/03/57/c803571e-6d17-f10f-fddf-fd4f7fc00d5e/22UMGIM37441.rgb.jpg>
    ///   - Usage of an "r" value of 30 results in an Akamai EdgeSuite error.
    pub r: u16,
    /// ### Unknown "n" value.
    /// 
    /// Of varying stability and presence.
    /// - <https://s1.mzstatic.com/us/r1000/000/Features/atv/AutumnResources/videos/entries.json>
    ///   - Any other values don't work. Not compared numerically, as "0" doesn't work.
    /// - <https://s1.mzstatic.com/us/r1000/0/Music122/v4/c8/03/57/c803571e-6d17-f10f-fddf-fd4f7fc00d5e/22UMGIM37441.rgb.jpg>
    ///   - Any other value **does** work.
    pub v: Option<&'a str>
}
impl<'a> Directives<'a> {
    // expected to pass contents after prefix (if present), no leading slash
    pub(crate) fn read(mut input: &'a str) -> Result<Option<Read<Directives<'a>>>, ReadError<'a>> {
        let start_ptr = input.as_ptr();

        macro_rules! read_r {
            ($in: ident, $r: ident) => {
                {
                    let r = $in.starts_with('r');
                    if r {
                        // i don't think theres a valid region that starts with 'r' but this feels icky
                        $in = &$in['r'.len_utf8()..];
                        let digits = read!($in, while: |char| char.is_ascii_digit());
                        $r = Some(digits.parse().map_err(|error| ReadError::BadNumeric {
                            inner: error,
                            variant: ParseValueVariant::R
                        })?);
                        if $in.as_bytes().get(0) != Some(&b'/') {
                            return Err(ReadError::ExpectedDelimiterAfterR)
                        }
                        $in = &$in['/'.len_utf8()..]
                    }
                    r
                }
            }
        }

        let mut r = None;

        let region = if !read_r!(input, r) {
            if let Some(region) = read!(input, delimit char: '/') {
                let region = Region::try_from(region);

                // idk how to handle this better
                if region.is_err() { return Ok(None) }
                let region = region.unwrap();

                read_r!(input, r);
                Some(region)
            } else { None }
        } else { None };

        let r = if let Some(r) = r { r } else { return Ok(None) };

        let mut bytes = unsafe { core::num::NonZeroUsize::new_unchecked(input.as_ptr().sub(start_ptr as usize) as usize) };

        let v = if let Some(after) = read!(input, delimit char: '/').filter(|s| s.chars().all(|char| char.is_ascii_digit())) {
            bytes = match bytes.checked_add('/'.len_utf8() + after.len()) {
                Some(read) => read,
                None => unsafe { core::hint::unreachable_unchecked() } // we'll always be <= size of `input`, and input is max len usize, so we won't go beyond that
            };
            Some(after)
        } else { None };

        Ok(Some(Read {
            bytes,
            value: Self {
                region,
                r, v
            }
        }))
    }
}
impl core::fmt::Display for Directives<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(region) = self.region {
            write!(f, "{region}/")?;
        }
        write!(f, "r{}/", self.r)?;
        if let Some(v) = self.v {
            write!(f, "{v}")?;
        }
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use std::num::NonZeroIsize;

    use super::*;

    #[test]
    fn bad_region() {
        assert_eq!(Directives::read("cn/r32/00"), Err(ReadError::UnknownRegion(UnknownRegion { region: "cn" })));
        // TODO: Figure out how I'd want to handle passing "ru".
    }

    #[test]
    fn no_after_r_value() {
        assert_eq!(Directives::read("r32"), Err(ReadError::ExpectedDelimiterAfterR));
        assert_eq!(Directives::read("au/r50"),    Err(ReadError::ExpectedDelimiterAfterR));
        assert_eq!(Directives::read("au/r50abc"), Err(ReadError::ExpectedDelimiterAfterR));
    }

    #[test]
    fn no_r_value_contents() {
        let no_r_num = Directives::read("au/r/30");
        assert!(matches!(no_r_num, Err(ReadError::BadNumeric { variant: ParseValueVariant::R, .. })));
        assert!({ if let Err(ReadError::BadNumeric { inner, .. }) = no_r_num { *inner.kind() == core::num::IntErrorKind::Empty } else { false } });
    }

    #[test]
    fn r_value() {
        assert_eq!(Directives::read("r32/"), Ok(Some(Read { bytes: core::num::NonZeroUsize::new(4).unwrap(), value: Directives {
            region: None,
            r: 32,
            v: None,
        }})));
    }

    #[test]
    fn r_value_and_region() {
        assert_eq!(Directives::read("au/r32/"), Ok(Some(Read { bytes: core::num::NonZeroUsize::new(7).unwrap(), value: Directives {
            region: Some(Region::AU),
            r: 32,
            v: None,
        }})));
        assert_eq!(Directives::read("au/r32/123"), Ok(Some(Read { bytes: core::num::NonZeroUsize::new(7).unwrap(), value: Directives {
            region: Some(Region::AU),
            r: 32,
            v: None, // no slash at the end to indicate terminated
        }})));
    }

    #[test]
    fn r_value_and_region_and_v_value() {
        assert_eq!(Directives::read("au/r32/123/"), Ok(Some(Read { bytes: core::num::NonZeroUsize::new(11).unwrap(), value: Directives {
            region: Some(Region::AU),
            r: 32,
            v: Some("123"),
        }})));
        assert_eq!(Directives::read("au/r32/not-numeric/"), Ok(Some(Read { bytes: core::num::NonZeroUsize::new(7).unwrap(), value: Directives {
            region: Some(Region::AU),
            r: 32,
            v: None,
        }})));

    }

    #[test]
    fn r_value_and_v_value() {
        assert_eq!(Directives::read("r32/not-numeric/"), Ok(Some(Read { bytes: core::num::NonZeroUsize::new(4).unwrap(), value: Directives {
            region: None,
            r: 32,
            v: None,
        }})));
        assert_eq!(Directives::read("r32/000/"), Ok(Some(Read { bytes: core::num::NonZeroUsize::new(8).unwrap(), value: Directives {
            region: None,
            r: 32,
            v: Some("000"),
        }})));
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
// TODO: Search to see if there are others.
pub enum Region {
    /// United States
    US,
    /// European Union
    EU,
    /// Japan
    JP,
    /// Australia
    AU,
}
impl Region {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(str: &str) -> Option<Self> {
        match str {
            "us" => Some(Self::US),
            "eu" => Some(Self::EU),
            "jp" => Some(Self::JP),
            "au" => Some(Self::AU),
            _ => None
        }
    }
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::US => "us",
            Self::EU => "eu",
            Self::JP => "jp",
            Self::AU => "au"
        }
    }
}
impl core::fmt::Display for Region {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.to_str())
    }
}
impl<'a> TryFrom<&'a str> for Region {
    type Error = UnknownRegion<'a>;
    fn try_from(region: &'a str) -> Result<Self, Self::Error> {
        Self::from_str(region).ok_or(Self::Error { region })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnknownRegion<'a> { pub region: &'a str }
impl core::error::Error for UnknownRegion<'_> {}
impl core::fmt::Display for UnknownRegion<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Unknown Accelerator region \"{}\"", self.region)
    }
}
