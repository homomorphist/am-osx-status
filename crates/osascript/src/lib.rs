#![doc = include_str!("../README.md")]
#![allow(unused)]

use std::{borrow::{Borrow, Cow}, io::{Read, Write}, process::Stdio, sync::atomic::AtomicBool, time::Duration};
use tokio::{io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader}, process::{Child, Command}};


#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("interpreted input as multi-line")]
    InterpretedAsMultiline,
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy)]
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


pub struct Session {
    child: Child,
    out_buf_write_lock_tx: tokio::sync::watch::Sender<bool>,
    out_buf_write_lock_rx: tokio::sync::watch::Receiver<bool>,
    out: std::sync::Arc<tokio::io::BufReader<tokio::process::ChildStdout>>,
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
        let (tx, rx) = tokio::sync::watch::channel(false);

        let mut session = Self {
            child,
            out_buf_write_lock_tx: tx,
            out_buf_write_lock_rx: rx,
            out,
        };

        // A preliminary evaluation needs to be performed because it decides to write it's own
        // command to stdout twice instead of once on the first execution, for whatever reason.
        session.writeline("1").await?;
        session.void_readline().await?; // command
        session.void_readline().await?; // command... again?
        ChunkRead::read_until_including(std::sync::Arc::get_mut(&mut session.out).unwrap(), b"\r\n>> ", None).await?; // read output + next prompt prefix

        Ok(session)
    }

    async fn writeline(&mut self, value: &str) -> Result<(), std::io::Error> {
        let mut stdin = self.child.stdin.take().unwrap();
        stdin.write_all(value.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        self.child.stdin.replace(stdin);
        Ok(())
    }

    async fn void_readline(&mut self) -> Result<(), std::io::Error> {
        ChunkRead::read_until_including(std::sync::Arc::get_mut(&mut self.out).unwrap(), b"\r\n", None).await.unwrap();
        Ok(())
    }

    async fn read_until_new_prompt(&mut self) -> Result<Vec<u8>, Error> {
        const MULTILINE_INDICATOR: &[u8; 2] = b"?>";

        // Delay if it hasn't finished in 200ms to check buffer is exclusively multiline indicator.
        // If so, odds are it's awaiting more input, so we should terminate.
        let (dl_tx, mut dl_rx) = tokio::sync::watch::channel(false);
        tokio::spawn(async move {
            tokio::time::sleep(core::time::Duration::from_millis(200)).await;
            dl_tx.send(true).expect("could not send force-yield");
            dbg!("sent");

        });

        let mut out = vec![];
        let mut out_reader = std::sync::Arc::get_mut(&mut self.out).unwrap();
        let mut first_run = false;

        while !first_run || !out_reader.buffer().is_empty() {
            dbg!("dawg");
            first_run = true;
            // We read a ">> ", but there was already more after it.
            // This means it was part of a string or some other external data, and not part of the REPL.
            // As such, continue reading.
            dbg!("<read>");
            out.extend_from_slice(&ChunkRead::read_until_including(out_reader, b">> ", Some(&mut dl_rx)).await?[..]);
            dbg!("</read>");

            // TODO: What if we like, perfectly aligned a read with the end of the buffer but there actually is more pending data?
            // I think that'd mean we'd need to use `self.out.fill_buf()`, but that can hang if we *did* reach the end.
            // So we could add a timeout, but, like... ugh.
        }

        dbg!("post mortem");

        // tokio::select!(
        //     out = async move {
        //         let mut out = ChunkRead::read_until_including(std::sync::Arc::get_mut(&mut out_mut).unwrap(), b">> ", Some(dl_rx)).await?;

        //         while !out_reader.buffer().is_empty() {
        //             // We read a ">> ", but there was already more after it.
        //             // This means it was part of a string or some other external data, and not part of the REPL.
        //             // As such, continue reading.
        //             out.extend_from_slice(&ChunkRead::read_until_including(out_reader, b">> ", Some(dl_rx)).await?[..])
        //             // TODO: What if we like, perfectly aligned a read with the end of the buffer but there actually is more pending data?
        //             // I think that'd mean we'd need to use `self.out.fill_buf()`, but that can hang if we *did* reach the end.
        //             // So we could add a timeout, but, like... ugh.
        //         }
                
        //         Ok(out)
        //     } => out,
        //     out = async move {
        //         tokio::time::sleep(core::time::Duration::from_millis(100)).await;
        //         let mut out_reader = out2.try_lock().unwrap();
        //         let pending_multiline = out_reader.buffer().windows(2).find(|window| window == MULTILINE_INDICATOR);
        //         Err::<Vec<u8>, Error>(Error::InterpretedAsMultiline)
        //     } => out
        // );

        unimplemented!()
    }

    pub async fn run(&mut self, value: &str) -> Result<ReplOutput, Error> {
        self.writeline(value).await?;
        if !value.is_empty() {
            self.void_readline().await?;
        }

        let mut out = self.read_until_new_prompt().await?;
        out.truncate(out.len() - b"\r\n>> ".len());
        Ok(ReplOutput { raw: out })
    } 
}

#[derive(Debug)]
pub struct ReplOutput {
    pub raw: Vec<u8>
}
impl ReplOutput {
    pub fn into_inner(self) -> Vec<u8> {
        self.raw
    }
    
    pub fn as_lossy_str(&self) -> Cow<str> {
        String::from_utf8_lossy(&self.raw[..])
    }


    // pub fn into_output() -> Result<String, String> {

    // }
}


pub(crate) trait ChunkRead<'a> {
    /// cancel-safe
    async fn peek_current_chunk(&self) -> &[u8];
    async fn get_chunk_view(&mut self) -> Result<&[u8], std::io::Error>;
    async fn shift(&mut self, amount: usize);


    // the standard fucking `read_until` does NOT want to cooperate, so i had to resort to this
    async fn read_until_including(&mut self, query: &[u8], mut pause: Option<&mut tokio::sync::watch::Receiver<bool>>) -> Result<Vec<u8>, std::io::Error> {
        let mut out = vec![];
        let mut matching = 0;
        'view: loop {
            // let view = self.get_chunk_view().await?;
            let view = match &mut pause {
                None => self.get_chunk_view().await?,
                Some(pause) => {
                    pause.wait_for(|paused| !paused).await.expect("channel closed");
                    tokio::select! {
                        view = self.get_chunk_view() => view?,
                        recv = pause.wait_for(|paused| *paused) => {
                            recv.expect("channel closed");
                            dbg!("wait, what?");
                            continue 'view
                        }
                    }
                }
            };

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
    async fn peek_current_chunk(&self) -> &[u8] {
        self.buffer()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{Session, Language};

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
            async fn peek_current_chunk(&self) -> &[u8] {
                unimplemented!()
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
            assert_eq!(MOCK.clone().read_until_including(b"\r\n>> ", None).await.unwrap(), b"This is an output.\r\n>> ");
            assert_eq!(MOCK.clone().read_until_including(b"This is an output.", None).await.unwrap(), b"This is an output.");
            assert_eq!(MOCK.clone().read_until_including(b"> Foo.", None).await.unwrap(), b"This is an output.\r\n>> Foo.");
            assert_eq!(MockChunkedRead::new(&[b"awesome", b"sauce"]).read_until_including(b"awesome", None).await.unwrap(), b"awesome");
        }
    }




    #[tokio::test]
    async fn basic() {
        let mut session = Session::new(Language::JavaScript).await.unwrap();

        dbg!(session.run("`").await.unwrap());

        // assert_eq!(session.run("12345").await.unwrap().as_lossy_str(), "=> 12345");
        // assert_eq!(session.run("console.log(\"\\n>> pranked\\n=> lol\"); \"=>\"").await.unwrap().as_lossy_str(), "\r\n>> pranked\r\n=> lol\r\n=> \"=>\"");


    }
}






