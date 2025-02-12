pub mod application;
pub mod track;

pub use application::ApplicationData;
pub use track::Track;


pub mod error {
    #[derive(Debug, thiserror::Error)]
    pub enum SessionEvaluationError {
        #[error("{0}")]
        DeserializationFailure(#[from] serde_json::Error),
        #[error("couldn't extract output")]
        ValueExtractionFailure { output: osascript::session::ReplOutput },
        #[error("{0}")]
        SessionFailure(#[from] osascript::session::SessionError),
        #[error("{0}")]
        SingleEvaluationFailure(#[from] tokio::io::Error),
    }

    #[derive(Debug, thiserror::Error)]
    pub enum SingleEvaluationError {
        #[error("{0}")]
        DeserializationFailure(#[from] serde_json::Error),
        #[error("couldn't extract output")]
        ValueExtractionFailure { output: osascript::session::ReplOutput },
        #[error("{0}")]
        IoError(#[from] tokio::io::Error),
    }
    
    
}


#[derive(Debug)]
pub struct Session {
    jxa: osascript::session::Session,
}
impl Session {
    pub async fn new() -> Result<Self, std::io::Error> {
        Ok(Self { jxa: osascript::session::Session::new(osascript::Language::JavaScript).await? })
    }

    async fn run_and_prepare_json(&mut self, script: &str) -> Result<String, error::SessionEvaluationError> {
        let output = self.jxa.run(script).await?;
        let output = match output.guess() {
            Ok(v) => v,
            Err(_) => return Err(error::SessionEvaluationError::ValueExtractionFailure { output }),
        };
        let output = &output[1..output.len() - 1]; // remove quotes
        Ok(unescape::unescape(output).expect("bad internal escape sequence"))
    }

    pub async fn application(&mut self) -> Result<ApplicationData, error::SessionEvaluationError> {
        let json = self.run_and_prepare_json("JSON.stringify(Application(\"Music\").properties())").await?;
        serde_json::from_str(&json).map_err(error::SessionEvaluationError::DeserializationFailure).map(ApplicationData::fix)
    }

    pub async fn now_playing(&mut self) -> Result<Option<crate::Track>, error::SessionEvaluationError> {
        match self.run_and_prepare_json("JSON.stringify(Application(\"Music\").currentTrack().properties())").await {
            Ok(json) => serde_json::from_str(&json).map_err(error::SessionEvaluationError::DeserializationFailure).map(Some),
            Err(error::SessionEvaluationError::ValueExtractionFailure { output }) if output.raw.get_inner() == b"!! Error: Error: Can't get object." => Ok(None),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "must be manually run with the correct environment setup"]
    async fn test_session() {
        let mut session = Session::new().await.unwrap();
        assert!(session.application().await.is_ok());
        assert!(session.now_playing().await.is_ok());
    }
}
