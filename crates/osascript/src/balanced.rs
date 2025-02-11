#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Region {
    SingleQuote, 
    DoubleQuote,
    Parenthesis,
    SquareBracket,
    CurlyBracket,
    TemplateString,
    Escaping
}
impl Region {
    pub const fn can_escape_within(self) -> bool {
        self.is_quote()
    }

    /// Whether this string is a quote. This determines:
    /// - Whether the opener character is the same as the closer character.
    /// - Whether a character can be escaped within this region.
    pub const fn is_quote(self) -> bool {
        matches!(self, Region::SingleQuote | Region::DoubleQuote | Region::TemplateString)
    }

    pub const fn get_from_opener(opener: char) -> Option<Region> {
        match opener {
            '\'' => Some(Region::SingleQuote),
            '"' => Some(Region::DoubleQuote),
            '(' => Some(Region::Parenthesis),
            '[' => Some(Region::SquareBracket),
            '{' => Some(Region::CurlyBracket),
            '`' => Some(Region::TemplateString),
            '\\' => Some(Region::Escaping),
            _ => None
        }
    }

    pub const fn get_from_closer(closer: char) -> Option<Region> {
        match closer {
            '\'' => Some(Region::SingleQuote),
            '"' => Some(Region::DoubleQuote),
            ')' => Some(Region::Parenthesis),
            ']' => Some(Region::SquareBracket),
            '}' => Some(Region::CurlyBracket),
            '`' => Some(Region::TemplateString),
            _ => None
        }
    }
}


#[derive(Debug, PartialEq)]
pub struct InvalidCharacterPlacementError;
impl core::error::Error for InvalidCharacterPlacementError {}
impl core::fmt::Display for InvalidCharacterPlacementError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Invalid character placement")
    }
}

fn internal_get_imbalanced_stack(input: &str, allow_template_strings: bool) -> Result<Vec<Region>, InvalidCharacterPlacementError> {
    let mut interpolating = false;
    let mut stack = Vec::new();

    for char in input.chars() {
        let recent = stack.last().copied();

        if recent == Some(Region::Escaping) {
            stack.pop();
            continue;
        }

        if char == '\\' && recent.is_some_and(Region::can_escape_within) {
            stack.push(Region::Escaping);
            continue;
        }

        if recent == Some(Region::TemplateString) && char == '$' {
            interpolating = true;
        }

        if let Some(region) = Region::get_from_closer(char) {
            if recent == Some(region) {
                stack.pop();
            } else if region.is_quote() {
                if recent.is_some_and(Region::is_quote) {
                    // Quote is part of a string literal.
                    continue;
                } else {
                    if !allow_template_strings && region == Region::TemplateString {
                        return Err(InvalidCharacterPlacementError);
                    }
                    // Treat as an opener.
                    stack.push(region);
                }
            } else {
                return Err(InvalidCharacterPlacementError);
            }
        } else if let Some(region) = Region::get_from_opener(char) {
            if recent == Some(Region::TemplateString) && region == Region::CurlyBracket {
                if interpolating {
                    stack.push(region);
                    interpolating = false;
                }
            } else if recent.is_some_and(Region::is_quote) {
                debug_assert!(!region.is_quote()); // Would've been handled in the closer case.
                continue; // literal character
            } else {
                stack.push(region);
            }
        }
    };

    Ok(stack)
}

pub fn get_imbalanced_stack(input: &str, language: crate::Language) -> Result<Vec<Region>, InvalidCharacterPlacementError> {
    internal_get_imbalanced_stack(input, match language {
        crate::Language::JavaScript => true,
        crate::Language::AppleScript => false
    })
}

pub fn is_balanced(input: &str, language: crate::Language) -> Result<bool, InvalidCharacterPlacementError> {
    Ok(get_imbalanced_stack(input, language)?.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        assert_eq!(internal_get_imbalanced_stack("abc", true), Ok(vec![]));
        assert_eq!(internal_get_imbalanced_stack("abc(", true), Ok(vec![Region::Parenthesis]));
        assert_eq!(internal_get_imbalanced_stack("abc'asdf(", true), Ok(vec![Region::SingleQuote]));
        assert_eq!(internal_get_imbalanced_stack("abc'asdf'(", true), Ok(vec![Region::Parenthesis]));
        assert_eq!(internal_get_imbalanced_stack("abc'asdf\\'(", true), Ok(vec![Region::SingleQuote]));
        assert_eq!(internal_get_imbalanced_stack("{'\"hello\"' + (", true), Ok(vec![Region::CurlyBracket, Region::Parenthesis]));
        assert_eq!(internal_get_imbalanced_stack("`${'\"\\", true), Ok(vec![Region::TemplateString, Region::CurlyBracket, Region::SingleQuote, Region::Escaping]));
        assert_eq!(internal_get_imbalanced_stack("`${'\"\\", false), Err(InvalidCharacterPlacementError));
        assert_eq!(internal_get_imbalanced_stack(")", true), Err(InvalidCharacterPlacementError));
    }   
}
