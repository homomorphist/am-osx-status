#![allow(private_bounds)]

use shared::HyphenatedUuidString;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Id<T: IdPossessor>(HyphenatedUuidString, core::marker::PhantomData<T>);
impl<T: IdPossessor> Id<T> {
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
    pub const fn contextless(self) -> HyphenatedUuidString {
        self.0
    }

    /// # Safety
    /// - The ID must be for an item of the specified type.
    pub const unsafe fn from_contextless(uuid: HyphenatedUuidString) -> Id<T> {
        Self(uuid, core::marker::PhantomData)
    }

}
impl<T: IdPossessor> core::fmt::Display for Id<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}
impl<T: IdPossessor> core::fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.pad(&format!("Id<{:?}(\"{}\")", T::VARIANT, self.as_str()))
    }
}
impl<T: IdPossessor> serde::Serialize for Id<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}
impl<'de, T: IdPossessor> serde::Deserialize<'de> for Id<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let id = HyphenatedUuidString::new(&s).ok_or_else(|| {
            <D::Error as serde::de::Error>::custom("Invalid UUID")
        })?;
        Ok(unsafe { Self::from_contextless(id) })
    }
}

/// - <https://musicbrainz.org/doc/MusicBrainz_Database/Schema>
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum IdSubject {
    /// Geographical location/region.
    /// More general than a [`Self::Place`].
    /// - <https://musicbrainz.org/doc/Area>
    Area,
    /// One or more (i.e. a an individual or a group) music artists.
    /// - <https://musicbrainz.org/doc/Artist>
    Artist,
    /// An organized activity.
    /// - <https://musicbrainz.org/doc/Event>
    Event,
    /// Something that can make sound.
    /// - <https://musicbrainz.org/doc/Instrument>
    Instrument,
    // TODO: Comprehend that.
    /// - <https://musicbrainz.org/doc/Label>
    Label,
    /// A physical location.
    /// More specific than an [`Self::Area`].
    /// - <https://musicbrainz.org/doc/Place>
    Place,
    /// A distinct track creation instance.
    /// - <https://musicbrainz.org/doc/Recording>
    Recording,
    // TODO: Describe.
    /// - <https://musicbrainz.org/doc/Release>
    Release,
    // TODO: Describe.
    /// - <https://musicbrainz.org/doc/Release_Group>
    ReleaseGroup,
    // TODO: Describe.
    /// - <https://musicbrainz.org/doc/Series>
    Series,
    // TODO: Describe. These, uh, don't have much documented on them. But they do exist!
    /// - <https://wiki.musicbrainz.org/Track>
    Track,
    // TODO: Describe.
    /// - <https://musicbrainz.org/doc/Work>
    Work,
}

pub(crate) trait IdPossessor {
    const VARIANT: IdSubject;
}

