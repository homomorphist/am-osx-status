#![doc = include_str!("../README.md")]

mod balanced;
use std::{borrow::Cow, process::Stdio, sync::Arc};
use tokio::{io::{AsyncBufReadExt, AsyncWriteExt}, process::Child};


/// The reason a session couldn't process the input.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("interpreted input as multi-line (an expression didn't fully terminate)")]
    InterpretedAsMultiline {
        /// If true, this was a test performed at the start of the function, and if the execution state *wasn't already messed up*, it should be fine to continue.
        /// If false, that means the session is in a bad state because an input (not necessarily this one) passed the check but still ended up requiring multi-line input.
        preemptive: bool
    },
    #[error("unable to test expression for being multi-line")]
    FailedToTestForMultiline,
    #[error("the REPL process does not exist; it may have crashed or been killed")]
    ProcessDoesNotExist,
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

/// A language that can be run in the `osascript` CLI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Language {
    JavaScript,
    AppleScript
}
impl Language {
    pub const fn to_str(&self) -> &'static str {
        match self {
            Self::JavaScript => "JavaScript",
            Self::AppleScript => "AppleScript"
        }
    }
}
impl core::fmt::Display for Language {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.to_str())
    }
}
impl Default for Language {
    fn default() -> Self {
        Self::AppleScript
    }
}

/// A REPL session.
/// 
/// This is a wrapper around the `osascript` REPL, which is a REPL for AppleScript and JavaScript for Automation (JXA).
/// 
/// Dropping the session will kill attempt to kill the REPL process, but it may not have an immediate effect.
/// 
/// It can be more performant to use a session when invoking many commands, but a caveat exists in that if an input is interpreted as multi-line, the session will be in a bad state.
/// If this is the case, the `run` function will return an `Error::InterpretedAsMultiline`, and the session should restarted (via `restart` or creating a new one).
/// 
/// # Important
/// 
/// This is an inherently more unstable method of evaluation, and buggy results can arise from odd inputs or outputs.
/// Be warned that it may unexpectedly hang or return invalid data.
/// 
pub struct Session {
    language: Language,
    process: Child,
    out: std::sync::Arc<tokio::io::BufReader<tokio::process::ChildStdout>>,
}
impl core::fmt::Debug for Session {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // let f_process = f.debug_tuple("PID").field(&self.process.id()).
        f.debug_struct("Session")
            .field("language", &self.language)
            .field("process",  &self.process)
            .finish_non_exhaustive()
    }
}
impl Session {
    pub async fn new(language: Language) -> Result<Self, std::io::Error> {
        // There's some funky stuff going on with stdout and such.
        // I thought it might have to do with needing a pseudo-terminal, but apparently all of the crates for that on macOS suck.
        // `faketty` did tell me that `script` can be used a similar manner, which thankfully did work, though there are still some quirks.
        let mut child = tokio::process::Command::new("script")
            .args([
                "-q", // quiet; no `script` extras
                "-F", // don't buffer output; flush immediately
                "/dev/null", // don't transcribe
                "osascript",
                "-l", language.to_str(),
                "-i",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let out = child.stdout.take().expect("cannot take stdout");
        let out = tokio::io::BufReader::new(out);
        let out = std::sync::Arc::new(out);

        let mut session = Self { process: child, out, language };

        // A preliminary evaluation needs to be performed because it decides to write it's own
        // command to stdout twice instead of once on the first execution, for whatever reason.
        session.writeline("1").await.map_err(|err| match err { SessionError::Io(io) => io, _ => unreachable!("invalid error variant for initial process state") })?;
        session.void_readline().await?; // command
        session.void_readline().await?; // command... again?
        ChunkRead::read_until_including(std::sync::Arc::get_mut(&mut session.out).unwrap(), b"\r\n>> ").await?; // read output + next prompt prefix

        Ok(session)
    }

    pub async fn javascript() -> Result<Self, std::io::Error> {
        Self::new(Language::JavaScript).await
    }

    pub async fn applescript() -> Result<Self, std::io::Error> {
        Self::new(Language::AppleScript).await
    }

    fn will_input_require_multiline(input: &str) -> Result<bool, balanced::InvalidCharacterPlacementError> {
        if input.contains("\n") {
            return Ok(true)
        } 
        Ok(!balanced::is_balanced(input, Language::JavaScript)?)
    }

    async fn writeline(&mut self, value: &str) -> Result<(), SessionError> {
        if !self.is_alive() { return Err(SessionError::ProcessDoesNotExist) }
        let mut stdin = self.process.stdin.take().expect("cannot take stdin");
        stdin.write_all(value.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        self.process.stdin.replace(stdin);
        Ok(())
    }

    async fn void_readline(&mut self) -> Result<(), std::io::Error> {
        ChunkRead::read_until_including(Arc::get_mut(&mut self.out).unwrap(), b"\r\n").await.unwrap();
        Ok(())
    }

    async fn read_until_new_prompt(&mut self) -> Result<Vec<u8>, std::io::Error> {
        let mut out = vec![];
        let mut first_run = false;
        let out_reader = Arc::get_mut(&mut self.out).expect("reader is not unique");

        while !first_run || !out_reader.buffer().is_empty() {
            first_run = true;

            // We can read a `>> `, but that doesn't necessarily mean that we finished and are waiting for another input,
            // since it could've just been part of something that was outputted.
            // As such, continue reading until the buffer doesn't fill itself up any more.
            out.extend_from_slice(&ChunkRead::read_until_including(out_reader, b">> ").await?[..]);

            // TODO: What if we perfectly aligned a read against the buffer size so that it's
            // considered empty despite there being more data simply awaiting a `fill_buff`?
        }

        Ok(out)
    }

    /// Attempts to run the provided code in the REPL.
    pub async fn run(&mut self, value: &str) -> Result<ReplOutput, SessionError> {
        if Self::will_input_require_multiline(value).map_err(|_| SessionError::FailedToTestForMultiline)? {
            return Err(SessionError::InterpretedAsMultiline { preemptive: true })
        }
;
        self.writeline(value).await?;

        if !value.is_empty() {
            self.void_readline().await?;
            if Arc::get_mut(&mut self.out).expect("reader is not unique").fill_buf().await? == b"?> " {
                return Err(SessionError::InterpretedAsMultiline { preemptive: false })
            }
        }

        let mut out = self.read_until_new_prompt().await?;
        out.truncate(out.len() - b"\r\n>> ".len());
        let raw = RawReplOutput(out);
        let out = ReplOutput { raw };
        Ok(out)
    } 

    /// Kills the REPL session.
    pub async fn kill(&mut self) -> Result<(), std::io::Error> {
        self.process.kill().await?;
        Ok(())
    }

    /// Restarts the REPL session.
    pub async fn restart(&mut self) -> Result<(), std::io::Error> {
        // Previous self will be dropped and automatically killed.
        *self = Self::new(self.language).await?;
        Ok(())
    }

    /// Returns whether or not the current REPL session is still running.
    pub fn is_alive(&mut self) -> bool {
        self.process.id().is_some()
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.process.start_kill();
    }
}


pub(crate) mod iter {
    const CARRIAGE_RETURNED_NEWLINE: &[u8] = b"\r\n";
    const LEN: usize = CARRIAGE_RETURNED_NEWLINE.len();

    /// An iterator that yields the indexes of "\r\n" in the output.
    pub struct CarriageReturnedNewlineIndexIterator<'a> {
        pub(crate) data: &'a [u8],
        pub(crate) l: usize,
        pub(crate) r: usize,
    }
    impl<'a> CarriageReturnedNewlineIndexIterator<'a> {
        pub const fn new(data: &'a [u8]) -> Self {
            Self { data, l: 0, r: data.len() }
        }
    }
    impl Iterator for CarriageReturnedNewlineIndexIterator<'_> {
        type Item = usize;
        fn next(&mut self) -> Option<Self::Item> {
            let index = self.data[self.l..self.r].windows(LEN).position(|x| x == CARRIAGE_RETURNED_NEWLINE)?;
            self.l += index + LEN;
            Some(self.l - LEN)
        }
    }
    impl core::iter::FusedIterator for CarriageReturnedNewlineIndexIterator<'_> {}
    impl core::iter::DoubleEndedIterator for CarriageReturnedNewlineIndexIterator<'_> {
        fn next_back(&mut self) -> Option<Self::Item> {
            let index = self.data[self.l..self.r].windows(LEN).rposition(|x| x == CARRIAGE_RETURNED_NEWLINE)?;
            self.r = self.l + index;
            Some(self.r)
        }
    }

    /// An iterator that returns lines (sequences of bytes between carriage-returned newlines) in the output.
    pub struct CarriageReturnedLinesIterator<'a> {
        sub: CarriageReturnedNewlineIndexIterator<'a>,
        done: bool,
    }
    impl<'a> CarriageReturnedLinesIterator<'a> {
        pub const fn new(data: &'a [u8]) -> Self {
            Self { sub: CarriageReturnedNewlineIndexIterator::new(data), done: false }
        }
    }
    impl<'a> Iterator for CarriageReturnedLinesIterator<'a> {
        type Item = &'a [u8];
        fn next(&mut self) -> Option<Self::Item> {
            if self.done { return None }
            let start = self.sub.l;
            let end;
            if let Some(l) = self.sub.next() {
                end = l;
            } else {
                self.done = true;
                end = self.sub.r;
            }
            Some(&self.sub.data[start..end])
        }
    }
    impl core::iter::FusedIterator for CarriageReturnedLinesIterator<'_> {}
    impl core::iter::DoubleEndedIterator for CarriageReturnedLinesIterator<'_> {
        fn next_back(&mut self) -> Option<Self::Item> {
            if self.done { return None }
            let start;
            let end = self.sub.r;
            if let Some(r) = self.sub.next_back() {
                start = r + LEN;
            } else {
                self.done = true;
                start = self.sub.l;
            }
            Some(&self.sub.data[start..end])
        }
    }
}

#[derive(Debug)]
pub struct RawReplOutput(Vec<u8>);
impl RawReplOutput {
    /// Returns the raw output; the bytes that were put to stdout/stderr, including any logged data and the output prefix (`=> `).
    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
    
    /// Returns the raw output as a lossy string; the characters that were put to stdout/stderr, including any logged strings and the output prefix (`=> `).
    pub fn as_lossy_str(&self) -> Cow<str> {
        String::from_utf8_lossy(&self.0[..])
    }

    /// Returns an iterator that yields lines in the output.
    fn lines(&self) -> iter::CarriageReturnedLinesIterator {
        iter::CarriageReturnedLinesIterator::new(&self.0)
    }

    /// Returns the presumed returned output; the bytes that followed the final instance of "\r\n=> or "=> " in the output.
    /// If it couldn't be found for whatever reason, `None` will be returned.
    pub fn get_likely_returned(&self) -> Option<&[u8]> {
        const MATCH: &[u8] = b"=> ";
        for line in self.lines().rev() {
            if line.starts_with(MATCH) {
                let offset = line.as_ptr() as usize - self.0.as_ptr() as usize;
                return Some(&self.0[offset + MATCH.len()..])
            }
        }
        None
    }

    /// Returns the presumed returned output as a lossy string; the characters that followed the final instance of "=> " or "=> " in the output.
    /// If it couldn't be found for whatever reason, `None` will be returned.
    pub fn get_likely_returned_as_lossy_str(&self) -> Option<Cow<str>> {
        self.get_likely_returned().map(|x| String::from_utf8_lossy(x))
    }
}
impl From<RawReplOutput> for Vec<u8> {
    fn from(val: RawReplOutput) -> Self {
        val.0
    }
}
impl From<Vec<u8>> for RawReplOutput {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}
impl AsRef<[u8]> for RawReplOutput {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
impl core::ops::Deref for RawReplOutput {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl core::ops::DerefMut for RawReplOutput {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl From<ReplOutput> for RawReplOutput {
    fn from(value: ReplOutput) -> Self {
        value.raw
    }
}
impl<'a> From<&'a ReplOutput> for &'a RawReplOutput {
    fn from(value: &'a ReplOutput) -> Self {
        &value.raw
    }
}

pub struct ReplOutput { pub raw: RawReplOutput }
impl ReplOutput {
    /// Returns what is plausibly (but not certainly) the outputted value as a result of running the expression.
    pub fn guess(&self) -> Option<Cow<str>> {
        self.raw.get_likely_returned_as_lossy_str()
    }
}

pub(crate) trait ChunkRead<'a> {
    /// cancel-safe
    async fn get_chunk_view(&mut self) -> Result<&[u8], std::io::Error>;
    async fn shift(&mut self, amount: usize);
    async fn read_until_including(&mut self, query: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let mut out = vec![];
        let mut matching = 0;
        'view: loop {
            let view = self.get_chunk_view().await?;
            
            let mut start_offset = 0;
            let mut sequential = 0;

            while matching != query.len() {
                let index = start_offset + sequential;
                if index < view.len() {
                    let char = view[index];
                    if char == query[matching] {
                        matching += 1;
                        sequential += 1;
                        continue;
                    } else {
                        matching = 0;
                        sequential = 0;
                        if char == query[matching] {
                            matching += 1;
                            sequential += 1;
                        } else {
                            start_offset += 1;
                        }
                        continue;
                    }
                }

                out.extend_from_slice(view);
                let view = view.len();
                self.shift(view).await;
                continue 'view;
            }

            let read = start_offset + sequential;
            out.extend_from_slice(&view[..read]);
            self.shift(read).await;

            return Ok(out)
        }
    }
}
impl ChunkRead<'_> for tokio::io::BufReader<tokio::process::ChildStdout> {
    async fn get_chunk_view(&mut self) -> Result<&[u8], std::io::Error> {
        AsyncBufReadExt::fill_buf(self).await
    }
    async fn shift(&mut self, amount: usize) {
        AsyncBufReadExt::consume(self, amount);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Session, Language, SessionError};

    mod chunk_read {
        use crate::ChunkRead;

        #[derive(Clone)]
        struct MockChunkedRead {
            data: &'static [&'static [u8]],
            pos: usize
        }
        impl MockChunkedRead {
            pub const fn new(data: &'static [&'static [u8]]) -> Self {
                Self {
                    data,
                    pos: 0
                }
            }
        }
        impl ChunkRead<'_> for MockChunkedRead {
            async fn get_chunk_view(&mut self) -> Result<&[u8], std::io::Error> {
                let mut idx = self.pos;
                for chunk in self.data.iter() {
                    match idx.checked_sub(chunk.len()) {
                        Some(sub) => { idx = sub }
                        None => return Ok(&chunk[idx..])
                    }
                }
                panic!("oob @ idx = {idx}")
            }
            async fn shift(&mut self, amount: usize) {
                self.pos += amount
            }
        }
  
        const MOCK: MockChunkedRead = MockChunkedRead::new(&[
            b"This is an output.\r",
            b"\n>",
            b"> Foo.",
        ]);

        #[tokio::test]
        async fn proper_mock_impl() {
            let mut mock = MOCK.clone();
            
            assert_eq!(mock.get_chunk_view().await.unwrap(), b"This is an output.\r");
            mock.shift(3).await;
            assert_eq!(mock.get_chunk_view().await.unwrap(), &b"This is an output.\r"[3..]);
            mock.shift(&b"This is an output.\r"[3..].len() + 1).await;
            assert_eq!(mock.get_chunk_view().await.unwrap(), b">");
            mock.shift(1).await;
            assert_eq!(mock.get_chunk_view().await.unwrap(), b"> Foo.");
            mock.shift(b"> Foo".len()).await;
            assert_eq!(mock.get_chunk_view().await.unwrap(), b".");
        }
 
        #[tokio::test]
        async fn read_until() {
            assert_eq!(MOCK.clone().read_until_including(b"\r\n>> ").await.unwrap(), b"This is an output.\r\n>> ");
            assert_eq!(MOCK.clone().read_until_including(b"This is an output.").await.unwrap(), b"This is an output.");
            assert_eq!(MOCK.clone().read_until_including(b"> Foo.").await.unwrap(), b"This is an output.\r\n>> Foo.");
            assert_eq!(MockChunkedRead::new(&[b"awesome", b"sauce"]).read_until_including(b"awesome").await.unwrap(), b"awesome");
        }
    }


    mod carriage_returned_lines {
        use crate::iter::*;

        const DATA: &[u8] = b"This is an output.\r\n>> Foo.\r\n>> Bar.\r\n>> Baz.";

        #[test]
        fn indices() {
            // forward
            let mut iter = CarriageReturnedNewlineIndexIterator::new(DATA); 
            assert_eq!(iter.next().unwrap(), 18);
            assert_eq!(iter.next().unwrap(), 27);
            assert_eq!(iter.next().unwrap(), 36);
            assert!(iter.next().is_none());
            assert!(iter.next_back().is_none());
            // backward
            let mut iter = CarriageReturnedNewlineIndexIterator::new(DATA); 
            assert_eq!(iter.next_back().unwrap(), 36);
            assert_eq!(iter.next_back().unwrap(), 27);
            assert_eq!(iter.next_back().unwrap(), 18);
            assert!(iter.next_back().is_none());
            assert!(iter.next().is_none());
            // intermixed
            let mut iter = CarriageReturnedNewlineIndexIterator::new(DATA);
            assert_eq!(iter.next_back().unwrap(), 36);
            assert_eq!(iter.next().unwrap(), 18);
            assert_eq!(iter.next_back().unwrap(), 27);
            assert!(iter.next().is_none());
            assert!(iter.next_back().is_none());
        }

        #[test]
        fn lines() {
            // forward
            let mut iter = CarriageReturnedLinesIterator::new(DATA); 
            assert_eq!(String::from_utf8_lossy(iter.next().unwrap()), "This is an output.");
            assert_eq!(String::from_utf8_lossy(iter.next().unwrap()), ">> Foo.");
            assert_eq!(String::from_utf8_lossy(iter.next().unwrap()), ">> Bar.");
            assert_eq!(String::from_utf8_lossy(iter.next().unwrap()), ">> Baz.");
            assert!(iter.next().is_none());
            assert!(iter.next_back().is_none());
            // // backward
            let mut iter = CarriageReturnedLinesIterator::new(DATA);
            assert_eq!(String::from_utf8_lossy(iter.next_back().unwrap()), ">> Baz.");
            assert_eq!(String::from_utf8_lossy(iter.next_back().unwrap()), ">> Bar.");
            assert_eq!(String::from_utf8_lossy(iter.next_back().unwrap()), ">> Foo.");
            assert_eq!(String::from_utf8_lossy(iter.next_back().unwrap()), "This is an output.");
            assert!(iter.next().is_none());
            assert!(iter.next_back().is_none());
            // // intermixed
            let mut iter = CarriageReturnedLinesIterator::new(DATA);
            assert_eq!(String::from_utf8_lossy(iter.next_back().unwrap()), ">> Baz.");
            assert_eq!(String::from_utf8_lossy(iter.next().unwrap()), "This is an output.");
            assert_eq!(String::from_utf8_lossy(iter.next_back().unwrap()), ">> Bar.");
            assert_eq!(String::from_utf8_lossy(iter.next().unwrap()), ">> Foo.");
            assert!(iter.next().is_none());
            assert!(iter.next_back().is_none());
        }
    }

    #[tokio::test]
    async fn answer_extraction() {
        let mut session = Session::new(Language::JavaScript).await.unwrap();

        let out = session.run("12345").await.unwrap();
        assert_eq!(out.raw.as_lossy_str(), "=> 12345");
        assert_eq!(out.guess().unwrap(), "12345");

        let out = session.run("console.log(\"\\n>> pranked\\n=> lol\"); \"=> yea\"").await.unwrap();
        assert_eq!(out.raw.as_lossy_str(), "\r\n>> pranked\r\n=> lol\r\n=> \"=> yea\"");
        assert_eq!(out.guess().unwrap(), "\"=> yea\"");

        let mut session = Session::new(Language::JavaScript).await.unwrap();
        assert!(matches!(session.run("`").await, Err(SessionError::InterpretedAsMultiline { preemptive: true })));


        assert!(session.kill().await.is_ok());
        assert!(matches!(session.run("hi").await, Err(SessionError::ProcessDoesNotExist)));
    }

    #[tokio::test]
    async fn multiline_input_prevention() {
        let mut session = Session::new(Language::JavaScript).await.unwrap();
        assert!(matches!(session.run("`").await, Err(SessionError::InterpretedAsMultiline { preemptive: true })));
    }

    #[tokio::test]
    async fn liveliness() {
        let mut session = Session::new(Language::JavaScript).await.unwrap();
        assert!(session.is_alive());
        assert!(session.kill().await.is_ok());
        assert!(!session.is_alive());
        for _ in 1..=2 {
            assert!(session.restart().await.is_ok());
            assert!(session.is_alive());
        }
    }
}
