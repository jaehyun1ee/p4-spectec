//! Structural checks before TeX serialization
//!
//! `validate` rejects leading or consecutive grid and gathered gaps.
//! The traversal follows serialization order, including visible fill separators,
//! so the first malformed structure retains its specific rendering error.

use super::doc::{Block, Doc, GridRow, interspersed};
use crate::backend::latex::error::{Error, Result};

// == Documents

// - Document

/// Rejects row sequences that cannot be serialized as content and separators.
fn validate_doc(doc: &Doc) -> Result<()> {
    match doc {
        Doc::Concat(docs) | Doc::Stacked(docs) | Doc::LeftStack(docs) | Doc::Numbered(docs) => {
            validate_docs(docs)?;
        }
        Doc::Group(doc)
        | Doc::Mathbin(doc)
        | Doc::Mathrel(doc)
        | Doc::Displaystyle(doc)
        | Doc::Delimited(_, doc)
        | Doc::Link(_, doc)
        | Doc::LayoutGroup(doc)
        | Doc::Nest(_, doc) => validate_doc(doc)?,
        Doc::Subscript(doc_l, doc_r)
        | Doc::Superscript(doc_l, doc_r)
        | Doc::Fraction(doc_l, doc_r) => {
            validate_doc(doc_l)?;
            validate_doc(doc_r)?;
        }
        Doc::Subsup(doc_base, doc_sub, doc_sup) => {
            validate_doc(doc_base)?;
            validate_doc(doc_sub)?;
            validate_doc(doc_sup)?;
        }
        Doc::Fill(_, separator, docs) => validate_docs(interspersed(separator, docs))?,
        Doc::Aligned(rows) => validate_docs(rows.iter().flatten())?,
        Doc::Grid(_, rows) => validate_grid_rows(rows)?,
        Doc::Gathered(blocks) => validate_blocks(blocks)?,
        _ => {}
    }
    Ok(())
}

/// Validates documents in serialization order, stopping at the first error.
fn validate_docs<'a>(docs: impl IntoIterator<Item = &'a Doc>) -> Result<()> {
    for doc in docs {
        validate_doc(doc)?;
    }
    Ok(())
}

// - Grid document

/// Rejects leading or consecutive grid gaps and validates row content.
fn validate_grid_rows(rows: &[GridRow]) -> Result<()> {
    let mut gap_allowed = false;
    // A content row permits at most one subsequent gap
    for row in rows {
        match row {
            // Validate cells before accepting a separator after their row
            GridRow::Cells(docs) => {
                validate_docs(docs)?;
                gap_allowed = true;
            }
            // Spanning content follows the same gap rule as ordinary cells
            GridRow::Spanning(doc) => {
                validate_doc(doc)?;
                gap_allowed = true;
            }
            // Leading and consecutive gaps have no preceding content row
            GridRow::Gap => {
                if !gap_allowed {
                    return Err(Error::MalformedGridGap);
                }
                gap_allowed = false;
            }
        }
    }
    Ok(())
}

// - Gathered document

/// Rejects leading or consecutive gathered gaps and validates each line.
fn validate_blocks(blocks: &[Block]) -> Result<()> {
    let mut gap_allowed = false;
    // A line permits at most one subsequent gap
    for block in blocks {
        match block {
            // Validate the line before accepting its following separator
            Block::Line(doc) => {
                validate_doc(doc)?;
                gap_allowed = true;
            }
            // Leading and consecutive gaps have no preceding line
            Block::Gap => {
                if !gap_allowed {
                    return Err(Error::MalformedGathered);
                }
                gap_allowed = false;
            }
        }
    }
    Ok(())
}

// == Entry point

/// Rejects malformed row gaps before the document reaches the shared printer.
pub(super) fn validate(doc: &Doc) -> Result<()> {
    validate_doc(doc)
}
