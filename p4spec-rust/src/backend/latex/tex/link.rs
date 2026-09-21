//! Link ownership across document regions and generated layout
//!
//! `link_unowned_doc` assigns fallback targets without replacing explicit links;
//! `normalize_after_layout` distributes links over concrete lines and cells.
//! `strip_links` removes navigation from invisible geometry copies.

use super::doc::{self, *};
use crate::backend::latex::error::{Error, Result};

// == Targets

/// Validates a nonempty local target containing ASCII names and primes.
pub(crate) fn target_of_string(text: &str) -> Result<Target> {
    if !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'\''))
    {
        Ok(Target(text.into()))
    } else {
        Err(Error::InvalidLinkTarget(text.into()))
    }
}

// == Ownership analysis

/// Detects existing links in a document.
fn has_link_doc(doc: &Doc) -> bool {
    match doc {
        Doc::Link(_, _) => true,
        Doc::Group(doc)
        | Doc::Mathbin(doc)
        | Doc::Mathrel(doc)
        | Doc::Displaystyle(doc)
        | Doc::Delimited(_, doc)
        | Doc::LayoutGroup(doc)
        | Doc::Nest(_, doc) => has_link_doc(doc),
        Doc::Subscript(doc_base, doc_sub)
        | Doc::Superscript(doc_base, doc_sub)
        | Doc::Fraction(doc_base, doc_sub) => has_link_doc(doc_base) || has_link_doc(doc_sub),
        Doc::Subsup(doc_base, doc_sub, doc_sup) => {
            has_link_doc(doc_base) || has_link_doc(doc_sub) || has_link_doc(doc_sup)
        }
        Doc::Concat(docs) | Doc::Stacked(docs) | Doc::LeftStack(docs) => {
            docs.iter().any(has_link_doc)
        }
        Doc::Aligned(rows) => rows.iter().any(|docs| docs.iter().any(has_link_doc)),
        Doc::Grid(_, rows) => rows.iter().any(|row| match row {
            Row::Cells(docs) => docs.iter().any(has_link_doc),
            Row::Spanning(doc) => has_link_doc(doc),
            Row::Gap => false,
        }),
        Doc::Gathered(blocks) => blocks.iter().any(|block| match block {
            Block::Line(doc) => has_link_doc(doc),
            Block::Gap => false,
        }),
        Doc::Fill(_, separator, docs) => has_link_doc(separator) || docs.iter().any(has_link_doc),
        Doc::Numbered(docs) => docs.iter().any(has_link_doc),
        _ => false,
    }
}

/// Detects regions that cannot share one enclosing fallback link.
fn has_boundary_doc(doc: &Doc) -> bool {
    match doc {
        Doc::Link(_, _) => true,
        Doc::Group(doc)
        | Doc::Mathbin(doc)
        | Doc::Mathrel(doc)
        | Doc::Displaystyle(doc)
        | Doc::Delimited(_, doc)
        | Doc::LayoutGroup(doc)
        | Doc::Nest(_, doc) => has_boundary_doc(doc),
        Doc::Subscript(doc_base, doc_sub)
        | Doc::Superscript(doc_base, doc_sub)
        | Doc::Fraction(doc_base, doc_sub) => {
            has_boundary_doc(doc_base) || has_boundary_doc(doc_sub)
        }
        Doc::Subsup(doc_base, doc_sub, doc_sup) => {
            has_boundary_doc(doc_base) || has_boundary_doc(doc_sub) || has_boundary_doc(doc_sup)
        }
        Doc::Concat(docs) | Doc::Stacked(docs) | Doc::LeftStack(docs) => {
            docs.iter().any(has_boundary_doc)
        }
        Doc::Aligned(rows) => rows.iter().any(|docs| docs.iter().any(has_boundary_doc)),
        Doc::Grid(_, rows) => rows.iter().any(|row| match row {
            Row::Cells(docs) => docs.iter().any(has_boundary_doc),
            Row::Spanning(doc) => has_boundary_doc(doc),
            Row::Gap => false,
        }),
        Doc::Gathered(blocks) => blocks.iter().any(|block| match block {
            Block::Line(doc) => has_boundary_doc(doc),
            Block::Gap => false,
        }),
        Doc::Fill(_, _, _) | Doc::Numbered(_) => true,
        _ => false,
    }
}

// == Fallback insertion

/// Links unowned regions while retaining explicit targets and fill separators.
pub(crate) fn link_unowned_doc(target: &Target, doc: Doc) -> Doc {
    // Give a boundary-free region one enclosing link
    if !has_boundary_doc(&doc) {
        return doc::link(target.clone(), doc);
    }
    match doc {
        Doc::Concat(docs) => link_unowned_concat(target, docs),
        Doc::Link(target_existing, doc_linked) => {
            // Flatten nested ownership only when an inner explicit link exists
            if has_link_doc(&doc_linked) {
                link_unowned_doc(&target_existing, *doc_linked)
            } else {
                Doc::Link(target_existing, doc_linked)
            }
        }
        Doc::Group(doc) => Doc::Group(Box::new(link_unowned_doc(target, *doc))),
        Doc::Mathbin(doc) => Doc::Mathbin(Box::new(link_unowned_doc(target, *doc))),
        Doc::Mathrel(doc) => Doc::Mathrel(Box::new(link_unowned_doc(target, *doc))),
        Doc::Displaystyle(doc) => Doc::Displaystyle(Box::new(link_unowned_doc(target, *doc))),
        Doc::LayoutGroup(doc) => Doc::LayoutGroup(Box::new(link_unowned_doc(target, *doc))),
        Doc::Delimited(delimiter, doc) => {
            Doc::Delimited(delimiter, Box::new(link_unowned_doc(target, *doc)))
        }
        Doc::Nest(indent, doc) => Doc::Nest(indent, Box::new(link_unowned_doc(target, *doc))),
        Doc::Subscript(doc_l, doc_r) => Doc::Subscript(
            Box::new(link_unowned_doc(target, *doc_l)),
            Box::new(link_unowned_doc(target, *doc_r)),
        ),
        Doc::Superscript(doc_l, doc_r) => Doc::Superscript(
            Box::new(link_unowned_doc(target, *doc_l)),
            Box::new(link_unowned_doc(target, *doc_r)),
        ),
        Doc::Fraction(doc_l, doc_r) => Doc::Fraction(
            Box::new(link_unowned_doc(target, *doc_l)),
            Box::new(link_unowned_doc(target, *doc_r)),
        ),
        Doc::Subsup(doc_base, doc_sub, doc_sup) => Doc::Subsup(
            Box::new(link_unowned_doc(target, *doc_base)),
            Box::new(link_unowned_doc(target, *doc_sub)),
            Box::new(link_unowned_doc(target, *doc_sup)),
        ),
        Doc::Fill(indent, separator, docs) => Doc::Fill(
            indent,
            separator,
            docs.into_iter()
                .map(|doc| link_unowned_doc(target, doc))
                .collect(),
        ),
        Doc::Aligned(rows) => Doc::Aligned(
            rows.into_iter()
                .map(|docs| {
                    docs.into_iter()
                        .map(|doc| link_unowned_doc(target, doc))
                        .collect()
                })
                .collect(),
        ),
        Doc::Grid(alignments, rows) => Doc::Grid(
            alignments,
            rows.into_iter()
                .map(|row| link_unowned_row(target, row))
                .collect(),
        ),
        Doc::Gathered(blocks) => Doc::Gathered(
            blocks
                .into_iter()
                .map(|block| match block {
                    Block::Line(doc) => Block::Line(link_unowned_doc(target, doc)),
                    Block::Gap => Block::Gap,
                })
                .collect(),
        ),
        Doc::Stacked(docs) => Doc::Stacked(
            docs.into_iter()
                .map(|doc| link_unowned_doc(target, doc))
                .collect(),
        ),
        Doc::LeftStack(docs) => Doc::LeftStack(
            docs.into_iter()
                .map(|doc| link_unowned_doc(target, doc))
                .collect(),
        ),
        Doc::Numbered(docs) => Doc::Numbered(
            docs.into_iter()
                .map(|doc| link_unowned_doc(target, doc))
                .collect(),
        ),
        _ => doc::link(target.clone(), doc),
    }
}

/// Coalesces adjacent boundary-free children into one fallback region.
fn link_unowned_concat(target: &Target, docs: Vec<Doc>) -> Doc {
    let mut docs_unowned = Vec::new();
    let mut docs_linked = Vec::new();
    // Flush pending content before each explicitly owned boundary
    for doc in docs {
        if has_boundary_doc(&doc) {
            if !docs_unowned.is_empty() {
                let doc = concat(std::mem::take(&mut docs_unowned));
                docs_linked.push(doc::link(target.clone(), doc));
            }
            docs_linked.push(link_unowned_doc(target, doc));
        } else {
            docs_unowned.push(doc);
        }
    }
    // Preserve the final unowned suffix
    if !docs_unowned.is_empty() {
        let doc = concat(docs_unowned);
        docs_linked.push(doc::link(target.clone(), doc));
    }
    concat(docs_linked)
}

/// Assigns fallback ownership independently to each visible grid cell.
fn link_unowned_row(target: &Target, row: Row) -> Row {
    match row {
        Row::Cells(docs) => Row::Cells(
            docs.into_iter()
                .map(|doc| link_unowned_doc(target, doc))
                .collect(),
        ),
        Row::Spanning(doc) => Row::Spanning(link_unowned_doc(target, doc)),
        Row::Gap => Row::Gap,
    }
}

// == Layout normalization

/// Distributes an enclosing target across concrete lines and grid cells.
pub(super) fn normalize_after_layout(target: &Target, doc: Doc) -> Doc {
    match doc {
        Doc::LeftStack(docs) => Doc::LeftStack(
            docs.into_iter()
                .map(|doc| link_unowned_doc(target, doc))
                .collect(),
        ),
        Doc::Grid(alignments, rows) => Doc::Grid(
            alignments,
            rows.into_iter()
                .map(|row| link_unowned_row(target, row))
                .collect(),
        ),
        doc => link_unowned_doc(target, doc),
    }
}

// == Invisible geometry

/// Removes every hyperlink while preserving all layout structure.
pub(super) fn strip_links(doc: &Doc) -> Doc {
    match doc {
        Doc::Link(_, doc) => strip_links(doc),
        Doc::Group(doc) => Doc::Group(Box::new(strip_links(doc))),
        Doc::Mathbin(doc) => Doc::Mathbin(Box::new(strip_links(doc))),
        Doc::Mathrel(doc) => Doc::Mathrel(Box::new(strip_links(doc))),
        Doc::Displaystyle(doc) => Doc::Displaystyle(Box::new(strip_links(doc))),
        Doc::LayoutGroup(doc) => Doc::LayoutGroup(Box::new(strip_links(doc))),
        Doc::Delimited(delimiter, doc) => Doc::Delimited(*delimiter, Box::new(strip_links(doc))),
        Doc::Nest(indent, doc) => Doc::Nest(*indent, Box::new(strip_links(doc))),
        Doc::Subscript(doc_l, doc_r) => {
            Doc::Subscript(Box::new(strip_links(doc_l)), Box::new(strip_links(doc_r)))
        }
        Doc::Superscript(doc_l, doc_r) => {
            Doc::Superscript(Box::new(strip_links(doc_l)), Box::new(strip_links(doc_r)))
        }
        Doc::Fraction(doc_l, doc_r) => {
            Doc::Fraction(Box::new(strip_links(doc_l)), Box::new(strip_links(doc_r)))
        }
        Doc::Subsup(doc_base, doc_sub, doc_sup) => Doc::Subsup(
            Box::new(strip_links(doc_base)),
            Box::new(strip_links(doc_sub)),
            Box::new(strip_links(doc_sup)),
        ),
        Doc::Fill(indent, separator, docs) => Doc::Fill(
            *indent,
            Box::new(strip_links(separator)),
            docs.iter().map(strip_links).collect(),
        ),
        Doc::Aligned(rows) => Doc::Aligned(
            rows.iter()
                .map(|docs| docs.iter().map(strip_links).collect())
                .collect(),
        ),
        Doc::Grid(alignments, rows) => Doc::Grid(
            alignments.clone(),
            rows.iter()
                .map(|row| match row {
                    Row::Cells(docs) => Row::Cells(docs.iter().map(strip_links).collect()),
                    Row::Spanning(doc) => Row::Spanning(strip_links(doc)),
                    Row::Gap => Row::Gap,
                })
                .collect(),
        ),
        Doc::Gathered(blocks) => Doc::Gathered(
            blocks
                .iter()
                .map(|block| match block {
                    Block::Line(doc) => Block::Line(strip_links(doc)),
                    Block::Gap => Block::Gap,
                })
                .collect(),
        ),
        Doc::Concat(docs) => Doc::Concat(docs.iter().map(strip_links).collect()),
        Doc::Stacked(docs) => Doc::Stacked(docs.iter().map(strip_links).collect()),
        Doc::LeftStack(docs) => Doc::LeftStack(docs.iter().map(strip_links).collect()),
        Doc::Numbered(docs) => Doc::Numbered(docs.iter().map(strip_links).collect()),
        doc => doc.clone(),
    }
}
