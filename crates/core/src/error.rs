use std::io;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not a git repository (or any parent up to mount point)")]
    NotARepo,

    #[error("git command failed: {0}")]
    GitCommand(String),

    #[error("failed to parse git output: {0}")]
    ParseError(String),

    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
