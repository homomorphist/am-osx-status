use crate::MaybeOwnedString;

pub mod error {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct UnknownCharacterEntity;
    impl core::error::Error for UnknownCharacterEntity {}
    impl core::fmt::Display for UnknownCharacterEntity {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "unknown character entity")
        }
    }

    #[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
    #[repr(u8)]
    pub enum CharacterEntityDecodingError {
        #[error("did not terminate (no ';' found)")]
        DidNotTerminate,
        #[error("could not parse numeric encoded codepoint")]
        InvalidForm(#[from] core::num::ParseIntError),
        #[error("unknown pre-defined character entity")]
        UnknownEntity(#[from] UnknownCharacterEntity),
        #[error("no associated character exists for the codepoint")]
        InvalidCharacter,
    }
    impl CharacterEntityDecodingError {
        fn discriminant(&self) -> u8 {
            unsafe { *<*const _>::from(self).cast::<u8>() }
        }
    }
    impl PartialOrd for CharacterEntityDecodingError {
        fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for CharacterEntityDecodingError {
        fn cmp(&self, other: &Self) -> core::cmp::Ordering {
            use core::cmp::Ordering;
            match self.discriminant().cmp(&other.discriminant()) {
                o @ Ordering::Greater |
                o @ Ordering::Less => o,
                Ordering::Equal => {
                    if let Self::InvalidForm(lhs_pie) = self {
                        let rhs_pie = if let Self::InvalidForm(pie) = other { pie } else { unsafe { core::hint::unreachable_unchecked() } };
                        let rhs_pie = *unsafe { core::mem::transmute::<&core::num::IntErrorKind, &u8>  (rhs_pie.kind()) };
                        let lhs_pie = *unsafe { core::mem::transmute::<&core::num::IntErrorKind, &u8>  (lhs_pie.kind()) };
                        lhs_pie.cmp(&rhs_pie)
                    } else {
                        Ordering::Equal
                    }
                }
            }
        }
    }
    impl core::hash::Hash for CharacterEntityDecodingError {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            state.write(&[self.discriminant()])
        }
    }
}

use error::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum CharacterEntity {
    QuotationMark,
    Ampersand,
    Apostrophe,
    LessThan,
    GreaterThan
}
impl CharacterEntity {
    pub const fn to_char(self) -> char {
        match self {
            Self::QuotationMark => '"',
            Self::Ampersand => '&',
            Self::Apostrophe => '\'',
            Self::LessThan => '>',
            Self::GreaterThan => '<'
        }
    }
    pub const fn to_str(self) -> &'static str {
        match self {
            Self::QuotationMark => "\"",
            Self::Ampersand => "&",
            Self::Apostrophe => "'",
            Self::LessThan => ">",
            Self::GreaterThan => "<",
        }
    }
}
impl core::fmt::Display for CharacterEntity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Write::write_char(f,self.to_char())
    }
}
impl<'a> TryFrom<&'a str> for CharacterEntity {
    type Error = UnknownCharacterEntity;
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        match value {
            "quot" => Ok(Self::QuotationMark),
            "amp" => Ok(Self::Ampersand),
            "apos" => Ok(Self::Apostrophe),
            "lt" => Ok(Self::LessThan),
            "gt" => Ok(Self::GreaterThan),
            _ => Err(UnknownCharacterEntity)
        }
    }
}
impl From<CharacterEntity> for char {
    fn from(value: CharacterEntity) -> Self {
        value.to_char()
    }
}
impl From<&CharacterEntity> for char {
    fn from(value: &CharacterEntity) -> Self {
        value.to_char()
    }
}
impl From<CharacterEntity> for &'static str {
    fn from(value: CharacterEntity) -> Self {
        value.to_str()
    }
}
impl From<&CharacterEntity> for &'static str {
    fn from(value: &CharacterEntity) -> Self {
        value.to_str()
    }
}
impl From<CharacterEntity> for String {
    fn from(value: CharacterEntity) -> Self {
        value.to_string()
    }
}
impl From<&CharacterEntity> for String {
    fn from(value: &CharacterEntity) -> Self {
        value.to_string()
    }
}


#[derive(Debug, PartialEq)]
enum Escape {
    Codepoint(char),
    Entity(CharacterEntity)
}
impl From<Escape> for char {
    fn from(value: Escape) -> Self {
        match value {
            Escape::Codepoint(char) => char,
            Escape::Entity(entity) => entity.to_char()
        }
    }
}

#[derive(Debug, PartialEq)]
struct EscapeInfo {
    pub character: Escape,
    /// The length of the escape, in characters (synonymous in this case with bytes, as only ASCII is used to escape),
    /// including the indicator (`&`) and terminator (`;`).
    pub length: usize,
    pub position: usize,
}


/// Yields (only) escaped characters from a string.
/// 
/// If it encounters a [`CharacterEntityDecodingError`], it will continue to return that [`CharacterEntityDecodingError`]
/// (because it will continue to try and read from where it last left off).
struct EscapeIterator<'a> {
    str: &'a str,
    pos: usize,
}
impl<'a> EscapeIterator<'a> {
    const fn new(str: &'a str) -> Self {
        Self { str, pos: 0 }
    }
}
impl Iterator for EscapeIterator<'_> {
    type Item = Result<EscapeInfo, CharacterEntityDecodingError>;
    fn next(&mut self) -> Option<Self::Item> {
        macro_rules! propagate_error_presence {
            ($result: expr) => {
                match $result {
                    Err(err) => return Some(Err(err.into())),
                    Ok(val) => val
                }
            };
        }

        let ampersand = self.pos + self.str[self.pos..].find('&')?;
        let semicolon = ampersand + propagate_error_presence!(self.str[ampersand..].find(';').ok_or(CharacterEntityDecodingError::DidNotTerminate));

        let slice = &self.str[ampersand + '&'.len_utf8()..semicolon];

        let escaped = if slice.starts_with('#') {
            let hex = slice.as_bytes()[1] == b'x';
            let radix = if hex { 16 } else { 10 };
            let slice = &slice[1 + if hex { 1 } else { 0 }..];
            let codepoint = propagate_error_presence!(u32::from_str_radix(slice, radix));
            let char = propagate_error_presence!(char::from_u32(codepoint).ok_or(CharacterEntityDecodingError::InvalidCharacter));
            Escape::Codepoint(char)
        } else {
            let entity = propagate_error_presence!(CharacterEntity::try_from(slice));
            Escape::Entity(entity)
        };

        let length = semicolon - ampersand + 1;;
        self.pos = semicolon;
        Some(Ok(EscapeInfo {
            character: escaped,
            length,
            position: ampersand,
        }))
    }
}
impl core::iter::FusedIterator for EscapeIterator<'_> {}

#[derive(Debug, PartialEq)]
enum CharDecodeResultOrStr<'a> {
    Char(Result<EscapeInfo, CharacterEntityDecodingError>),
    Str(&'a str)
}
struct EscapeChunksIterator<'a> {
    sub: EscapeIterator<'a>,
    idx: usize,
    done: bool,
    next: Option<Result<EscapeInfo, CharacterEntityDecodingError>>
}

/// Yields escaped and unescaped chunks of a string.
/// 
/// If it encounters a [`CharacterEntityDecodingError`], it will continue to return that [`CharacterEntityDecodingError`]
/// (because it will continue to try and read from where it last left off).
impl<'a> EscapeChunksIterator<'a> {
    fn new(str: &'a str) -> Self {
        Self {
            sub: EscapeIterator::new(str),
            idx: 0,
            done: false,
            next: None
        }
    }
}
impl<'a> Iterator for EscapeChunksIterator<'a> {
    type Item = CharDecodeResultOrStr<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        if let Some(next) = self.next.take() {
            return Some(CharDecodeResultOrStr::Char(next))
        }

        match self.sub.next() {
            None => {
                self.done = true;
                let slice = &self.sub.str[self.idx..];
                if !slice.is_empty() {
                    Some(CharDecodeResultOrStr::Str(slice))
                } else {
                    None
                }
            },
            Some(result) => {
                match result {
                    Err(err) => Some(CharDecodeResultOrStr::Char(Err(err))),
                    Ok(ref escape) => {
                        let slice = &self.sub.str[self.idx..escape.position];
                        self.idx = escape.position + escape.length;
                        if slice.is_empty() {
                            Some(CharDecodeResultOrStr::Char(result))
                        } else {
                            self.next = Some(result);
                            Some(CharDecodeResultOrStr::Str(slice))
                        }
                    }
                }
            }
        }
    }
}


#[cfg(test)]
mod escapes_chunks_iterator {
    use super::*;

    #[test]
    fn basic() {
        use super::*;
    
        let mut iter = EscapeChunksIterator::new("&gt;bottomless pit supervisor");
        assert_eq!(iter.next(), Some(CharDecodeResultOrStr::Char(Ok(EscapeInfo { character: Escape::Entity(CharacterEntity::GreaterThan), length: "&gt;".len(), position: 0 }))));
        assert_eq!(iter.next(), Some(CharDecodeResultOrStr::Str("bottomless pit supervisor")));
        assert_eq!(iter.next(), None);
    
        let mut iter = EscapeChunksIterator::new("Alice &amp; Bob");
        assert_eq!(iter.next(), Some(CharDecodeResultOrStr::Str("Alice ")));
        assert_eq!(iter.next(), Some(CharDecodeResultOrStr::Char(Ok(EscapeInfo { character: Escape::Entity(CharacterEntity::Ampersand), length: "&amp;".len(), position: 6 }))));
        assert_eq!(iter.next(), Some(CharDecodeResultOrStr::Str(" Bob")));
        assert_eq!(iter.next(), None);
    
        let mut iter = EscapeChunksIterator::new("&quot;Die!&quot;");
        assert_eq!(iter.next(), Some(CharDecodeResultOrStr::Char(Ok(EscapeInfo { character: Escape::Entity(CharacterEntity::QuotationMark), length: "&quot;".len(), position: 0 }))));
        assert_eq!(iter.next(), Some(CharDecodeResultOrStr::Str("Die!")));
        assert_eq!(iter.next(), Some(CharDecodeResultOrStr::Char(Ok(EscapeInfo { character: Escape::Entity(CharacterEntity::QuotationMark), length: "&quot;".len(), position: 10 }))));
        assert_eq!(iter.next(), None);
    
        let mut iter = EscapeChunksIterator::new("&apos;");
        assert_eq!(iter.next(), Some(CharDecodeResultOrStr::Char(Ok(EscapeInfo { character: Escape::Entity(CharacterEntity::Apostrophe), length: "&apos;".len(), position: 0 }))));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn empty() {
        let mut iter: EscapeChunksIterator<'_> = EscapeChunksIterator::new("");
        assert_eq!(iter.next(), None);
    }


    #[test]
    fn invalid_continuously_returns_error() {
        use super::*;

        let mut iter = EscapeChunksIterator::new("&apos");
        assert_eq!(iter.next(), Some(CharDecodeResultOrStr::Char(Err(error::CharacterEntityDecodingError::DidNotTerminate))));
        assert_eq!(iter.next(), Some(CharDecodeResultOrStr::Char(Err(error::CharacterEntityDecodingError::DidNotTerminate))));
        assert_eq!(iter.next(), Some(CharDecodeResultOrStr::Char(Err(error::CharacterEntityDecodingError::DidNotTerminate))));
    }
}

/// Lazily-escaped XML character data.
// NOTE: This will still allocate if given data that doesn't need any escaping.
//       As such, you should check for the presence of escaping prior to constructing this.
// TODO: Implement PartialEq<
#[derive(Debug, Clone)]
pub struct XmlCharacterDataWithEscaping<'a> {
    raw: &'a str,
    unescaped: core::cell::OnceCell<Result<String, CharacterEntityDecodingError>>,
}
impl<'a> XmlCharacterDataWithEscaping<'a> {
    const fn escapes(value: &'a str) -> EscapeIterator<'a> {
        EscapeIterator::new(value)
    }
    
    fn unescape(value: &'a str) -> Result<String, CharacterEntityDecodingError> {
        let mut out = String::with_capacity(value.len()); // approx
        let mut ended = 0;

        for escape in Self::escapes(value) {
            let escape = escape?;
            out.push_str(&value[ended..escape.position]);
            out.push(escape.character.into());
            ended = escape.position + escape.length - 1;
        }

        out.push_str(&value[ended..]);
        Ok(out)
    }

    pub const fn new(escaped: &'a str) -> Self {
        Self {
            raw: escaped,
            unescaped: core::cell::OnceCell::new()
        }
    }

    pub fn get(&self) -> Result<&str, &CharacterEntityDecodingError> {
        self.unescaped.get_or_init(|| Self::unescape(self.raw)).as_ref().map(String::as_str)
    }

    pub const fn get_unescaped(&self) -> &str {
        self.raw
    }

    pub fn into_string(mut self) -> Result<String, CharacterEntityDecodingError> {
        self.unescaped.take().unwrap_or_else(|| Self::unescape(self.raw))
    }

    pub fn did_unescape(&self) -> bool {
        self.unescaped.get().is_some()
    }
}

// TODO: Benchmark cost of allocation versus the cost of using EscapeChunksIterator for all of these.
impl PartialEq<str> for XmlCharacterDataWithEscaping<'_> {
    fn eq(&self, other: &str) -> bool {
        (self.raw == other) || {
            self.get() == Ok(other)
        }
    }
}
impl PartialEq for XmlCharacterDataWithEscaping<'_> {
    fn eq(&self, other: &Self) -> bool {
        (self.raw == other.raw) || {
            self.get() == other.get()
        }
    }
}
impl Eq for XmlCharacterDataWithEscaping<'_> {}
impl PartialOrd<str> for XmlCharacterDataWithEscaping<'_> {
    fn partial_cmp(&self, other: &str) -> Option<std::cmp::Ordering> {
        self.get().partial_cmp(&Ok(other))
    }
}
impl PartialOrd for XmlCharacterDataWithEscaping<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for XmlCharacterDataWithEscaping<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.get().cmp(&other.get())
    }
}

#[derive(Debug, Clone)]
pub enum XmlCharacterData<'a> {
    Plain(&'a str), // CDATA or PCDATA w/o escape characters
    WithEntities(XmlCharacterDataWithEscaping<'a>) // PCDATA w/ escaping and such, or normal CDATA which could possibly be escaping (since it had an ampersand)
}
impl<'a> XmlCharacterData<'a> {
    pub fn get(&self) -> Result<&str, &CharacterEntityDecodingError> {
        match self {
            Self::Plain(text) => Ok(*text),
            Self::WithEntities(inner) => inner.get()
        }
    }
    pub fn into_maybe_owned(self) -> Result<MaybeOwnedString<'a>, CharacterEntityDecodingError> {
        Ok(match self {
            Self::Plain(text) => MaybeOwnedString::Borrowed(text),
            Self::WithEntities(inner) => MaybeOwnedString::Owned(inner.into_string()?)
        })
    }
    pub fn maybe_escaping(text: &'a str) -> Self {
        if text.contains('&') { // haha
            Self::WithEntities(XmlCharacterDataWithEscaping::new(text))
        } else {
            Self::Plain(text)
        }
    }
    pub fn is_just_whitespace(&self) -> Result<bool, &CharacterEntityDecodingError> {
        Ok(self.get()?.trim_matches(crate::defs::WHITESPACE).is_empty())
    }
    pub fn raw(&self) -> &str {
        match self {
            Self::Plain(text) => text,
            Self::WithEntities(inner) => inner.get_unescaped()
        }
    }
}
impl PartialEq<str> for XmlCharacterData<'_> {
    fn eq(&self, other: &str) -> bool {
        match self {
            Self::Plain(text) => *text == other,
            Self::WithEntities(inner) => inner.get() == Ok(other)
        }
    }
}
impl PartialEq for XmlCharacterData<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Plain(lhs), Self::Plain(rhs)) => lhs == rhs,
            (Self::Plain(txt), Self::WithEntities(enc)) |
            (Self::WithEntities(enc), Self::Plain(txt)) => enc.get() == Ok(*txt),
            (Self::WithEntities(lhs), Self::WithEntities(rhs)) => lhs.get() == rhs.get()
        }
    }
}
impl Eq for XmlCharacterData<'_> {}
impl PartialOrd<str> for XmlCharacterData<'_> {
    fn partial_cmp(&self, other: &str) -> Option<std::cmp::Ordering> {
        match self {
            Self::Plain(text) => text.partial_cmp(&other),
            Self::WithEntities(inner) => inner.get().partial_cmp(&Ok(other))
        }
    }
}
impl PartialOrd for XmlCharacterData<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for XmlCharacterData<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::Plain(lhs), Self::Plain(rhs)) => lhs.cmp(rhs),
            (Self::Plain(txt), Self::WithEntities(enc)) => Ok(*txt).cmp(&enc.get()),
            (Self::WithEntities(enc), Self::Plain(txt)) => enc.get().cmp(&Ok(*txt)),
            (Self::WithEntities(lhs), Self::WithEntities(rhs)) => lhs.get().cmp(&rhs.get())
        }
    }
}

