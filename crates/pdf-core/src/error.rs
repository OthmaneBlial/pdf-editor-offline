use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("split publication stopped after {completed_count} outputs in {directory}: {source}", directory=.directory.display())]
    PartialSplit {
        directory: PathBuf,
        completed: Vec<PathBuf>,
        completed_count: usize,
        failed_output: Option<PathBuf>,
        #[source]
        source: Box<Error>,
    },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("PDF error: {0}")]
    Pdf(#[from] lopdf::Error),
    #[error("resource limit exceeded: {0}")]
    Limit(&'static str),
    #[error("invalid document: {0}")]
    Invalid(&'static str),
    #[error("invalid page selection: {0}")]
    Pages(&'static str),
    #[error("operation refused: {0}")]
    Refused(&'static str),
    #[error("output already exists: {0}")]
    OutputExists(PathBuf),
    #[error("saved output failed verification: {0}")]
    Verification(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Authentication failed; the password itself is never part of this error.
    pub fn is_invalid_password(&self) -> bool {
        matches!(self, Self::Pdf(lopdf::Error::InvalidPassword))
    }
}
