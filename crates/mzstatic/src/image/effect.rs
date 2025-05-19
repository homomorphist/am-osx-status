
// https://music.apple.com/us/playlist/todays-chill/pl.2bb29727dbc34a63936787297305c37c


// todo: make the terminology ("discriminator", "variant" consistent in framing)
// todo: further research into numbers


use crate::read;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnknownEffectError;
impl core::fmt::Display for UnknownEffectError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "unknown effect")
    }
}
impl core::error::Error for UnknownEffectError {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Effect {
    /// Pad the image so that when it is displayed in a circle, the image can be viewed as a square (with rounded corners).
    /// Will not work correctly for images greater than 2000x2000. The region not in a circle will have blurred colors sampled from the image.
    /// 
    /// Literal representation: "ac".
    // TODO: Test & document behavior on non-square images.
    SquareFitCircle,
    Frame(Framing),
    MusicVideo, // = mv ; could be "music video" ??
    BackgroundBlur, // ? = bb
    BackgroundFill, // ? = bf


    // mv => music video ?
    // sr => irrelevant (used at home)
    // ac => pad to square to fit in circle with blurred color around where doesnt fit
    // bf => background fill?
    // bb => could be background blur but i haven't seen it in high res or anything that matters. only typically used 80x80 or 220x220 (search results)
    // mv => used in music videos. could maybe sstand for movie for all i know
    // sr => used in wide editorial cards on homepage also this https://is1-ssl.mzstatic.com/image/thumb/Features/v4/a5/fc/4b/a5fc4bf1-aecc-538a-6c11-a181dd8e93a2/cb81b7ec-9b6f-4795-8c91-f80f6016809e.png/220x220sr.webp
    // sc => ? seen on podcasts, user icons
    // cc => ? will force to square
    // vf => real wide. king crimson banner https://is1-ssl.mzstatic.com/image/thumb/Features125/v4/30/b8/fc/30b8fc23-fc6d-8006-fabd-b265b9c5a180/mzl.wnqeoeqa.jpg/2400x933vf-60.jpg
    // ea => real wide. kendrick lamar banner https://is1-ssl.mzstatic.com/image/thumb/Features122/v4/3a/64/ae/3a64aedb-3b2a-3eb0-74de-6018eb900fc5/fd0d1a37-3179-4978-ba51-1bae0fc6e993.png/2400x933ea-60.jpg

}
impl Effect {
    /// Returns a two-character representation of the transformation variant to apply to the asset.
    pub fn variant(&self) -> &'static str {
        match self {
            Self::SquareFitCircle => "ac",
            Self::MusicVideo => "mv",
            Self::BackgroundBlur => "bb",
            Self::BackgroundFill => "bf",
            Self::Frame(..) => "SC"
        }
    }
}
impl<'a> TryFrom<&'a str> for Effect {
    type Error = UnknownEffectError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        match value {
            "ac" => Ok(Self::SquareFitCircle),
            "mv" => Ok(Self::MusicVideo),
            "bb" => Ok(Self::BackgroundBlur),
            "bf" => Ok(Self::BackgroundFill),
            _ if &value[0..=1] == "SC" => {
                // todo PDCXS
                // todo can out of bounds in [3..]?
                Ok(Self::Frame(Framing::try_from(&value[3..]).unwrap())) // todo handle error
            }
            _ => Err(UnknownEffectError)
        }
    }
}
impl core::str::FromStr for Effect {
    type Err = UnknownEffectError;

    fn from_str(str: &str) -> Result<Self, Self::Err> {
        Self::try_from(str)
    }
}
impl core::fmt::Display for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let variant = self.variant();
        if let Self::Frame(frame) = self {
            write!(f, "{variant}.{frame}")
        } else {
            write!(f, "{variant}")
        }
    }
}


#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum FramingParseError<'a> {
    #[error("unknown frame type \"{0}\"")]
    UnknownFrame(&'a str),
    #[error("frame variant parse failure: {0}")]
    VariantParseError(#[from] core::num::ParseIntError)
}

// SC 
//   - FPESS "FP" = ?, "ESS" = "ESSENTIALS"
//     - FPESS01 | FPESS02 => "ESSENTIALS" at top in monospace, white border.
//     - FPESS03 => "ESSENTIALS" at top in isolated section,
//     - FPESS04 => "ESSENTIALS" at top in isolated section, smaller text & larger border
//   - CAESS "CA" = ?, "ESS" = "ESSENTIALS"
//     - CAESS01 => conventional
//     - CAESS02 => conventional, no apple music logo
//   - CAHGOY => ?????? works fine for the essentials
//     - CAHGOY01

// FPESS03, CAHGOY01, DN01, FPMAF01, CAESS02 => all irrelevant i think


// - MVESS04 video essentials https://is1-ssl.mzstatic.com/image/thumb/Features112/v4/ab/50/ef/ab50ef3b-c936-44f0-7b01-bb113a133547/mza_5527770582281912184.png/632x632SC.MVESS04.webp?l=en-US
// FPMAF01 - very weird idk if it even actually does anything. might just add the logo
// - https://is1-ssl.mzstatic.com/image/thumb/Features116/v4/6c/a1/67/6ca167a2-3345-fb31-6399-f73c531088ec/8efea0a6-8ce9-414c-b0b5-c54f9679f09a.png/632x632SC.FPMAF01.webp?l=en-US
// - https://is1-ssl.mzstatic.com/image/thumb/Features116/v4/3c/3c/33/3c3c330e-b8e7-146b-96cd-cb532e07b097/U0MtTVMtV1ctRml0bmVzc19QbHVzLVRoZV9CZWF0bGVzLUFEQU1fSUQ9MTU5NTIyMTQyNi5wbmc.png/632x632SC.FPMAF01.webp?l=en-US
// usually 03, only saw 02 in chill and sing?

// TODO: Make enum for sub-variants after I experiment with all the behavior.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FeaturedPlaylist {
    Essentials(u8),
    EssentialsClassical(u8),
    InspiredBy(u8), 
    DeepCuts(u8),
    Influences(u8),
    Live(u8),
    Chill(u8),
    Flipped(u8),
    Sing(u8),
    LoveSongs(u8),
    Sampled(u8),
    TheSongwriters(u8),
    SetList(u8),
    Undiscovered(u8),
}
impl FeaturedPlaylist {
    pub const PREFIX: &'static str = "FP";

    /// Returns the static discriminator *without* any prefix.
    pub const fn static_str(&self) -> &'static str {
        match self {
            Self::Essentials(..) => "ESS",
            Self::EssentialsClassical(..) => "ESSC",
            Self::InspiredBy(..) => "INS",
            Self::DeepCuts(..) => "DC",
            Self::Influences(..) => "INF",
            Self::Live(..) => "LIVE",
            Self::Chill(..) => "CHL",
            Self::Flipped(..) => "FLIP",
            Self::Sing(..) => "SING",
            Self::LoveSongs(..) => "LS",
            Self::Sampled(..) => "SAMP",
            Self::TheSongwriters(..) => "TSW",
            Self::SetList(..) => "SL",
            Self::Undiscovered(..) => "UD",
        }
    }

    pub const fn get_variant_number(&self) -> u8 {
        *match self {
            Self::Essentials(n) => n,
            Self::EssentialsClassical(n) => n,
            Self::InspiredBy(n) => n,
            Self::DeepCuts(n) => n,
            Self::Influences(n) => n,
            Self::Live(n) => n,
            Self::Chill(n) => n,
            Self::Flipped(n) => n,
            Self::Sing(n) => n,
            Self::LoveSongs(n) => n,
            Self::Sampled(n) => n,
            Self::TheSongwriters(n) => n,
            Self::SetList(n) => n,
            Self::Undiscovered(n) => n
        }
    }

    pub fn from_deconstructed(variant: &str, number: u8) -> Option<Self> {
        match variant {
            "ESS" => Some(Self::Essentials(number)),
            "ESSC" => Some(Self::EssentialsClassical(number)),
            "INS" => Some(Self::InspiredBy(number)),
            "DC" => Some(Self::DeepCuts(number)),
            "INF" => Some(Self::Influences(number)),
            "LIVE" => Some(Self::Live(number)),
            "CHL" => Some(Self::Chill(number)),
            "FLIP" => Some(Self::Flipped(number)),
            "SING" => Some(Self::Sing(number)),
            "LS" => Some(Self::LoveSongs(number)),
            "SAMP" => Some(Self::Sampled(number)),
            "TSW" => Some(Self::TheSongwriters(number)),
            "SL" => Some(Self::SetList(number)),
            "UD" => Some(Self::Undiscovered(number)),
            _ => None
        }
    }
}
impl core::fmt::Display for FeaturedPlaylist {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "FP{}{:02}", self.static_str(), self.get_variant_number())
    }
}


// https://is1-ssl.mzstatic.com/image/thumb/Features116/v4/6d/4c/1e/6d4c1e44-aed2-6225-8fc2-9e24c16b30ff/4f8fa624-f639-45c7-80da-f2d09842adc4.png/296x296SC.DNC01.webp
// - https://music.apple.com/us/playlist/hilary-hahn-violin-mixtape/pl.12d48043c3964c1289cb1d8fdebe83d4
// https://is1-ssl.mzstatic.com/image/thumb/Features126/v4/eb/2d/d6/eb2dd633-e015-ae41-e758-7ce3f9589a97/4f7449ed-4983-4b54-836c-7f9539123470.png/296x296SC.DNC01.webp


// SC.{effect} = cube
// SH.{effect} = wide? looks wacky even after adjusting resolution
// https://is1-ssl.mzstatic.com/image/thumb/Features116/v4/78/4e/b1/784eb1dc-50df-632a-c7f0-8928d54dc070/mza_4913277387679559953.png/300x172SH.FPUD02.webp?l=en-US
// https://is1-ssl.mzstatic.com/image/thumb/Features211/v4/fa/c9/88/fac98880-913c-b62e-48cb-7048801789fa/mza_1932705350931991483.png/296x296SC.FPESSC02.webp?l=en-US 

// TODO: Make enum for sub-variants after I experiment with all the behavior.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FeaturedCategory {
    // 02 => bottom left
    //  https://is1-ssl.mzstatic.com/image/thumb/Features/v4/82/89/43/828943e4-6e29-5aff-cb7d-b83853581163/e8475a08-6ffe-4216-86ca-151eeda0d51d.png/296x296SC.CAESS02.webp?l=en-US 
    // but not for https://is1-ssl.mzstatic.com/image/thumb/Features/v4/e0/20/1e/e0201e02-c94b-f581-4cbe-d41eec97d5f8/bedb6497-60b3-40fd-ba2f-502da28c3a8e.png/220x220SC.CAESS02.webp?l=en-US
    Essentials(u8),
    HitsOfTheYear(u8), // HGOY = Hits _ _ Year ; GO = greatest of? :: greatest hits of the year?

    // CADC01 => same as DC01 ?? just watermark branding i guess
}
impl FeaturedCategory {
    pub const PREFIX: &'static str = "FC";

    /// Returns the static discriminator *without* any prefix.
    pub const fn static_str(&self) -> &'static str {
        match self {
            Self::Essentials(..) => "ESS",
            Self::HitsOfTheYear(..) => "HGOY",
        }
    }

    pub const fn get_variant_number(&self) -> u8 {
        *match self {
            Self::Essentials(n) => n,
            Self::HitsOfTheYear(n) => n
        }
    }

    pub fn from_deconstructed(variant: &str, number: u8) -> Option<Self> {
        match variant {
            "ESS" => Some(Self::Essentials(number)),
            "HGOY" => Some(Self::HitsOfTheYear(number)),
            _ => None
        }
    }
}
impl core::fmt::Display for FeaturedCategory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "CA{}{:02}", self.static_str(), self.get_variant_number())
    }
}


// looks interesting https://is1-ssl.mzstatic.com/image/thumb/FuseSocial124/v4/69/21/8c/69218c18-37ec-e67a-34c5-0af724e0cb08/Job5e9c5524-8fce-444c-b364-7476d7a0b5aa-108342134-PreviewImage_preview_image_nonvideo_sdr-Time1608577588485.png/680x382mv.webp

// FPESS[01-04] framed essentials
// FPSL[??] framed set list

// CADC01 => (?) adds apple music branding in top right. used for official playlists/collections
// ^ CADC = category collection? seems more like [CA,DC] then but what is DC 

// https://is1-ssl.mzstatic.com/image/thumb/Features125/v4/97/65/1a/97651ac2-a24c-0f40-1809-84c08075a2da/U0MtTVMtV1ctVG9wXzI1LUF0bGFudGEtQURBTV9JRD0xNTU1OTkzODkwLnBuZw.png/296x296cc-60.jpg
// ^ top 25 atlanta but interestingly decode the filename it's base64 => "SC-MS-WW-Top_25-Atlanta-ADAM_ID=1555993890.png"
// ^ SC => is framing variant
// https://is1-ssl.mzstatic.com/image/thumb/Features126/v4/85/89/28/8589281a-4930-0b2c-30cc-ee488e6eb748/U0MtTVMtV1ctVG9wXzI1LUJlaWppbmctQURBTV9JRD0xNTU1OTk0MDU3LnBuZw.png/296x296cc.webp
// ^ diff uuid for beijing prob not predictable

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Framing {
    FeaturedPlaylist(FeaturedPlaylist),
    FeaturedCategory(FeaturedCategory),
    AppleMusicWatermarkTopRight { classical: bool },
    PDCXS { variant: u8, payload: GeneratedPlaylistCoverPayload, signature: String }
}
impl core::fmt::Display for Framing {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FeaturedCategory(fc) => fc.fmt(f),
            Self::FeaturedPlaylist(fp) => fp.fmt(f),
            Self::AppleMusicWatermarkTopRight { classical } => f.write_str(if *classical { "DNC01" } else { "DN01" }),
            Self::PDCXS { .. } => f.write_str("PDCXS"),
        }?;

        match self {
            Self::PDCXS { variant, .. } => {
                write!(f, "{variant:02}")
            }
            _ => Ok(()),
        }
    }
}
impl<'a> TryFrom<&'a str> for Framing {
    type Error = FramingParseError<'a>;

    fn try_from(mut value: &'a str) -> Result<Self, Self::Error> {
        let frame = read!(value, while: |char| char.is_ascii_uppercase());
        let variant = read!(value, while: |char| char.is_ascii_digit()).parse()?;


        if frame == "PDXCS" {
            unimplemented!()
        } else if let Some(sub) = frame.strip_prefix(FeaturedPlaylist::PREFIX) {
            FeaturedPlaylist::from_deconstructed(sub, variant).map(Framing::FeaturedPlaylist).ok_or(FramingParseError::UnknownFrame(sub))
        } else if let Some(sub) = frame.strip_prefix(FeaturedCategory::PREFIX) {
            FeaturedCategory::from_deconstructed(sub, variant).map(Framing::FeaturedCategory).ok_or(FramingParseError::UnknownFrame(sub))
        } else {
            // TODO: DN01/DNC01 too
            Err(FramingParseError::UnknownFrame(frame))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rgb {
    #[doc =   "Red color channel"] pub r: u8,
    #[doc = "Green color channel"] pub g: u8,
    #[doc =  "Blue color channel"] pub b: u8
}
impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
    const fn to_raw_hex_with_alphabet(self, alphabet: [u8; 16]) -> [u8; 6] {
        [
            alphabet[(self.r >> 4) as usize],
            alphabet[(self.r & 0xF) as usize],
            alphabet[(self.g >> 4) as usize],
            alphabet[(self.g & 0xF) as usize],
            alphabet[(self.b >> 4) as usize],
            alphabet[(self.b & 0xF) as usize],
        ]
    }
    const fn to_raw_hex_uppercase(self) -> [u8; 6] {
        self.to_raw_hex_with_alphabet([
            b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9',
            b'A', b'B', b'C', b'D', b'E', b'F'
        ])
    }
}
impl core::fmt::UpperHex for Rgb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let raw = &self.to_raw_hex_uppercase();
        let str = unsafe { core::str::from_utf8_unchecked(raw) };
        write!(f, "{str}")
    }
}


#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GeneratedPlaylistCoverPayload {
    background_colors: [Rgb; 4],
    text: String,
    text_color: Rgb,
    // vkey: u16, // I've only seen it as '1' so far.
}
impl core::fmt::Display for GeneratedPlaylistCoverPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, color) in self.background_colors.iter().enumerate() {
            let delimiter = if i == 0 { "?" } else { "&" };
            write!(f, "{delimiter}c{}={color:X}", i + 1)?
        }
        let encoded_text = {
            use base64::prelude::*;
            BASE64_URL_SAFE.encode(&self.text)
        };
        write!(f, "&t={encoded_text}")?;
        write!(f, "&tc={:X}", self.text_color)?;
        write!(f, "&vkey=1")?; // idk
        Ok(())
    }
}
