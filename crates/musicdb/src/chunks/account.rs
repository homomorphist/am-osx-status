use crate::{boma::*, chunk::*, id, setup_eaters, PersistentId};
use super::derive_list;

#[allow(unused)]
#[derive(Debug)]
pub struct Account<'a> {
    bomas: Vec<Boma<'a>>,
    pub persistent_id: <Self as id::persistent::Possessor>::Id,
}
impl<'a> Chunk for Account<'a> {
    const SIGNATURE: Signature = Signature::new(*b"isma");
}
impl<'a> SizedFirstReadableChunk<'a> for Account<'a> {
    type ReadError = std::io::Error;
    type AppendageLengths = crate::chunk::appendage::lengths::LengthWithAppendagesAndQuantity;
    fn read_sized_content(cursor: &mut ChunkCursor<'a>, offset: usize, length: u32, appendage_lengths: &Self::AppendageLengths) -> Result<Self, Self::ReadError> {
        setup_eaters!(cursor, offset, length);
        let persistent_id = id!(Account)?;
        skip_to_end!()?;
        let bomas = cursor.reading_chunks::<Boma>(appendage_lengths.count as usize).collect::<Result<_, _>>()?;
        Ok(Self { bomas, persistent_id })
    }
}
impl<'a> id::persistent::Possessor for Account<'a> {
    #[allow(private_interfaces)]
    const IDENTITY: id::persistent::PossessorIdentity = id::persistent::PossessorIdentity::Account;
    type Id = PersistentId<Account<'a>>;
    fn get_persistent_id(&self) -> Self::Id {
        self.persistent_id
    }
}
impl id::cloud::library::Possessor for Account<'_> {
    #[allow(private_interfaces)]
    const IDENTITY: id::cloud::library::PossessorIdentity = id::cloud::library::PossessorIdentity::Account;
}


derive_list!(pub AccountInfoList, Account<'a>, *b"Lsma");
