use crate::{boma::*, chunk::*, id, setup_eaters, PersistentId};
use super::{album::Album, artist::Artist, derive_list};

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

    fn read_sized_content(cursor: &mut std::io::Cursor<&'a [u8]>, offset: u64, length: u32) -> Result<Self, Self::ReadError> {
        setup_eaters!(cursor, offset, length);
        skip!(4)?; // appendage byte length
        let boma_count = u32!()?;
        let persistent_id = id!(Account)?;
        skip_to_end!();
        let bomas = cursor.reading_chunks::<Boma>(boma_count as usize).collect::<Result<_, _>>()?;
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

derive_list!(pub AccountInfoList, Account<'a>, *b"Lsma");
