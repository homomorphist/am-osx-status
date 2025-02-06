#[derive(Debug)]
pub struct InvalidTokenError;
impl core::error::Error for InvalidTokenError {}
impl core::fmt::Display for InvalidTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid token")
    }
}


#[derive(Debug)]
pub struct MalformedInputError { message: String }
impl core::error::Error for MalformedInputError {}
impl core::fmt::Display for MalformedInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("malformed: ")?;
        f.write_str(&self.message)
    }
}


#[derive(Debug)]
pub struct ListenDateTooHistoric;
impl core::error::Error for ListenDateTooHistoric {}
impl core::fmt::Display for ListenDateTooHistoric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("timestamp is below earliest allowed listening date")
    }
}


