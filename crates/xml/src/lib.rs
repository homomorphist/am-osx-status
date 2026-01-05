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
        #[error("tag opened at {} did not close", .0.start_location())]
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

        #[error("section opened at {} did not close", .0.start_location())]
        NonNestingDidNotClose(super::Span<'a>, super::NonNestingSection),

        #[error("element opened at {} did not close", .0.span.start_location())]
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
    // TODO: Use an arena for this too?
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

pub mod block_span {
    use super::Span;

    pub mod side {
        use super::BlockSpan;

        pub(crate) trait BlockSpanSide: core::fmt::Display + core::fmt::Debug {
            const IS_OPENER: bool;
            const IS_CLOSER: bool = !Self::IS_OPENER;
            const VALUE: Side;
            const DISPLAY: &'static str;
        }

        pub enum Side {
            Opener,
            Closer
        }


        pub mod variant {
            #[derive(Debug)]
            pub struct Opener;
            #[derive(Debug)]
            pub struct Closer;
            impl super::BlockSpanSide for Opener { const VALUE: super::Side = super::Side::Opener; const DISPLAY: &'static str = "opener"; const IS_OPENER: bool = true;  }
            impl super::BlockSpanSide for Closer { const VALUE: super::Side = super::Side::Closer; const DISPLAY: &'static str = "closer"; const IS_OPENER: bool = false; }
            impl core::fmt::Display for Opener { fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { f.write_str(<Self as super::BlockSpanSide>::DISPLAY) } }
            impl core::fmt::Display for Closer { fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { f.write_str(<Self as super::BlockSpanSide>::DISPLAY) } }
        }


        #[derive(Debug)]
        #[expect(private_bounds)]
        pub struct BadBlockSpanSide<'a, T: BlockSpan<'a>, S: BlockSpanSide> {
            /// What was encountered instead of the expected encloser.
            /// The length of the span is the length of the encloser, starting from where the encloser should've started (zero in the case of an opener, or the length of the passed span minus the length of the closer).
            pub got: super::Span<'a>,
            /// The side which had the malformed encloser.
            pub side: core::marker::PhantomData<S>,
            /// The block span variant attempting to be constructed.
            pub block: core::marker::PhantomData<T>
        }

        #[expect(private_bounds)]
        impl<'a, T: BlockSpan<'a>, S: BlockSpanSide> BadBlockSpanSide<'a, T, S> {
            pub fn new(got: super::Span<'a>) -> Self {
                Self {
                    got,
                    side: core::marker::PhantomData,
                    block: core::marker::PhantomData,
                }
            }
        }

        impl<'a, T: BlockSpan<'a>, S: BlockSpanSide> core::error::Error for BadBlockSpanSide<'a, T, S> {}
        impl<'a, T: BlockSpan<'a>, S: BlockSpanSide> core::fmt::Display for BadBlockSpanSide<'a, T, S> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, "bad {} on {} block span (got {}, expected {})", S::DISPLAY, T::NAME, self.got, T::get_side(S::VALUE))
            }
        }

    }

    use side::*;


    #[derive(Debug, thiserror::Error)]
    pub enum CloserError<'a, T: BlockSpan<'a>>  {
        /// The block span contained an extra instance of the closer within the body.
        #[error("unexpected closer appearance in block body at relative index {starting_at}")]
        PresentWithin { starting_at: usize },
        /// The block span did not terminate with the expected closer.
        #[error(transparent)]
        NotTerminating(side::BadBlockSpanSide<'a, T, side::variant::Closer>)
    }
    impl<'a, T: BlockSpan<'a>> From<side::BadBlockSpanSide<'a, T, side::variant::Closer>> for CloserError<'a, T> {
        fn from(value: side::BadBlockSpanSide<'a, T, side::variant::Closer>) -> Self {
            Self::NotTerminating(value)
        }
    }


    #[derive(Debug, thiserror::Error)]
    pub enum BlockSpanConstructionError<'a, T: BlockSpan<'a>> {
        #[error(transparent)]
        Opener(side::BadBlockSpanSide<'a, T, side::variant::Opener>),
        #[error(transparent)]
        Closer(CloserError<'a, T>)
    }
    impl<'a, T: BlockSpan<'a>> From<CloserError<'a, T>> for BlockSpanConstructionError<'a, T> {
        fn from(value: CloserError<'a, T>) -> Self {
            Self::Closer(value)   
        }
    }
    impl<'a, T:  BlockSpan<'a>> From<side::BadBlockSpanSide<'a, T, side::variant::Opener>> for BlockSpanConstructionError<'a, T> {
        fn from(value: side::BadBlockSpanSide<'a, T, side::variant::Opener>) -> Self {
            Self::Opener(value)
        }
    }
    impl<'a, T:  BlockSpan<'a>> From<side::BadBlockSpanSide<'a, T, side::variant::Closer>> for BlockSpanConstructionError<'a, T> {
        fn from(value: side::BadBlockSpanSide<'a, T, side::variant::Closer>) -> Self {
            Self::Closer(CloserError::NotTerminating(value))   
        }
    }

    pub trait BlockSpan<'a>: core::ops::Deref<Target = Span<'a>> + core::fmt::Debug {
        const OPENER: &'static str;
        const OPENER_BACKWARDS_SHIFT: isize = if Self::OPENER.len() >= isize::MAX as usize { panic!("opener length cannot fit in isize") } else { -(Self::OPENER.len() as isize) };
        const CLOSER: &'static str;
        const NAME: &'static str;

        fn get_side(side: Side) -> &'static str {
            match side {
                Side::Opener => Self::OPENER,
                Side::Closer => Self::CLOSER,
            }
        }

        fn as_span(&self) -> Span<'a>;

        fn get_supposed_opener(span: &Span<'a>) -> Span<'a> {
            span.slice_with(0..Self::OPENER.len())
        }
        fn get_supposed_closer(span: &Span<'a>) -> Span<'a> {
            span.slice_with((span.len() - Self::CLOSER.len())..)
        }
        fn get_supposed_content(span: &Span<'a>) -> Span<'a> {
            span.slice_with(Self::OPENER.len()..(span.len() - Self::CLOSER.len()))
        }

        fn new(span: Span<'a>) -> Result<Self, BlockSpanConstructionError<'a, Self>> where Self: Sized {
            let supposed_opener = Self::get_supposed_opener(&span);
            if supposed_opener != Self::OPENER { return Err(BlockSpanConstructionError::Opener(BadBlockSpanSide::new(supposed_opener))) };

            let supposed_closer = Self::get_supposed_closer(&span);
            if supposed_closer != Self::CLOSER { return Err(BlockSpanConstructionError::Closer(CloserError::NotTerminating(BadBlockSpanSide::new(supposed_closer)))) };

            if let Some(duplicate_index) = Self::get_supposed_content(&span).find(Self::CLOSER) {
                return Err(BlockSpanConstructionError::Closer(CloserError::PresentWithin { starting_at: duplicate_index }))
            };

            Ok(unsafe { Self::new_unchecked(span) })
        }

        /// # Safety
        /// - The span must start with [`Self::OPENER`].
        /// - The span must contain [`Self::CLOSER`] only once, that being at the end of the span.
        unsafe fn new_unchecked(span: Span<'a>) -> Self where Self: Sized;

        fn opener(&self) -> Span<'a> {
            Self::get_supposed_opener(self)
        }
        fn closer(&self) -> Span<'a> {
            Self::get_supposed_closer(self)
        }
        fn content(&self) -> Span<'a> {
            Self::get_supposed_content(self)
        }

        /// Returns `None` if the closer is not found.
        /// # Safety
        /// The input span must start with [`Self::OPENER`].
        unsafe fn parse_after_opening_unchecked(input: &Span<'a>) -> Option<Self> where Self: Sized {
            let closer_at = unsafe { input.slice_with(Self::OPENER.len()..) }.as_str().find(Self::CLOSER)?;
            unsafe { Some(Self::new_unchecked(input.slice_with(..closer_at + Self::OPENER.len() + Self::CLOSER.len()))) }
        }

        fn parse_after_opening(input: &Span<'a>) -> Result<Option<Self>, BadBlockSpanSide<'a, Self, side::variant::Opener>> where Self: Sized {
            let backwards_extended = input.slice_signed_clamping(Self::OPENER_BACKWARDS_SHIFT, input.len() + Self::OPENER.len());
            let supposed_opener = Self::get_supposed_opener(&backwards_extended);
            if supposed_opener != Self::OPENER { return Err(BadBlockSpanSide::new(supposed_opener) )}
            Ok(unsafe { Self::parse_after_opening_unchecked(&backwards_extended) })
        }
    }

    macro_rules! mk_block_span {
        ($ident: ident, $name: literal, [$l: literal, $r: literal]) => {
            #[repr(transparent)]
            #[derive(Debug, PartialEq, Clone, Copy)]
            pub struct $ident<'a>(Span<'a>);
            impl<'a> BlockSpan<'a> for $ident<'a> {
                const OPENER: &'static str = $l;
                const CLOSER: &'static str = $r;
                const NAME: &'static str = $name;
                unsafe fn new_unchecked(span: Span<'a>) -> Self { Self(span) }
                fn as_span(&self) -> Span<'a> { self.0 }
            }
            impl<'a> core::ops::Deref for $ident<'a> {
                type Target = Span<'a>;
                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }
        }
    }

    mk_block_span!(CommentSpan,            "comment", ["<!--",      "-->"]);
    mk_block_span!(CharacterDataBlockSpan, "cdata",   ["<![CDATA[", "]]>"]);

    #[test]
    fn basic() {
        macro_rules! test_with_span {
            ($span: ident) => {
                let content = "hello :3";
                let string = $span::OPENER.to_owned() + content + $span::CLOSER;
                let block = $span::new(Span::new_root(&string)).unwrap();
                assert_eq!(block.opener(), Span::new_root($span::OPENER));
                assert_eq!(block.closer(), $span::CLOSER);
                assert_eq!(block.closer().offset, $span::OPENER.len() + content.len());
                assert_eq!(block.content(), content);
                assert_eq!(block.content().offset, $span::OPENER.len());
            }
        }
    
        test_with_span!(CommentSpan);
        test_with_span!(CharacterDataBlockSpan);
    }
}


use block_span::{CharacterDataBlockSpan, CommentSpan, BlockSpan};

#[derive(Debug, PartialEq)]
pub struct OpeningTagSpan<'a> {
    pub span: Span<'a>,
    pub attributes: Attributes<'a>,
    name_length: usize,
}
impl<'a> OpeningTagSpan<'a> {
    pub fn get_name_span(&self) -> Span<'a> {
        const OFFSET: usize = const { '<'.len_utf8() };
        self.span.slice(OFFSET, self.name_length)
    }
    pub const fn is_self_closing(&self) -> bool {
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
            let span: Span<'_> = input.range(..CommentSpan::OPENER.len());
            return Ok(Some(SectionOpener::Comment(span)))
        }

        if input.starts_with(CharacterDataBlockSpan::OPENER) {
            let span: Span<'_> = input.range(..CharacterDataBlockSpan::OPENER.len());
            return Ok(Some(SectionOpener::CharacterData(span)))
        }

        let name_ends_at = input.find(['/', '>', ' ', '\t', '\r', '\n']).ok_or(SectionOpenerReadError::TagDidNotClose(*input))?;
        let name_last_character_index = input.find(' ').map(|v| v.min(name_ends_at)).unwrap_or(name_ends_at);
        let name_last_character_index = NonZeroUsize::new(name_last_character_index).ok_or(SectionOpenerReadError::InvalidTagName)?;
        let name_length = name_last_character_index.get() - '<'.len_utf8();
        let name = input.range(1..name_last_character_index.get());

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
                    if parsing.is_none() && (name != "xml" || *char != b'?') {
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
                        //  <tag [...]/>
                        //             |
                        //             └ Signifies the end of a tag; stops parsing attributes.
                        //
                        if crate::defs::WHITESPACE_U8.contains(char) { index += 1; continue }
                        if *char == b'>' { index += 1; break }
                        if *char == b'/' && input.as_bytes().get(index + 1) == Some(&b'>') {dbg!("meow"); index += 2; break }
                        parsing = Some(Parsing::Key);
                        parsing_started_at = Some(unsafe { NonZeroUsize::new_unchecked(index) });
                    } else {
                        debug_assert_eq!(parsing, Some(Parsing::Key));
                        // Continually read letters of the key until we reach either whitespace or a ".".
                        // If we don't read a "=", we'll match that in the next case. This is just to stop reading what we consider a "key". 
                        match char {
                            b' ' | b'\t' | b'\r' | b'\n' | b'=' => {
                                key = Some(input.range(parsing_started_at.unwrap().get()..index));
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
                    let span = input.slice_with((parsing_started_at.unwrap().get() + 1)..index);
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
            span: input.range(..index),
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
    pub const fn block(span: CharacterDataBlockSpan<'a>) -> CharacterDataSpan<'a> {
        CharacterDataSpan {
            as_block: span
        }
    }
    pub const fn abrupt_node(span: Span<'a>) -> CharacterDataSpan<'a> {
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
    const fn as_raw_span(&self) -> Span<'a> {
        unsafe { self.as_abrupt_node }
    }
    pub fn is_block(&self) -> bool {
        // TODO: And closer?
        self.as_raw_span().starts_with(CharacterDataBlockSpan::OPENER)
    }
    pub fn as_block_span(&self) -> Option<CharacterDataBlockSpan<'_>> {
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
        if input.is_empty() { return Ok(None) }

        let node = if let Some(opener) = SectionOpener::parse(input).map_err(|err| NodeParseError::BadSectionOpener(err))? {
            match opener {
                // TODO: Don't use `ok().flatten()` here; properly propagate errors.
                SectionOpener::Tag(opener) => Node::Element(Element::parse_after_opening(input, opener, arena)?),
                SectionOpener::Comment(opener) => Node::Comment(CommentSpan::parse_after_opening(input).ok().flatten().ok_or(NodeParseError::NonNestingDidNotClose(opener, NonNestingSection::Comment))?),
                SectionOpener::CharacterData(opener) => {
                    let span = CharacterDataBlockSpan::parse_after_opening(input).ok().flatten().ok_or(NodeParseError::NonNestingDidNotClose(opener, NonNestingSection::UnescapedCharacterData))?;
                    let text = XmlCharacterData::Plain(span.content().as_str());
                    Self::Text(text, CharacterDataSpan::block(span))
                },
            }
        } else {
            let text = match input.find('<') {
                Some(opener) => input.slice_with(..opener),
                None => *input,
            };
            Self::Text(XmlCharacterData::maybe_escaping(text.as_str()), CharacterDataSpan::abrupt_node(text))
        };

        let consumed_bytes = node.span().length;
        Ok(Some(Read { value: arena.add(node).map_err(NodeParseError::ArenaPushFailure)?, consumed_bytes }))
    }

    pub fn span(&self) -> Span<'a> {
        match self {
            Self::Comment(span) => span.as_span(),
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

    pub const fn as_element(&self) -> Option<&Element<'a, A>> {
        match self {
            Self::Element(element) => Some(element),
            Self::Comment(_) | Self::Text(..) => None,
        }
    }

    pub const fn as_cdata(&self) -> Option<&XmlCharacterData<'a>> {
        match self {
            Self::Comment(_) |
            Self::Element(_) => None,
            Self::Text(cdata, _) => Some(cdata)
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
        let mut after = input.range(opener.span.length..);

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
                after = after.range(consumed_bytes..);
                children.add(index).map_err(NodeParseError::ArenaReferenceListPushFailure)?;
            } else {
                return Err(NodeParseError::ElementDidNotClose(opener))
            }
        }
    }

    pub const fn attributes(&self) -> &Attributes<'_> {
        &self.opener.attributes
    }

    pub fn tag_name(&self) -> &'a str {
        self.opener.get_name_span().as_str()
    }

    pub const fn is_self_closing(&self) -> bool {
        self.closer.is_none()
    }

    pub const fn span(&self) -> Span<'a> {
        let opener = self.opener.span;
        match &self.closer {
            None => opener, // self-closing, represented just by opener
            Some(closer) => unsafe { Span::new_unchecked(opener.top, opener.offset, closer.span.offset - opener.offset + closer.span.length) }
        }
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
        self.span.range(OFFSET..(OFFSET + self.name_length))
    }
}
// TODO: error on closing tag not terminating
fn parse_closing_tag(span: Span<'_>) -> Option<ClosingTagSpan<'_>> {
    if !span.starts_with("</") { return None }
    let end = span.find(">").unwrap();
    // TODO: allow whitespace where applicable, only allow valid characters
    Some(ClosingTagSpan {
        span: span.range(..(end + 1)),
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

        macro_rules! general {
            ($input: literal, $extracted: literal, $self_closing: literal) => {
                {
                    let parsed = p!($input).unwrap().unwrap();
                    let SectionOpener::Tag(tag) = parsed else { panic!("wasn't parsed as an opener tag") };
                    assert_eq!(tag.get_name_span().as_str(), $extracted);
                    assert_eq!(tag.is_self_closing(), $self_closing);
                }
            }
        }

        #[test]
        fn basic() {
            assert_eq!(p!("jor"), Ok(None)); // no '<'
            assert_eq!(p!(" <hogwash />"), Ok(None)); // leading space
            general!("<tag>",      "tag", false);
            general!("<tag \n\r>", "tag", false);
            general!("<longer-tag-name>", "longer-tag-name", false);
    
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
            } if core::ptr::eq(tag_ref_in, input.as_ptr())));
            assert_eq!(&output.get_name_span(), "gender");
            assert!(matches!(output.get_name_span(), Span {
                top: name_ref_in,
                offset: 1, // '<'.len_utf8(),
                length: 6, // "gender".len()
                lifetime: _,
            } if core::ptr::eq(name_ref_in, input.as_ptr())));


            macro_rules! general {
                ($input: literal, $extracted: literal, $self_closing: literal) => {
                    {
                        let parsed = p!($input).unwrap().unwrap();
                        let SectionOpener::Tag(tag) = parsed else { panic!("wasn't parsed as an opener tag") };
                        assert_eq!(tag.get_name_span().as_str(), $extracted);
                        assert_eq!(tag.is_self_closing(), $self_closing);
                    }
                }
            }
        }

        #[test]
        fn self_closing() {
            general!("<tag/>",          "tag", true);
            general!("<tag    \r\t />", "tag", true);
        }
    
        #[test]
        fn attributes() {
            assert!(matches!(p!("<asdf jor='ready up!'"), Err(SectionOpenerReadError::TagDidNotClose(..))));
            assert!(matches!(p!("<input disabled>)"), Err(SectionOpenerReadError::AttributeParseError(AttributeParseError::ExpectedDelimiterAfterKey)))); // these types of boolean attributes are NOT valid XML
            let valid = p!("<tag attr=\"value\" another='value2'>").unwrap().unwrap();
            let SectionOpener::Tag(tag) = valid else { panic!("wasn't parsed as an opener tag") };
            assert!(!tag.is_self_closing());
            assert_eq!(tag.get_name_span().as_str(), "tag");
            assert_eq!(tag.attributes.0.len(), 2);
            assert!(tag.attributes.0.contains_key(&Span::new_root("attr")));
            assert!(tag.attributes.0.contains_key(&Span::new_root("another")));
        }

        mod blocks {
            use super::*;

            #[test]
            fn cdata() {
                assert!(matches!(p!(CharacterDataBlockSpan::OPENER), Ok(Some(SectionOpener::CharacterData(Span {
                    offset: 0,
                    length: 9, // "<![CDATA[".len()
                    ..
                })))));
            }

            #[test]
            fn comment() {
                assert!(matches!(p!(CommentSpan::OPENER), Ok(Some(SectionOpener::Comment(Span {
                    offset: 0,
                    length: 4, // "<!--".len()
                    ..
                })))));
            }
        }
    }

    mod elements {
        use crate::arena::vec::VecNodeArena;

        use super::*;
        
        #[test]
        fn several_in_a_row() {
            let input = "<tag>hello</tag><tag>world</tag>";
            let span = Span::new_root(input);
            let mut arena = VecNodeArena::default();
            let mut after = span;
            let mut children = Vec::new();
            while let Some(node) = Node::parse(&after, &mut arena).unwrap() {
                let Read { value: index, consumed_bytes } = node;
                after = after.range(consumed_bytes..);
                children.push(index);
            }

            macro_rules! check {
                ($element: expr, $expected_name: literal, $expected_text: literal) => {
                    let element = $element.as_element().expect("not an element");
                    assert_eq!(element.tag_name(), $expected_name);
                    assert_eq!(element.children.len(), 1);
                    let child_index = element.children.get(0).expect("no child");
                    let child = arena.get(child_index);
                    let text = child.as_cdata().expect("child not cdata");
                    assert_eq!(text.get().unwrap(), $expected_text);
                }
            }

            assert_eq!(children.len(), 2);
            check!(arena.get(&children[0]), "tag", "hello");
            check!(arena.get(&children[1]), "tag", "world");
        }
    }
}
