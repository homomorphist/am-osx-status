#![allow(clippy::result_large_err)]
use xml::{arena::{vec::{NodeIndex, VecNodeArena}, NodeArena}, cdata::XmlCharacterData, error::CharacterEntityDecodingError, span::Span, Element, Node};

type NA<'a> = xml::arena::vec::VecNodeArena<'a>;


#[derive(Debug)]
pub struct Deserializer<'de> {
    arena: VecNodeArena<'de>,
    stack: Vec<NodeIndex>, // [0] = highest, [n] = deepest,
}


impl<'de> Deserializer<'de> {
    pub fn parse(input: &'de str) -> Result<Option<Self>, Error<'de>> {
        let input = &input["<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n".len()..];
        // ^^ fixme so robust
        // dbg!(input);
        let mut arena = VecNodeArena::new();
        let input = Span::new_root(input);
        let index = xml::Node::parse(&input, &mut arena).map_err(Error::ParseError)?;
        let index = if let Some(xml::Read { value: index, .. }) = index { index } else { return Ok(None) };
        Ok(Some(Self { 
            arena,
            stack: vec![index]
        }))
    }

    pub fn reconstruct(self) -> String {
        let mut out = String::new();

        fn write_node<'a>(arena: &VecNodeArena<'a>, index: NodeIndex, out: &mut String) {
            match arena.get(&index) {
                Node::Element(element) => {
                    out.push_str(&format!("<{}>", element.tag_name()));
                    for child in &element.children {
                        write_node(arena, *child, out);
                    }
                    out.push_str(&format!("</{}>", element.tag_name()));
                }
                Node::Text(text, _) => {
                    out.push_str(text.raw());
                }
                Node::Comment(comment) => {
                    out.push_str(&format!("<!--{}-->", comment.as_str()));
                }
            }
        }

        write_node(&self.arena, self.stack[0], &mut out);
        out
    }
}


#[derive(thiserror::Error, Debug)]
pub enum Error<'a> {
    #[error("{0}")]
    Custom(String),
    #[error("unknown tag \"{0}\"")]
    UnknownTag(&'a str),
    #[error("expected \"{}\" element to have no children @ {}", .0.opener.get_name_span(), .0.opener.span.start_location())]
    ExpectedEmpty(Element<'a, NA<'a>>),
    #[error("expected a key @ {}", .0.span().start_location())]
    ExpectedKey(xml::Node<'a, NA<'a>>),
    #[error("expected a value @ {}", .0.span().start_location())]
    ExpectedValue(xml::Node<'a, NA<'a>>),
    #[error("expected a text @ {}", .0.span().start_location())]
    ExpectedText(xml::Node<'a, NA<'a>>),
    #[error("expected an element @ {}", .0.span().start_location())]
    ExpectedElement(xml::Node<'a, NA<'a>>),
    #[error("expected only one child @ {}", .0.span().start_location())]
    ExpectedOnlyOneChild(xml::Node<'a, NA<'a>>),
    #[error("{0}")]
    ParseError(xml::error::NodeParseError<'a,  NA<'a>>),
    #[error("cannot decode character: {0}")]
    CharacterReferenceDecodingError(#[from] CharacterEntityDecodingError)
}
impl serde::de::Error for Error<'_> {
    fn custom<T>(msg: T) -> Self where T: core::fmt::Display {
        Self::Custom(msg.to_string())
    }
}


impl<'de> serde::de::Deserializer<'de> for &mut Deserializer<'de> {
    type Error = Error<'de>;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: serde::de::Visitor<'de> {
        let out = match self.get_current_node() {
            Node::Comment(..) => {
                if self.goto_next_sibling().is_none() {
                    visitor.visit_unit()
                } else {
                    return self.deserialize_any(visitor);
                }
            }
            Node::Text(text, _) => match text {
                // TODO: only conditionally permit this when parsing `key`
                XmlCharacterData::Plain(plain) => visitor.visit_borrowed_str(plain),
                XmlCharacterData::WithEntities(_) => {
                    let owned = if let XmlCharacterData::WithEntities(data) = self.take_current_node().into_cdata().unwrap() { data } else { unreachable!() };
                    visitor.visit_string(owned.into_string()?)
                }
            },
            Node::Element(element) => {
                let tag = element.tag_name();
                match tag {
                    "plist" => {
                        let mut children = element.children.iter()
                            .map(|index| (index, self.arena.take(index)))
                            .filter(|(_, node)| match node {
                                Node::Comment(_) => false,
                                Node::Text(text, _) if text.is_just_whitespace().unwrap() => false,
                                _ => true
                            });


                        let child = children.next();
                        if child.is_none() || children.next().is_some()  {
                            return Err(Error::ExpectedOnlyOneChild(self.take_current_node()))
                        }

                        let (index, node) = child.unwrap();
                        if node.as_element().is_none() {
                            return Err(Error::ExpectedElement(node))
                        };

                        self.arena.replace(index, node); // we took owned when iterating children
                        *self.stack.last_mut().unwrap() = *index;
                        return self.deserialize_any(visitor)
                    },

                    "true" |
                    "false" => {
                        if !element.is_self_closing() {
                            return Err(Error::ExpectedEmpty(self.take_current_node().into_element().unwrap()))
                        }

                        visitor.visit_bool(matches!(tag, "true"))
                    },
                    "key" |
                    "string" |
                    "integer" |
                    "real" |
                    "date" |
                    "data" => {
                        if tag == "data" {
                            unimplemented!()
                        }

                        match self.take_singular_child_as_text()? {
                            XmlCharacterData::Plain(plain) => visitor.visit_borrowed_str(plain),
                            XmlCharacterData::WithEntities(data) => visitor.visit_string(data.into_string()?)
                        }
                    },
                    "array" => {
                        self.goto_first_child()?;
                        let value = visitor.visit_seq(ArraySeq::new(self));
                        self.stack.pop();
                        value
                    },
                    "dict" => {
                        if self.goto_first_child()?.is_none() {
                            return visitor.visit_map(DictionarySequence::empty())
                        }
                        let value = visitor.visit_map(DictionarySequence::new(self));
                        self.stack.pop();
                        value
                    },
                    _ => {
                        return Err(Error::UnknownTag(tag))
                    }
                }
            }
        }?;

        self.goto_next_sibling();

        Ok(out)
    }
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error> where V: serde::de::Visitor<'de> {
        visitor.visit_some(self)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}
impl<'de> Deserializer<'de> {
    pub fn take_current_node(&self) -> Node<'de, NA<'de>> {
        self.arena.take(self.get_current_node_index())
    }
    pub fn get_current_node(&self) -> &Node<'de, NA<'de>> {
        self.arena.get(self.get_current_node_index())
    }
    pub fn get_current_node_index(&self) -> &NodeIndex {
        self.stack.last().expect("no valid node selected!?")
    }

    pub fn get_parent_node(&self) -> Option<&Node<'de, NA<'de>>> {
        self.stack.get(self.stack.len().checked_sub(2)?).map(|index| self.arena.get(index))
    }

    fn get_following_siblings(&self) -> impl Iterator<Item = (NodeIndex, &Node<'de, VecNodeArena<'de>>)> {
        static EMPTY: Vec<NodeIndex> = vec![];
        let current = self.get_current_node_index();
        let siblings = self.get_parent_node()
            .map(|parent| &parent.as_element().expect("parent should always be an element!?").children)
            .unwrap_or(&EMPTY);

        siblings.iter()
            .skip_while(move |index| *index != current)
            .skip(1)
            .map(|index| (*index, self.arena.get(index)))
    }

    /// Returns whether a non-whitespace node is selected.
    pub fn skip_whitespace(&mut self) -> Result<bool, CharacterEntityDecodingError> {
        while matches!(self.get_current_node(), Node::Text(text, _) if text.is_just_whitespace().map_err(Clone::clone)?) {
            if self.goto_next_sibling().is_none() { return Ok(false) }
        }
        Ok(true)
    }

    pub fn current_as_element_or_else(&self, func: fn(Node<'de, NA<'de>>) -> Error<'de>) -> Result<&xml::Element<'de, NA<'de>>, Error<'de>> {
        if let Some(element) = self.get_current_node().as_element() {
            Ok(element)
        } else {
            Err(func(self.take_current_node()))
        } 
    }

    pub fn current_as_element(&self) -> Result<&xml::Element<'de, NA<'de>>, Error<'de>> {
        self.current_as_element_or_else(Error::ExpectedElement)
    }

    pub fn take_singular_child(&self) -> Result<Node<'de, NA<'de>>, Error<'de>> {
        let mut children = self.current_as_element()?.children.iter()
            .map(|index| self.arena.take(index))
            .filter(|node| match node {
                Node::Comment(_) => false,
                Node::Text(text, _) if text.is_just_whitespace().unwrap_or(false) => false,
                _ => true
            });
        let child = children.next();
        let next = children.next();
        if child.is_none() || next.is_some()  {
            dbg!(&next);
            return Err(Error::ExpectedOnlyOneChild(self.take_current_node()))
        }
        Ok(child.unwrap())
    }

    pub fn take_singular_child_as_text(&self) -> Result<XmlCharacterData<'de>, Error<'de>> {
        let child = self.take_singular_child()?;
        if let Node::Text(text, _) = child { Ok(text) } else {
            Err(Error::ExpectedText(child))
        }
    }

    pub fn goto_next_sibling(&mut self) -> Option<&Node<'de, NA<'de>>> {
        let index = self.get_following_siblings().next()?.0;
        *self.stack.last_mut().unwrap() = index;
        Some(self.arena.get(&index))
    }

    pub fn goto_first_child(&mut self) -> Result<Option<&Node<'de, NA<'de>>>, Error<'de>> {
        Ok(self.current_as_element()?.children.first().copied().map(|index| {
            self.stack.push(index);
            self.arena.get(&index)
        }))
    }
}

struct ArraySeq<'a, 'de> {
    deserializer: &'a mut Deserializer<'de>,
}
impl<'a, 'de> ArraySeq<'a, 'de> {
    fn new(deserializer: &'a mut Deserializer<'de>) -> Self {
        // TODO: Precalculate length.
        Self {
            deserializer,
        }
    }
}
impl<'de> serde::de::SeqAccess<'de> for ArraySeq<'_, 'de> {
    type Error = Error<'de>;
    
    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error> where T: serde::de::DeserializeSeed<'de> {
        if !self.deserializer.skip_whitespace()? { return Ok(None) }
        self.deserializer.current_as_element()?;
        seed.deserialize(&mut *self.deserializer).map(Some)
    }
}


struct DictionarySequence<'a, 'de> {
    deserializer: Option<&'a mut Deserializer<'de>>,
}
impl<'a, 'de> DictionarySequence<'a, 'de> {
    fn new(deserializer: &'a mut Deserializer<'de>) -> Self {
        Self {
            deserializer: Some(deserializer),
        }
    }

    fn empty() -> Self {
        Self {
            deserializer: None,
        }
    }
}
impl<'de> serde::de::MapAccess<'de> for DictionarySequence<'_, 'de> {
    type Error = Error<'de>;
    
    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error> where K: serde::de::DeserializeSeed<'de> {
        let deserializer = if let Some(deserializer) = self.deserializer.as_mut() {
            deserializer.skip_whitespace()?;
            deserializer
        } else { return Ok(None) };


        let element = deserializer.current_as_element_or_else(Error::ExpectedValue)?;
        if element.tag_name() != "key" {
            return Err(Error::ExpectedKey(deserializer.take_current_node()))
        }

        seed.deserialize(&mut **deserializer).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error> where V: serde::de::DeserializeSeed<'de> {
        let deserializer = unsafe { self.deserializer.as_mut().unwrap_unchecked() };
        deserializer.skip_whitespace()?;

        let element = deserializer.current_as_element_or_else(Error::ExpectedValue)?;
        if element.tag_name() == "key" {
            return Err(Error::ExpectedValue(deserializer.take_current_node()))
        }

        let deserialized = seed.deserialize(&mut **deserializer).expect("ok");
        if deserializer.goto_next_sibling().is_none() {
            self.deserializer = None;
        }

        Ok(deserialized)
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    #[test]
    fn basic() {
        let input = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict><key>value</key><string>jor</string></dict>\n</plist>\n";
        let mut deserializer = super::Deserializer::parse(input)
            .expect("failed to parse")
            .expect("failed to parse");
        #[allow(unused)]
        #[derive(serde::Deserialize)]
        struct Test { value: Option<String> }
        Test::deserialize(&mut deserializer).expect("failed to deserialize");
    }
}
