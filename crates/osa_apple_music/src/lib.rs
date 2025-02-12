use serde::Deserialize;
mod track;

rating::def_rating!({}, Rating);
impl<T> PartialEq<T> for Rating where T: AsRef<Rating> {
    fn eq(&self, other: &T) -> bool {
        self == other.as_ref()
    }
}

pub(crate) mod rating {
    use super::*;

    #[macro_export]
    macro_rules! def_rating {
        ({ $(#[$meta: meta])* }, $ident: ident) =>  {
            #[derive(Debug, Deserialize, PartialEq)]
            $(#[$meta])*
            pub enum $ident {
                User(u8),
                Computed(u8)
            }
        };
        ({ $(#[$meta: meta])* }, $ident: ident, equivalent => $into: ident) => {
            def_rating!({ $(#[$meta])* }, $ident);
            impl From<$ident> for $into {
                fn from(value: $ident) -> $into {
                    unsafe { core::mem::transmute(value) }
                }
            }
            impl From<$into> for $ident {
                fn from(value: $into) -> $ident {
                    unsafe { core::mem::transmute(value) }
                }
            }
            impl AsRef<$into> for $ident {
                fn as_ref(&self) -> &$into {
                    unsafe { core::mem::transmute(self) }
                }
            }
        }
    }

    pub use def_rating;

    def_rating!({
        /// The rating of a track's album.
        #[serde(tag = "albumRatingKind", content = "albumRating", rename_all = "lowercase")]
    }, ForTrackAlbum, equivalent => Rating);
}
