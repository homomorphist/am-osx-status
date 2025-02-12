#![doc = include_str!("../README.md")]

pub(crate) mod balanced;
pub mod session;

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

/// Run the provided code in the specified language.
/// This does not establish a session. It spawns a new process for each call.
pub async fn run(code: &str, language: Language) -> tokio::io::Result<SingleEvaluationOutput> {
    use tokio::io::AsyncWriteExt;
    use std::process::Stdio;

    let mut child = tokio::process::Command::new("osascript")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args([
            "-l", language.to_str(),
            "-",
        ])
        .spawn()?;

    child.stdin.take().expect("cannot get stdin").write_all({
        code.as_bytes()
    }).await?;

    child.wait_with_output().await.map(|output| SingleEvaluationOutput { raw: output })
}

#[derive(Debug)]
pub struct SingleEvaluationOutput {
    pub raw: std::process::Output,
}
impl SingleEvaluationOutput {
    /// The output piped to the standard output stream.
    /// Notably, this does not include the final output of the script, which can be found in `stderr`.
    /// If something is logged to the console, it'll end up here.
    pub fn stdout(&self) -> std::borrow::Cow<str> {
        String::from_utf8_lossy(&self.raw.stdout)
    }

    /// The output piped to the standard error stream.
    /// This includes the final output of the script.
    pub fn stderr(&self) -> std::borrow::Cow<str> {
        String::from_utf8_lossy(&self.raw.stderr)
    }
}
