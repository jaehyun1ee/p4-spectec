//! Invoke the configured C preprocessor for P4 file inputs.
//!
//! Include directories and the source path are passed to `cc -E`; stdout
//! becomes lexer input and a nonzero status becomes a located P4 error. A file
//! containing `#include <core.p4>` is therefore expanded before tokenization.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::lang::common::source::{Position, Span};

use super::error::{P4Error, P4ErrorKind};

pub fn preprocess(includes: &[PathBuf], path: impl AsRef<Path>) -> Result<String, P4Error> {
    let path = path.as_ref();
    let mut command = Command::new("cc");
    for include in includes {
        command.arg(format!("-I{}", include.display()));
    }
    command.args(["-undef", "-nostdinc", "-E", "-x", "c"]);
    command.arg(path);
    let output = command.output();
    let output = output.map_err(|error| {
        let kind = P4ErrorKind::Preprocessor {
            status: None,
            stderr: error.to_string(),
        };
        P4Error::new(kind, file_span(path))
    })?;
    if !output.status.success() {
        let kind = P4ErrorKind::Preprocessor {
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        };
        return Err(P4Error::new(kind, file_span(path)));
    }
    let source = String::from_utf8(output.stdout);
    source.map_err(|error| {
        let kind = P4ErrorKind::Preprocessor {
            status: output.status.code(),
            stderr: error.to_string(),
        };
        P4Error::new(kind, file_span(path))
    })
}

fn file_span(path: &Path) -> Span {
    let position = Position::new(path.to_string_lossy().into_owned(), 0, 0);
    Span::new(position.clone(), position)
}
