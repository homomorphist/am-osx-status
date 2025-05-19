use maybe_owned_string::MaybeOwnedString;

use crate::{accelerator::Directives, pool::Pool, read};

pub mod effect;
pub mod quality;

/// The image format to output.
/// 
/// ## Note on File Extensions
/// 
/// There are several file extensions which are accepted by Apple but
/// which end up being treated as a JPEG despite being indicative of
/// another format.
/// 
/// A list of these can be found in the [`Self::Jpg`] documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImageFormat {
    Webp,
    Png,
    /// ## Aliases
    /// 
    ///  - `jpeg`
    ///  - `rgb`
    ///  - `rgba`
    ///  - `sgi`
    ///  - `tif`
    ///  - `tiff`
    ///  - `ico` 
    /// 
    Jpg,
    /// [Layer Source Representation][overview] (`lsr`)
    /// 
    /// [overview]: https://developer.apple.com/library/archive/documentation/Xcode/Reference/xcode_ref-Asset_Catalog_Format/LSRFormatOverview.html
    LayeredImage
}
impl<'a> TryFrom<&'a str> for ImageFormat {
    type Error = ();
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        match value {
            "webp" => Ok(Self::Webp),
            "png" => Ok(Self::Png),
            "jpg" | "jpeg" => Ok(Self::Jpg),
            "rgb" | "rgba" | "sgi" | "tif" | "tiff" | "ico" => Ok(Self::Jpg),
            "lsr" => Ok(Self::LayeredImage),
            _ => Err(())
        }
    }
}
impl core::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", match self {
            ImageFormat::Webp => "webp",
            ImageFormat::Jpg => "jpg", // do we wanna represent those other fucked jpeg aliases
            ImageFormat::Png => "png",
            ImageFormat::LayeredImage => "lsr"
        })
    }
}


/// A representation of either the horizontal or vertical axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dimension {
    #[doc = "The horizontal axis."] X,
    #[doc = "The vertical axis."] Y
}
impl core::fmt::Display for Dimension {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", match self {
            Self::X => 'X',
            Self::Y => 'Y'
        })
    }
}





#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum DetailsParseError<'a> {
    #[error("{0}")]
    QualityOutOfBounds(#[from] quality::OutOfRangeError),
    #[error("could not parse quality: {0}")]
    QualityNotParsable(core::num::ParseIntError),

    #[error("{0}: \"{1}\"")]
    UnknownEffect(effect::UnknownEffectError, &'a str),

    #[error("bad resolution: {0} @ {1} dimension")]
    BadResolution(core::num::ParseIntError, Dimension),

    #[error("unsupported image format \"{0}\"")]
    UnsupportedImageFormat(&'a str),

    #[error("cannot find file extension delimiter")]
    MissingFileExtensionDelimiter,
    #[error("cannot find resolution dimension delimiter")]
    MissingResolutionDelimiter,
    #[error("unknown url parameter(s) present")]
    UnknownUrlParameter
}


/// The primary part of an mzstatic image URL.
/// 
/// This will roughly match the following RegEx: `^\d+x\d+(?:SC\.[A-Z]+[0-9]{2}|[a-z]{2})\.[a-z]+(?:\?l=[A-Za-z-]+)?$`
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Details<'a> {
    /// The file format to return.
    pub image_format: ImageFormat,
    /// An effect to apply on the image.
    pub effect: Option<effect::Effect>,
    /// The quality of the image; higher is better.
    /// Restricted to the range `[0, 999]` (zero to nine-hundred and ninety-nine, inclusive on both ends).
    pub quality: Option<quality::Quality>,
    /// The resolution of the image to return.
    // TODO: Document limitations, figure out if you can provide only one value to get with native aspect ratio.
    // ^ i figured that out :-] its three effects, two for one sid omitted, one for both. idk how to represent in type system
    pub resolution: (u16, u16),
    /// The language to use for the framing text, if provided.
    /// An optional [IETF language tag](https://en.wikipedia.org/wiki/IETF_language_tag), of varying levels of support.
    // Defaults to (and falls back to) English  for me; but does that differ depending on IP or something?
    // The following values worked alright: `ru-RU`, `ru`, `es-419`.
    // When used by Apple, it's usually in the `en-US` / `ru-RU` format.
    pub language: Option<MaybeOwnedString<'a>>
}
impl<'a> Details<'a> {
    pub fn edit_url(url: &'a str, edit: impl FnOnce(Details) -> Details) -> Result<String, DetailsParseError<'a>> {
        let last_slash: usize = url.rfind('/').unwrap();
        let image = edit(Details::new(&url[(last_slash + 1)..])?);
        Ok(format!("{}{image}", &url[..=last_slash]))
    }
    pub fn new(mut url: &'a str) -> Result<Self, DetailsParseError<'a>> {
        let resolution = {
            let x = read!(url, delimit: "x").ok_or(DetailsParseError::MissingResolutionDelimiter)?;    
            let y = read!(url, while: |char| char.is_ascii_digit());      

            (
                x.parse().map_err(|err| DetailsParseError::BadResolution(err, Dimension::X))?,
                y.parse().map_err(|err| DetailsParseError::BadResolution(err, Dimension::Y))?
            )
        };

        let framing_or_file_extension_delimiter = url.find('.').ok_or(DetailsParseError::MissingFileExtensionDelimiter)?;
        let maybe_quality_delimiter = url.find("-");

        // The effect, if present, will directly follow the resolution.
        // (So far, it's always been primarily specified by two characters, aside from a framing specifier)
        let effect = read!(url, delimit_at: maybe_quality_delimiter.unwrap_or(framing_or_file_extension_delimiter));
        let effect = if effect.is_empty() { None } else {
            Some(effect::Effect::try_from(effect).map_err(|e| DetailsParseError::UnknownEffect(e, effect))?)
        };

        let quality = if maybe_quality_delimiter.is_some() {
            // The modifier doesn't really have a delimiter, it's shoved in right after the Y-resolution.
            let quality = read!(url, while: |char| char.is_ascii_digit()).parse().map_err(DetailsParseError::QualityNotParsable)?;
            let quality = quality::Quality::new(quality).map_err(DetailsParseError::QualityOutOfBounds)?;
            url = &url[1..]; // Pass the file extension delimiter.
            Some(quality)
        } else { None };

        let maybe_parameters_delimiter = url.find("?");

        let (file_extension, language) = if let Some(parameters_delimiter) = maybe_parameters_delimiter {
            let file_extension = read!(url, delimit_at: parameters_delimiter);

            if &url.as_bytes()[0..=1] != b"l=" || url.find('&').is_some() {
                return Err(DetailsParseError::UnknownUrlParameter)
            }

            let language = MaybeOwnedString::Borrowed(&url["l=".len()..]);

            (file_extension, Some(language))
        } else { (url, None) };

        let image_format = ImageFormat::try_from(file_extension)
            .map_err(|_| DetailsParseError::UnsupportedImageFormat(file_extension))?;

        Ok(Self {
            language,
            quality,
            resolution,
            image_format,
            effect,
        })
    }
}
impl core::fmt::Display for Details<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}x{}",
            self.resolution.0,
            self.resolution.1
        )?;
        if let Some(effect) = &self.effect {
            write!(f, "{effect}")?;
        }
        if let Some(quality) = self.quality {
            write!(f, "-{quality}")?;
        }
        write!(f, ".{}", self.image_format)?;
        if let Some(language) = &self.language {
            write!(f, "?l={language}")?;
        }
        Ok(())
    }
}
impl Default for Details<'_> {
    fn default() -> Self {
        Self {
            image_format: ImageFormat::Png,
            quality: None,
            effect: None,
            resolution: (300, 300),
            language: None
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Prefix {
    /// - This section is present only in `/^is[1-5](?:-ssl)$/` subdomains. It seems to specify that a thumbnail is being retrieved.
    /// - If you remove this part of the URL and swap the subdomain to one satisfying `/^a[1-5]$/` whilst removing the thumbnail payload, it will return a lossless(?) version of the image.
    /// - It is always accompanied by a relevant thumbnail detail payload, except in the case of `/image/thumb/gen/`, which will be discussed later.
    ImageThumbnail,
    ImagePf, // ?
}
impl Prefix {
    pub(crate) fn read(input: &str) -> Option<super::Read<Prefix>> {
        if let Some(mut after) = input.strip_prefix("image/") {
            let read: &str = read!(after, delimit char: '/')?;
            Some(super::Read {
                bytes: unsafe { core::num::NonZeroUsize::new_unchecked("image/".len() + read.len() + '/'.len_utf8()) }, // For us not to return none in the below path it needs to be one of the valid prefixes, and all prefixes have a len > 0.
                value: match read {
                    "thumb" => Self::ImageThumbnail,
                    "pf" => Self::ImagePf,
                    _ => return None
                }
            })
        } else {
            None
        }
    }
    pub const fn to_str(&self) -> &'static str {
        match self {
            Self::ImageThumbnail => "image/thumb",
            Self::ImagePf => "image/pf",
        }
    }
}
impl core::fmt::Display for Prefix {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.to_str())
    }
}




#[derive(Debug)]
pub enum ParseError<'a> {
    BadImageParameters(Option<DetailsParseError<'a>>),
    BadDirectives(crate::accelerator::ReadError<'a>),
    BadPool(crate::pool::ParseError),
    BadDetails(DetailsParseError<'a>),
    BadProtocol,
    BadDomain,
    NoPool,
}
impl<'a> From<crate::accelerator::ReadError<'a>> for ParseError<'a> {
    fn from(value: crate::accelerator::ReadError<'a>) -> Self {
        Self::BadDirectives(value)
    }
}
impl<'a> From<DetailsParseError<'a>> for ParseError<'a> {
    fn from(value: DetailsParseError<'a>) -> Self {
        Self::BadDetails(value)
    }
}
impl From<crate::pool::ParseError> for ParseError<'_> {
    fn from(value: crate::pool::ParseError) -> Self {
        Self::BadPool(value)
    }
}





#[derive(Debug, Clone)]
pub enum PoolOrSagaSpecifier {
    Pool(crate::pool::Pool),
    /// Oh, lord. I'm gonna just have to try my best with this one.
    /// 
    /// On some very rare occasions, on old(?) files, they'll not have a pool (at least, I don't think this really counts as one...) but will have something like this:
    /// - <https://is1-ssl.mzstatic.com/image/thumb/SG-MQ-US-035-Image000001/v4/8d/46/70/8d467083-d1f9-a588-7a50-ff916291021f/image/600x600cc.jpg>
    /// - <https://is2-ssl.mzstatic.com/image/thumb/SG-S3-US-Std-Image-000001/v4/4b/57/4a/4b574a76-7ef8-5c16-b3a2-36a275e34851/image/500x500cc.jpg>
    /// 
    /// Now, "s3" to me pointed to Amazon S3, a data hosting service. MQ could refer to AmazonMQ, but that doesn't make too much sense.
    /// 
    /// "US" seems like some sort of region code, which reinforces that idea. Attempting to change to to something like "EU" gives us some sort of Spring Boot error.
    /// 
    /// After some Google-fu/dorking, I found "s3-us-std-102-prod-contentmover.prod-digitalhub.com.akadns.net". DigitalHub is another known Apple URL.
    /// Further digging let me to <https://www.thebeachcats.com/forums/viewtopic/topic/14766>. Here, you can see some image links to digitalhub with a similar format,
    /// but which also contain a ridiculous amount of base64-encoded data in the url. Decoding it, for whatever reason, seems to give some raw HTTP data of some sort.
    /// Within this is "us-std-00001.s3-external-1.amazonaws.com", which to me confirms that it's related to AWS.
    /// 
    /// I've concluded from this expedition that DigitalHub is the entrypoint for assets stored in external providers, or something..?
    /// 
    /// If you go through known subdomains for DigitalHub you'll see some other weird subdomains, which might be connected.
    /// But, also, that one I found earlier wasn't even a digitalhub.com domain, it was a weird prod-digitalhub.com.akadns.
    /// I saw some with a dot instead of a dash there, as well. AkaDNS is obviously just Akamai DNS, but, god damn. This shit is so fucking convoluted.
    ///  - <https://s3-eu-irl-105-prod.digitalhub.com/>
    ///  - <https://s3-us-nca-prod.prod-digitalhub.com.akadns.net>
    /// 
    /// Oh, maybe it just... only has dashes for the Akamai one, to "encode" it? I don't know, man.
    /// 
    /// More live-notes are in the "saga.txt". I'm calling this "Saga" because one error referred to this as a Saga Token.
    /// 
    /// Anyways, fuck all of this. I'm not touching it for a ten foot pole, at least for a while.
    Saga(String), // What's the connection to accelerator directive?
}
impl PoolOrSagaSpecifier {
    fn read(input: &str) -> Option<crate::Read<Self>> {
        let v = crate::pool::Pool::read(input).unwrap(); // cannot be arsed
        Some(crate::Read {
            bytes: v.bytes,
            value: Self::Pool(v.value)
        })
    }
}
impl core::fmt::Display for PoolOrSagaSpecifier {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Pool(pool) => write!(f, "{pool}"),
            Self::Saga(saga) => write!(f, "{saga}"),
        }
    }
}


#[derive(Debug, Clone)]
pub struct MzStaticImage<'a> {
    pub https: bool,
    pub accelerator_directives: Option<crate::accelerator::Directives<'a>>,
    pub pool: PoolOrSagaSpecifier,
    pub prefix: Option<Prefix>,
    pub asset_token: MaybeOwnedString<'a>,
    /// ### Known Subdomains
    /// 
    /// - `/^a[1-5]$/` - High-quality non-thumbnail image distribution.
    /// - `/^is[1-5](?:-ssl)$/` - Generated thumbnail or otherwise dynamically edited images.
    /// - `/^[rs]$/` - Purpose unknown; seen hosting HTML assets like icons in a PDF format for "da-storefront".
    /// - `/^s[1-5]$/` - Purpose unknown; seen hosting JSON data for the drone footage that scrolls in the background of an Apple TV. Not interchangeable with single-letter 's'.
    /// - `/^apps$/` - Purpose unknown; seen hosting an Android manifest and a (broken?) web build for some sort of Apple web-app.
    /// - `/^itc$/` - Purpose unknown.
    pub subdomain: MaybeOwnedString<'a>,
    pub parameters: Details<'a>
}
impl<'a> MzStaticImage<'a> {
    // todo: return result
    pub fn parse(mut url: &'a str) -> Result<Self, ParseError<'a>> {
        macro_rules! eat {
            ($str: ident, [assert] $literal: literal, $($reason: tt)*) => {
                if !$str.starts_with($literal) {
                    return Err(ParseError::$($reason)*);
                };
                $str = &$str[$literal.len()..];
            };
            ($str: ident, [optional] $literal: literal) => {
                {
                    let present = $str.starts_with($literal);
                    if present {
                        $str = &$str[$literal.len()..];
                    };
                    present
                }
            };
            ($str: ident, [strip] $literal: literal) => {
                {
                    let present = $str.starts_with($literal);
                    if present {
                        $str = &$str[$literal.len()..];
                    };
                    present
                }
            };
            ($str: ident, [pass] $expr: expr) => {
                {
                    if let Some(read) = $expr {
                        url = &url[read.bytes.get()..];
                        Some(read.value)
                    } else { None }
                }
            };
        }

        eat!(url, [assert] "http", BadProtocol);
        let tls = eat!(url, [optional] "s");
        eat!(url, [assert] "://", BadProtocol);

        // fixme
        let subdomain = read!(url, delimit: ".").unwrap().into();

        eat!(url, [assert] "mzstatic.com/", BadDomain);

        let prefix = eat!(url, [pass] Prefix::read(url));
        let directives = eat!(url, [pass] Directives::read(url)?);
        let pool =  eat!(url, [pass] Some(Pool::read(url)?)).unwrap(); // fixme
    
        let last_slash = url.rfind('/').unwrap();
        let (path, details) = url.split_at(last_slash);
        let details = Details::new(&details[1..])?;

        Ok(Self {
            https: tls,
            accelerator_directives: directives,
            asset_token: path.into(),
            subdomain,
            parameters: details,
            pool: PoolOrSagaSpecifier::Pool(pool),
            prefix
        })
    }

    pub fn with_pool_and_token(pool_and_token: MaybeOwnedString<'a>) -> Result<Self, ParseError<'a>> {
        if let Ok(pool) = Pool::read(&pool_and_token) {
            let token: MaybeOwnedString<'_> = match pool_and_token {
                MaybeOwnedString::Borrowed(borrowed) => MaybeOwnedString::Borrowed(&borrowed[pool.bytes.get() + '/'.len_utf8()..]),
                MaybeOwnedString::Owned(owned) => MaybeOwnedString::Owned((owned[pool.bytes.get() + '/'.len_utf8()..]).to_string())
            };
            Ok(Self {
                accelerator_directives: None,
                pool: PoolOrSagaSpecifier::Pool(pool.value),
                prefix: None,
                https: true,
                subdomain: MaybeOwnedString::Borrowed("a1"),
                asset_token: token,
                parameters: Details::default(),
            })
        } else {
            //idfk
            Err(ParseError::NoPool)
        }
    }
}
impl core::fmt::Display for MzStaticImage<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "http{}://", if self.https { "s" } else { "" })?;
        write!(f, "{}.mzstatic.com/", self.subdomain)?;
        if let Some(prefix) = self.prefix { write!(f, "{prefix}/")?; }
        if let Some(accelerator_directives) = self.accelerator_directives { write!(f, "{accelerator_directives}/")?; }
        write!(f, "{}/{}/{}", self.pool, self.asset_token, self.parameters)
    }
}




#[cfg(test)]
mod tests {
    use super::*;
    use quality::*;
    use effect::*;

    #[test]
    fn general() {
        assert_eq!(Details::new("600x600ac.jpg"), Ok(Details {
            image_format: ImageFormat::Jpg,
            quality: None,
            effect: Some(Effect::SquareFitCircle),
            resolution: (600, 600),
            language: None
        }));
    }

    #[test]
    fn complex() {
        assert_eq!(Details::new("3x401SC.FPESS03-159.jpg?l=ru-RU"), Ok(Details {
            image_format: ImageFormat::Jpg,
            quality: quality::Quality::new(159).ok(),
            effect: Some(Effect::Frame(Framing::FeaturedPlaylist(FeaturedPlaylist::Essentials(3)))),
            resolution: (3, 401),
            language: Some(MaybeOwnedString::Borrowed("ru-RU"))
        }));
    }

    // #[test]
    // fn edit() {
    //     const BASE: &str = "https://is1-ssl.mzstatic.com/image/thumb/AMCArtistImages126/v4/94/06/4d/94064d6b-c650-84a8-ae0a-bd3cf427898e/be14d48b-0f96-45d5-b15e-d255e87c48b6_ami-identity-795f9bb1320daa20b961333f6f8c6511-2023-08-17T07-24-42.519Z_cropped.png";
    //     assert_eq!(&MzStaticImageParameters::edit_url(&format!("{}/600x600.jpg", BASE), |mut jor| {
    //         jor.effect = Some(Effect::Frame(Framing::EssentialsFP(2)));
    //         jor.quality = Some(500);
    //         jor.image_format = ImageFormat::Png;
    //         jor.language = Some("ru");
    //         jor
    //     }).unwrap(), &format!("{}/600x600SC.FPESS02-500.png?l=ru", BASE));
    // }
}
