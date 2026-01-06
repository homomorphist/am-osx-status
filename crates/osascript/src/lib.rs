#![doc = include_str!("../README.md")]

pub(crate) mod balanced;
pub mod repl;

/// A language that can be run in the `osascript` CLI.
/// Defaults to [`Self::AppleScript`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Default)]
pub enum Language {
    JavaScript,
    /// [AppleScript]; a natural language programming language for macOS.
    /// 
    /// [AppleScript]: https://en.wikipedia.org/wiki/AppleScript
    #[default]
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

/// Run the provided code in the specified language.
/// This does not establish a session. It spawns a new process for each call.
pub async fn run<I, S>(code: &str, language: Language, args: I) -> tokio::io::Result<SingleEvaluationOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr> 
{
    spawn(code, language, args).await?.wait().await
}


/// Spawns an `osascript` process with the given code and language.
/// Returns a handle to the process.
pub async fn spawn<I, S>(code: &str, language: Language, args: I) -> tokio::io::Result<ProcessHandle>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr> 
{
    use tokio::io::AsyncWriteExt;
    use std::process::Stdio;

    let mut child = tokio::process::Command::new("/usr/bin/osascript")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(["-l", language.to_str(), "-"])
        .args(args)
        .spawn()?;

    let mut stdin = child.stdin.take().expect("cannot get stdin");
    stdin.write_all(code.as_bytes()).await?;
    child.stdin.replace(stdin);

    Ok(ProcessHandle {
        internal: child
    })
}

/// A handle to a running `osascript` process.
/// Dropping the handle will not kill the process.
#[derive(Debug)]
pub struct ProcessHandle {
    pub internal: tokio::process::Child,
}
impl ProcessHandle {
    pub async fn wait(self) -> tokio::io::Result<SingleEvaluationOutput> {
        self.internal.wait_with_output().await.map(|output| SingleEvaluationOutput { raw: output })
    }
}
/// The result of a single evaluation of a script.
#[derive(Debug)]
pub struct SingleEvaluationOutput {
    pub raw: std::process::Output,
}
impl SingleEvaluationOutput {
    /// The output piped to the standard output stream.
    /// Notably, this does not include the final output of the script, which can be found in `stderr`.
    /// If something is logged to the console, it'll end up here.
    pub fn stdout(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.raw.stdout)
    }

    /// The output piped to the standard error stream.
    /// This includes the final output of the script.
    pub fn stderr(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.raw.stderr)
    }
}
