pub mod artwork;

#[derive(Eq, PartialEq, Hash, Debug)]
pub enum Component {
    AlbumImage,
    ArtistImage,
    ITunesData
}

#[derive(Default, Debug)]
pub struct ComponentSolicitation {
    // TODO: Use a BitSet or similar for efficiency / reduce allocations
    pub list: std::collections::HashSet<Component>
}
impl core::ops::AddAssign<Self> for ComponentSolicitation {
    fn add_assign(&mut self, rhs: Self) {
        for component in rhs.list {
            self.list.insert(component);
        }
    }
}
