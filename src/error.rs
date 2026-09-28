//! Error type shared by the library and the HTTP layer.
//!
//! Every variant carries a message written for the person using the app, not
//! for a developer: it should say what happened and, where possible, what to
//! do next. The HTTP layer maps variants to status codes.

use std::io;

#[derive(Debug, thiserror::Error)]
pub enum CairnError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    BadRequest(String),
    /// The operating system refused access (filesystem permissions).
    #[error("{0}")]
    PermissionDenied(String),
    /// The published file changed since editing began.
    #[error("{0}")]
    Conflict(String),
    /// Another Cairn client holds the edit lock, or ours was released.
    #[error("{0}")]
    Locked(String),
    /// The workspace is open read-only (unsupported schema or no write access).
    #[error("{0}")]
    ReadOnly(String),
    /// A request-supplied path was rejected by the path-safety gate.
    #[error("{0}")]
    PathRejected(String),
    #[error("{0}")]
    InvalidImage(String),
    #[error("{0}")]
    Io(String),
    /// No browser that can make PDFs was found on this computer.
    #[error("{0}")]
    PdfUnavailable(String),
}

pub type Result<T> = std::result::Result<T, CairnError>;

impl From<io::Error> for CairnError {
    fn from(err: io::Error) -> Self {
        match err.kind() {
            io::ErrorKind::PermissionDenied => CairnError::PermissionDenied(
                "You don't have permission to change files in this folder. \
                 Ask whoever manages the shared folder for write access."
                    .into(),
            ),
            io::ErrorKind::NotFound => {
                CairnError::NotFound("The file or folder could not be found.".into())
            }
            _ => CairnError::Io(format!("A file operation failed: {err}")),
        }
    }
}

impl CairnError {
    /// Short machine-readable code used by the browser UI.
    pub fn code(&self) -> &'static str {
        match self {
            CairnError::NotFound(_) => "not_found",
            CairnError::BadRequest(_) => "bad_request",
            CairnError::PermissionDenied(_) => "permission_denied",
            CairnError::Conflict(_) => "conflict",
            CairnError::Locked(_) => "locked",
            CairnError::ReadOnly(_) => "read_only",
            CairnError::PathRejected(_) => "path_rejected",
            CairnError::InvalidImage(_) => "invalid_image",
            CairnError::Io(_) => "io_error",
            CairnError::PdfUnavailable(_) => "pdf_unavailable",
        }
    }
}
