#![doc = include_str!("../README.md")]
#![allow(unused)]
pub mod cdata;
pub mod defs;
pub mod arena;
pub mod span;
use arena::*;
use error::*;
use cdata::XmlCharacterData;
use span::{Span, SingleFileLocation};

use maybe_owned_string::MaybeOwnedString;

use core::num::{NonZeroUsize, NonZero};

pub mod error {
    pub use super::cdata::error::*;

    #[derive(thiserror::Error, Debug, PartialEq)]
    pub enum AttributeParseError {
        #[error("expected value after attribute KV delimiter")]
        ExpectedValueAfterDelimiter,
        #[error("expected KV delimiter after key")]
        ExpectedDelimiterAfterKey,
    }

    #[derive(thiserror::Error, Debug, PartialEq)]
    pub enum SectionOpenerReadError<'a> {
        // TODO: better error here
        #[error("tag opened at line {} did not close", .0.start_location().line)]
        TagDidNotClose(super::Span<'a>),
        #[error("invalid tag name")]
        InvalidTagName,
        #[error("attribute parse error: {0}")]
        AttributeParseError(#[from] AttributeParseError)
    }

    #[derive(thiserror::Error, Debug)]
    pub enum NodeParseError<'a, A: super::NodeArena<'a>> {
        #[error("{0}")]
        BadSectionOpener(SectionOpenerReadError<'a>),

        #[error("section opened at line {} did not close", .0.start_location().line)]
        NonNestingDidNotClose(super::Span<'a>, super::NonNestingSection),

        #[error("element opened on line {} did not close", .0.span.start_location().line)]
        ElementDidNotClose(super::OpeningTagSpan<'a>),

        #[error("arena push failure")]
        ArenaPushFailure(A::Error),
        #[error("arena reference list push failure")]
        ArenaReferenceListPushFailure(<A::NodeReferenceList as super::arena::NodeReferenceCollection<'a>>::Error),
    }
}



#[derive(Debug, PartialEq)]
pub struct Read<T> {
    pub value: T,
    pub consumed_bytes: usize,
}
impl<T> Read<T> {
    fn into_inner(self) -> T {
        self.value
    }
}
impl<T> AsRef<T> for Read<T> {
    fn as_ref(&self) -> &T {
        &self.value
    }
}


#[derive(Debug, PartialEq, Default)]
pub struct Attributes<'a>(
    // TODO: Is this really how we want to implement this?
    std::collections::HashMap<
        Span<'a>,
        XmlCharacterData<'a>,
    >
);


#[derive(Debug)]
pub enum NonNestingSection {
    UnescapedCharacterData,
    Comment,
}

trait BlockSpan<'a>: core::ops::Deref<Target = Span<'a>> {
    const OPENER: &'static str;
    const CLOSER: &'static str;

    fn new(span: Span<'a>) -> Self where Self: Sized;

    fn opener(&self) -> Span<'a> {
        unsafe { self.slice_bytes_inclusive(0, Some(NonZero::new_unchecked(Self::OPENER.len()))) }
    }
    fn closer(&self) -> Span<'a> {
        unsafe { self.slice_bytes_off_of_end_inclusive(Self::CLOSER.len()) }
    }
    fn content(&self) -> Span<'a> {
        unsafe { self.slice_bytes_inclusive(Self::OPENER.len(),  Some(NonZero::new_unchecked(self.len() - Self::CLOSER.len()))) }
    }

    fn parse_after_opening(input: &Span<'a>, opener: Span<'a>) -> Option<Self> where Self: Sized {
        let closer_at = unsafe { input.slice_bytes_inclusive(Self::OPENER.len(), None) }.as_str().find(Self::CLOSER)?;
        unsafe { Some(Self::new(input.slice_bytes_inclusive(0, Some(NonZero::new_unchecked(closer_at + Self::OPENER.len() + Self::CLOSER.len())) ))) }
    }
}

macro_rules! mk_block_span {
    ($ident: ident, [$l: literal, $r: literal]) => {
        #[repr(transparent)]
        #[derive(Debug, PartialEq, Clone, Copy)]
        pub struct $ident<'a>(Span<'a>);
        impl<'a> BlockSpan<'a> for $ident<'a> {
            const OPENER: &'static str = $l;
            const CLOSER: &'static str = $r;
            fn new(span: Span<'a>) -> Self { Self(span) }
        }
        impl<'a> core::ops::Deref for $ident<'a> {
            type Target = Span<'a>;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    }
}

mk_block_span!(CommentSpan,            ["<!--",      "-->"]);
mk_block_span!(CharacterDataBlockSpan, ["<![CDATA[", "]]>"]);


#[derive(Debug, PartialEq)]
pub struct OpeningTagSpan<'a> {
    pub span: Span<'a>,
    pub attributes: Attributes<'a>,
    name_length: usize,
}
impl<'a> OpeningTagSpan<'a> {
    pub fn get_name_span(&self) -> Span<'a> {
        const OFFSET: usize = const { '<'.len_utf8() };
        unsafe { self.span.slice_bytes_inclusive(OFFSET, Some(NonZero::new_unchecked(OFFSET + self.name_length - if self.is_self_closing() { 1 } else { 0 })) ) }
    }
    pub fn is_self_closing(&self) -> bool {
        b'/' == unsafe { *self.span.top.add(self.span.offset + self.span.length - 2) }
    }
}

#[derive(Debug, PartialEq)]
pub enum SectionOpener<'a> {
    Tag(OpeningTagSpan<'a>), 
    CharacterData(Span<'a>),
    Comment(Span<'a>),
}
impl<'a> SectionOpener<'a> {
    pub fn parse(input: &Span<'a>) -> Result<Option<SectionOpener<'a>>, SectionOpenerReadError<'a>> {
        if !input.starts_with("<") { return Ok(None) };

        if input.starts_with(CommentSpan::OPENER) {
            let ends_at = unsafe { NonZero::new_unchecked(CommentSpan::OPENER.len()) };
            let span: Span<'_> = unsafe { input.slice_bytes_inclusive(0, Some(ends_at)) };
            return Ok(Some(SectionOpener::Comment(span)))
        }

        if input.starts_with(CharacterDataBlockSpan::OPENER) {
            let ends_at =  unsafe { NonZero::new_unchecked(CharacterDataBlockSpan::OPENER.len()) };
            let span = unsafe { input.slice_bytes_inclusive(0, Some(ends_at)) };
            return Ok(Some(SectionOpener::CharacterData(span)))
        }

        let name_ends_at = input.find(['>', ' ', '\t', '\r', '\n']).ok_or(SectionOpenerReadError::TagDidNotClose(*input))?;
        let name_last_character_index = input.find(' ').map(|v| v.min(name_ends_at)).unwrap_or(name_ends_at);
        let name_last_character_index = NonZeroUsize::new(name_last_character_index).ok_or(SectionOpenerReadError::InvalidTagName)?;
        let name_length = name_last_character_index.get() - '<'.len_utf8();
        let name = unsafe { input.slice_bytes_inclusive(1, Some(name_last_character_index)) };

        #[derive(Debug, PartialEq)]
        enum Quote {
            Single,
            Double,
        }

        #[derive(Debug, PartialEq)]
        enum Parsing {
            Key,
            Value,
            AfterDelimiter,
        }

        
        let mut parsing = None;
        let mut parsing_started_at = None;
        let mut key: Option<Span> = None;
        let mut quote: Option<Quote> = None;
        let mut attributes = Attributes::default();
        let mut index = name_ends_at + 1;

        if input.as_bytes().get(name_ends_at) != Some(&b'>') {
            loop {
                let char = input.as_bytes().get(index).ok_or(SectionOpenerReadError::TagDidNotClose(*input))?;
                if key.is_none() {
                    debug_assert!(matches!(parsing, None | Some(Parsing::Key)));
                    if parsing.is_none() && if name == "xml" { *char != b'?' } else { true } {
                        //
                        // Find the end of the tag, or the start of a key, skipping whitespace.
                        //
                        //  <tag   attribute="value" [...]
                        //      ╚═╝┆
                        //         ╰╴`parsing_started_at`
                        //
                        //  <?xml [...] ?>
                        //              |
                        //              └ Exemption to not treat this as the start of a key.
                        //
                        //  <tag [...]>
                        //            |
                        //            └ Signifies the end of a tag; stops parsing attributes.
                        //
                        if crate::defs::WHITESPACE_U8.contains(char) { index += 1; continue }
                        if *char == b'>' { index += 1; break }
                        parsing = Some(Parsing::Key);
                        parsing_started_at = Some(unsafe { NonZeroUsize::new_unchecked(index) });
                    } else {
                        debug_assert_eq!(parsing, Some(Parsing::Key));
                        // Continually read letters of the key until we reach either whitespace or a ".".
                        // If we don't read a "=", we'll match that in the next case. This is just to stop reading what we consider a "key". 
                        match char {
                            b' ' | b'\t' | b'\r' | b'\n' | b'=' => {
                                key = Some(unsafe { input.slice_bytes_inclusive(parsing_started_at.unwrap().get(), Some(NonZero::new_unchecked(index))) });
                                parsing = if *char == b'=' { Some(Parsing::AfterDelimiter) } else { None }
                            },
                            b'>' => return Err(SectionOpenerReadError::AttributeParseError(AttributeParseError::ExpectedDelimiterAfterKey)),
                            // TODO: Prohibit characters like '<', maybe '&' and such.
                            _ => {}
                        }
                    }
                } else if !matches!(parsing, Some(Parsing::Key | Parsing::Value)) {
                    // Find the delimiter of a key-value pair if it hasn't yet been encountered because of whitespace padding terminating the key end index search.
                    // Then, find the start of the value by searching for a quote, ignoring whitespace padding.
                    match char {
                        b' ' | b'\t' | b'\r' | b'\n' => {},
                        b'=' if parsing.is_none() => { parsing = Some(Parsing::AfterDelimiter); }
                        b'\'' if parsing == Some(Parsing::AfterDelimiter) => { quote = Some(Quote::Single); parsing = Some(Parsing::Value); parsing_started_at = Some(unsafe { core::num::NonZero::new_unchecked(index) })}
                        b'\"' if parsing == Some(Parsing::AfterDelimiter) => { quote = Some(Quote::Double); parsing = Some(Parsing::Value); parsing_started_at = Some(unsafe { core::num::NonZero::new_unchecked(index) })}
                        _ if parsing == Some(Parsing::AfterDelimiter) => return Err(AttributeParseError::ExpectedValueAfterDelimiter.into()),
                        _  => return Err(AttributeParseError::ExpectedDelimiterAfterKey.into())
                    }
                } else if match quote {
                    Some(Quote::Double) => *char == b'\"',
                    Some(Quote::Single) => *char == b'\'',
                    None => unreachable!("parsing value without opening quote")
                } {
                    let span = unsafe { input.slice_bytes_inclusive(
                        parsing_started_at.unwrap().get() + 1,
                        Some(unsafe { NonZero::new_unchecked(index) })
                    ) };
                    attributes.0.insert(
                        key.expect("no key for value (unreachable)"),
                        cdata::XmlCharacterData::maybe_escaping(span.as_str())
                    );
                    key = None;
                    parsing = None;
                }

                index += 1;
                continue 
            }
        }


        Ok(Some(Self::Tag(OpeningTagSpan { 
            span: unsafe { input.slice_bytes_inclusive(0, Some(NonZeroUsize::new_unchecked(index))) },
            name_length,
            attributes
        })))
    }

    pub fn span(&self) -> &Span<'a> {
        match self {
            Self::Tag(tag) => &tag.span,
            Self::Comment(span) |
            Self::CharacterData(span) => span,
        }
    }

}

pub union CharacterDataSpan<'a> {
    as_block: CharacterDataBlockSpan<'a>,
    as_abrupt_node: Span<'a>,
}
impl<'a> CharacterDataSpan<'a> {
    pub fn block(span: CharacterDataBlockSpan<'a>) -> CharacterDataSpan<'a> {
        CharacterDataSpan {
            as_block: span
        }
    }
    pub fn abrupt_node(span: Span<'a>) -> CharacterDataSpan<'a> {
        CharacterDataSpan {
            as_abrupt_node: span
        }
    }
    pub fn as_content_span(&'a self) -> Span<'a> {
        if let Some(block) = self.as_block_span() {
            block.content()
        } else {
            self.as_raw_span()
        }
    }
    fn as_raw_span(&self) -> Span<'a> {
        unsafe { self.as_abrupt_node }
    }
    pub fn is_block(&self) -> bool {
        self.as_raw_span().starts_with(CharacterDataBlockSpan::OPENER)
    }
    pub fn as_block_span(&self) -> Option<CharacterDataBlockSpan> {
        if self.is_block() {
            Some(unsafe { self.as_block })
        } else {
            None
        }
    }
}
impl core::fmt::Debug for CharacterDataSpan<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_block() {
            write!(f, "CharacterDataSpan::Block({:?})", unsafe { self.as_block })
        } else {
            write!(f, "CharacterDataSpan::AbruptNode({:?})", unsafe { self.as_abrupt_node })
        }
    }
}
impl PartialEq for CharacterDataSpan<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_raw_span() == other.as_raw_span()
    }
}


#[derive(Debug, PartialEq)]
pub enum Node<'a, A: NodeArena<'a>> {
    Element(Element<'a, A>),
    Comment(CommentSpan<'a>),
    Text(XmlCharacterData<'a>, CharacterDataSpan<'a>), // including whitespace indentation
}
impl<'a, A: NodeArena<'a>> Node<'a, A> {
    pub fn parse(input: &Span<'a>, arena: &mut A) -> Result<Option<Read<A::NodeReference>>, NodeParseError<'a, A>> {
        if input.len() == 0 { return Ok(None) }

        let node = if let Some(opener) = SectionOpener::parse(input).map_err(|err| NodeParseError::BadSectionOpener(err))? {
            match opener {
                SectionOpener::Tag(opener) => Node::Element(Element::parse_after_opening(input, opener, arena)?),
                SectionOpener::Comment(opener) => Node::Comment(CommentSpan::parse_after_opening(input, opener).ok_or(NodeParseError::NonNestingDidNotClose(opener, NonNestingSection::Comment))?),
                SectionOpener::CharacterData(opener) => {
                    let span = CharacterDataBlockSpan::parse_after_opening(input, opener).ok_or(NodeParseError::NonNestingDidNotClose(opener, NonNestingSection::UnescapedCharacterData))?;
                    let text = XmlCharacterData::Plain(span.content().as_str());
                    Self::Text(text, CharacterDataSpan::block(span))
                },
            }
        } else {
            let text = unsafe { input.slice_bytes_inclusive(0, input.find('<').and_then(NonZero::new)) };
            Self::Text(XmlCharacterData::maybe_escaping(text.as_str()), CharacterDataSpan::abrupt_node(text))
        };

        let consumed_bytes = node.span().length;
        Ok(Some(Read { value: arena.add(node).map_err(NodeParseError::ArenaPushFailure)?, consumed_bytes }))
    }

    pub fn span(&self) -> Span<'a> {
        match self {
            Self::Comment(span) => span.0,
            Self::Text(_, span) => span.as_raw_span(),
            Self::Element(element) => element.span(),
        }
    }

    pub fn into_element(self) -> Option<Element<'a, A>> {
        match self {
            Self::Element(element) => Some(element),
            Self::Comment(_) | Self::Text(..) => None,
        }
    }


    pub fn as_element(&self) -> Option<&Element<'a, A>> {
        match self {
            Self::Element(element) => Some(element),
            Self::Comment(_) | Self::Text(..) => None,
        }
    }

    pub fn into_cdata(self) -> Option<XmlCharacterData<'a>> {
        match self {
            Self::Comment(_) |
            Self::Element(_) => None,
            Self::Text(cdata, _) => Some(cdata)
        }
    }
}




#[derive(Debug, PartialEq)]
pub struct Element<'a, A: NodeArena<'a>> {
    pub opener: OpeningTagSpan<'a>,
    pub closer: Option<ClosingTagSpan<'a>>,
    pub children: A::NodeReferenceList,
}
impl<'a, A: NodeArena<'a>> Element<'a, A> {
    fn parse_after_opening(input: &Span<'a>, opener: OpeningTagSpan<'a>, arena: &mut A) -> Result<Element<'a, A>, NodeParseError<'a, A>>  {
        if opener.is_self_closing() {
            return Ok(Self {
                opener,
                closer: None,
                children: A::NodeReferenceList::default()
            })
        };

        let mut children = A::NodeReferenceList::default();
        let mut after = unsafe { input.slice_bytes_inclusive(opener.span.length, None) };

        if &*opener.get_name_span() == "?xml" {
            return Ok(Self {
                opener,
                closer: None,
                children
            });
        }


        loop {
            if let Some(closer) = parse_closing_tag(after) {
                if closer.get_name_span() != opener.get_name_span() {
                    return Err(NodeParseError::ElementDidNotClose(opener))
                }
                return Ok(Self {
                    opener,
                    closer: Some(closer),
                    children
                })
            }

            if let Some(next) = Node::parse(&after, arena)? {
                let Read { value: index, consumed_bytes } = next;
                after = unsafe { after.slice_bytes_inclusive(consumed_bytes, None) };
                children.add(index).map_err(NodeParseError::ArenaReferenceListPushFailure)?;
            } else {
                return Err(NodeParseError::ElementDidNotClose(opener))
            }
        }
    }

    pub fn attributes(&self) -> &Attributes {
        &self.opener.attributes
    }

    pub fn tag_name(&self) -> &'a str {
        self.opener.get_name_span().as_str()
    }

    pub fn is_self_closing(&self) -> bool {
        self.opener.is_self_closing()
    }

    pub fn span(&self) -> Span<'a> {
        let opener = self.opener.span;
        let end = self.closer.as_ref().map(|closer| NonZero::new(closer.span.offset + closer.span.length - opener.offset).unwrap());
        unsafe { opener.slice_bytes_inclusive_allow_oob(0, end) }
    }
}


#[derive(Debug, PartialEq)]
pub struct ClosingTagSpan<'a> {
    pub span: Span<'a>,
    name_length: usize
}
impl<'a> ClosingTagSpan<'a> {
    pub fn get_name_span(&self) -> Span<'a> {
        const OFFSET: usize = const { "</".len() };
        unsafe { self.span.slice_bytes_inclusive(OFFSET, Some(NonZero::new_unchecked(OFFSET + self.name_length)) ) }
    }
}
// TODO: error on closing tag not terminating
fn parse_closing_tag(span: Span<'_>) -> Option<ClosingTagSpan<'_>> {
    if !span.starts_with("</") { return None }
    let end = span.find(">").unwrap();
    // TODO: allow whitespace where applicable, only allow valid characters
    Some(ClosingTagSpan {
        span: unsafe { span.slice_bytes_inclusive(0, Some(NonZero::new_unchecked(end + 1))) },
        name_length: end - 2,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    mod section_opener {
        use super::*;

        macro_rules! p {
            ($value: expr) => {
                SectionOpener::parse(&Span::new_root($value))
            };
        }

        #[test]
        fn basic() {
            assert_eq!(p!("jor"), Ok(None));
            assert_eq!(p!(" <hogwash />"), Ok(None));
    
            let input = "<gender>coolio</gender>";
            let output = p!(input);
            let output = if let Ok(Some(SectionOpener::Tag(tag))) = output { tag } else { panic!() };
            assert!(matches!(output, OpeningTagSpan {
                name_length: 6, // "gender".len()
                attributes: _,
                span: Span {
                    top: tag_ref_in,
                    offset: 0,
                    length: 8, // "<gender>".len()
                    lifetime: _,
                }
            } if tag_ref_in == input.as_ptr()));
            assert_eq!(&output.get_name_span(), "gender");
            assert!(matches!(output.get_name_span(), Span {
                top: name_ref_in,
                offset: 1, // '<'.len_utf8(),
                length: 6, // "gender".len()
                lifetime: _,
            } if name_ref_in == input.as_ptr()));
        }
    
        #[test]
        fn cdata_block() {
            assert!(matches!(p!(CharacterDataBlockSpan::OPENER), Ok(Some(SectionOpener::CharacterData(Span {
                offset: 0,
                length: 9, // "<![CDATA[".len()
                ..
            })))));
        }

        #[test]
        fn comment_block() {
            assert!(matches!(p!(CommentSpan::OPENER), Ok(Some(SectionOpener::Comment(Span {
                offset: 0,
                length: 4, // "<!--".len()
                ..
            })))));
        }


        // TODO: Write tests.
        #[test]
        fn attributes() {
            assert!(matches!(p!("<asdf jor='ready up!'"), Err(SectionOpenerReadError::TagDidNotClose(..))));
        }
    }

}
