use std::{collections::HashSet, ops::AddAssign};

pub mod artwork;

#[derive(Eq, PartialEq, Hash, Debug)]
pub enum Component {
    AlbumImage,
    ArtistImage,
    ITunesData
}

#[derive(Default, Debug)]
pub struct ComponentSolicitation {
    pub list: HashSet<Component>
}
impl AddAssign<ComponentSolicitation> for ComponentSolicitation {
    fn add_assign(&mut self, rhs: ComponentSolicitation) {
        for component in rhs.list {
            self.list.insert(component);
        }
    }
}
