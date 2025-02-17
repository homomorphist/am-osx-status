pub mod application;
pub mod track;

pub use application::ApplicationData;
pub use tokio::io::{AsyncWriteExt, AsyncReadExt, AsyncBufReadExt};
pub use track::Track;

const SERVER_JS: &str = include_str!("../non-rust/server.js");

pub mod error {
    #[derive(Debug, thiserror::Error)]
    pub enum SessionEvaluationError {
        #[error("couldn't deserialize value: {0}")]
        DeserializationFailure(#[from] serde_json::Error),
        #[error("couldn't extract output")]
        ValueExtractionFailure { output: osascript::repl::Output },
        #[error("internal osascript session failure: {0}")]
        SessionFailure(#[from] osascript::repl::Error),
        #[error("io failure: {0}")]
        IoFailure(#[from] tokio::io::Error),
    }
    
    #[derive(Debug, thiserror::Error)]
    pub enum SingleEvaluationError {
        #[error("couldn't deserialize failure: {0}")]
        DeserializationFailure(#[from] serde_json::Error),
        #[error("couldn't extract output")]
        ValueExtractionFailure { output: osascript::repl::Output },
        #[error("io failure: {0}")]
        IoFailure(#[from] tokio::io::Error),
    }
}

#[derive(Debug)]
pub struct Session {
    pid: u32,
    socket: tokio::net::UnixStream,
}
impl Session {
    pub async fn new(socket_path: impl AsRef<std::path::Path>) -> Result<Self, std::io::Error> {
        let mut handle = osascript::spawn(SERVER_JS, osascript::Language::JavaScript, [
            socket_path.as_ref().to_str().expect("invalid socket path")
        ]).await?;


        let pid = handle.internal.id();
        let mut stderr = handle.internal.stderr.take().expect("no stderr");

        tokio::spawn(async move {
            handle.internal.wait().await.unwrap()
        });
        
        let mut buffer = Vec::new();
        stderr.read_buf(&mut buffer).await?;
        if buffer != b"Listening for connections...\n" {
            panic!("invalid server output: {}", String::from_utf8_lossy(&buffer));
        }

        let socket = tokio::net::UnixStream::connect(socket_path).await?;

        Ok(Self {
            pid: pid.expect("no pid"),
            socket
        })
    }

    async fn exec<T>(&mut self, message: &str) -> Result<T, error::SessionEvaluationError> where T: serde::de::DeserializeOwned {
        self.socket.write_all(message.as_bytes()).await?;
        self.socket.flush().await?;
        let mut buffer = [0; 1024];
        let mut json = String::new();
        loop {
            let mut done = false;
            let mut bytes = self.socket.read(&mut buffer).await?;
            if bytes == 0 { break; }
            if buffer[bytes - 1] == b'\0' { bytes -= 1; done = true; }
            json.push_str(std::str::from_utf8(&buffer[..bytes]).map_err(|_| {
                <serde_json::Error as serde::de::Error>::custom("invalid utf-8")
            })?);
            if done { break; }
        };

        serde_json::from_str(&json).map_err(error::SessionEvaluationError::DeserializationFailure)
    }

    pub async fn application(&mut self) -> Result<ApplicationData, error::SessionEvaluationError> {
        self.exec("application").await.map(ApplicationData::fix)
    }

    pub async fn now_playing(&mut self) -> Result<Option<crate::Track>, error::SessionEvaluationError> {
        self.exec("current track").await
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        // omg this is horrible pls
        std::process::Command::new("kill")
            .arg("-9")
            .arg(self.pid.to_string())
            .output()
            .expect("couldn't kill server");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "must be manually run with the correct environment setup"]
    async fn test_session() {
        let path = "/tmp/osa-apple-music-test.sock";
        let mut session = Session::new(path).await.unwrap();
        assert!(session.application().await.is_ok());
        assert!(session.now_playing().await.is_ok());
        std::fs::remove_file(path).unwrap();
    }
}
