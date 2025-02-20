use serde::{Deserialize, Serialize};
use crate::id::{IdPossessor, IdSubject};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Recording {
    pub id: crate::Id<Self>,
    pub artist_credit: super::artist::credit::List,
}
impl IdPossessor for Recording {
    const VARIANT: IdSubject = IdSubject::Recording;
}
