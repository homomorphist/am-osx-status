use enum_bitset::EnumBitset;

pub mod artwork;

#[derive(Copy, Clone, PartialEq, Eq, Debug, EnumBitset)]
#[bitset(name = ComponentSolicitation)]
pub enum Component {
    AlbumImage,
    ArtistImage,
    ITunesData
}
