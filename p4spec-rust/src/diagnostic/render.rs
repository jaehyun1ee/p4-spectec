//! Source resolution and codespan output at presentation boundaries
//!
//! A renderer caches successful and unavailable source loads for its lifetime.
//! Explicit text overrides replace cached disk contents.
//! Reports keep their original spans even when source text is unavailable.
//! Rendering prepares output before writing stderr so invalid spans cannot
//! leave a partially printed diagnostic.

use std::{collections::HashMap, fs, io::Write};

use codespan_reporting::{
    diagnostic::{Diagnostic as CodeDiagnostic, Label as CodeLabel},
    files::{self, Files, SimpleFiles},
    term::{
        self,
        termcolor::{Buffer, BufferWriter},
    },
};

use crate::lang::common::source::{Position, Span};

use super::{
    ColorChoice, Diagnostic, Label, LabelStyle, Report, ReportKind, Severity, SnippetConfig,
};

// = Helpers

/// Formats even invalid byte columns without overflowing their display offset.
fn span_location(span: &Span) -> String {
    let loc = |pos: &Position| {
        format!("{}:{}:{}", pos.file.escape_debug(), pos.line, pos.column as u128 + 1)
    };
    if span.left == span.right {
        loc(&span.left)
    } else {
        format!("{}-{}", loc(&span.left), loc(&span.right))
    }
}

// = Configuration

/// Configures snippet presentation and the visible trace budget.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Delegates source layout policy to codespan.
    pub snippet: SnippetConfig,
    /// Selects color behavior for stderr output.
    pub color: ColorChoice,
    /// Limits visible trace nodes without modifying the report.
    pub trace_limit: usize,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self { snippet: SnippetConfig::default(), color: ColorChoice::Auto, trace_limit: 64 }
    }
}

// = Rendering errors

/// Distinguishes inconsistent source coordinates from output failures.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// Rejects a malformed span rather than changing its coordinates.
    #[error("invalid diagnostic span {}: {reason}", span_location(.span))]
    InvalidSpan {
        /// Preserves the rejected coordinates.
        span: Box<Span>,
        /// Explains the violated coordinate contract.
        reason: &'static str,
    },
    /// Preserves an error from codespan or the output writer.
    #[error(transparent)]
    Output(#[from] files::Error),
}

// = Source cache and renderer

/// Records whether cached source text can be printed safely as a snippet.
#[derive(Clone, Copy)]
struct Source {
    id: usize,
    printable: bool,
}

/// Resolves source text and renders reports without changing semantic state.
pub struct Renderer {
    config: RenderConfig,
    files: SimpleFiles<String, String>,
    cache: HashMap<String, Option<Source>>,
}

impl Renderer {
    // - invalid_*: span errors

    fn invalid_span(span: &Span, reason: &'static str) -> RenderError {
        RenderError::InvalidSpan { span: Box::new(span.clone()), reason }
    }

    // - register_*: source storage

    /// Registers source text and marks whether its snippet is safe to print.
    fn register_source(&mut self, file: String, text: String) -> Source {
        // Codespan may print context lines outside the labelled range
        let mut chars = text.chars().peekable();
        let mut printable = true;
        while let Some(ch) = chars.next() {
            if ch.is_control()
                && ch != '\n'
                && ch != '\t'
                && !(ch == '\r' && chars.peek() == Some(&'\n'))
            {
                printable = false;
                break;
            }
        }
        // Keep the source untouched so span validation uses original byte offsets
        Source { id: self.files.add(file.escape_debug().to_string(), text), printable }
    }

    // - resolve_*: sources and offsets

    /// Caches both successful and unavailable reads under the original file name.
    fn resolve_source(&mut self, file: &str) -> Option<Source> {
        // An explicit override or previous load always wins
        if let Some(source) = self.cache.get(file) {
            return *source;
        }

        // Unreadable and non-UTF-8 files retain location-only diagnostics
        let source = fs::read_to_string(file)
            .ok()
            .map(|text| self.register_source(file.to_owned(), text));
        self.cache.insert(file.to_owned(), source);
        source
    }

    /// Resolves a one-based line and byte column without clamping either.
    fn resolve_offset(&self, id: usize, pos: &Position, span: &Span) -> Result<usize, RenderError> {
        let text = self.files.source(id)?;
        // SimpleFiles permits an extra sentinel line; source spans do not
        let line_max = self.files.line_index(id, text.len())? + 1;
        if pos.line == 0 || pos.line > line_max {
            return Err(Self::invalid_span(span, "line is outside the source"));
        }

        // A newline starts the next line; CR remains an original source byte
        let range = self.files.line_range(id, pos.line - 1)?;
        let line = text[range.clone()]
            .strip_suffix('\n')
            .unwrap_or(&text[range.clone()]);
        if pos.column > line.len() {
            return Err(Self::invalid_span(span, "byte column is outside the line"));
        }

        // Coordinates must lie between complete UTF-8 characters
        let offset = range.start + pos.column;
        if !text.is_char_boundary(offset) {
            return Err(Self::invalid_span(span, "byte column splits a UTF-8 character"));
        }
        Ok(offset)
    }

    // - append_*: labels and location notes

    /// Retains a label's role and location when a snippet cannot be produced.
    fn append_location_note(
        label: &Label,
        loc: &str,
        reason: Option<&str>,
        diagnostic: &mut CodeDiagnostic<usize>,
    ) {
        let role = match label.style {
            LabelStyle::Primary => "at",
            LabelStyle::Secondary => "related location at",
        };
        let message =
            if label.message.is_empty() { String::new() } else { format!(": {}", label.message) };
        let reason = reason.map_or_else(String::new, |reason| format!(" ({reason})"));
        diagnostic
            .notes
            .push(format!("{role} {loc}{message}{reason}"));
    }

    /// Converts a label or preserves its location as an explicit fallback note.
    fn append_label(
        &mut self,
        label: &Label,
        diagnostic: &mut CodeDiagnostic<usize>,
    ) -> Result<(), RenderError> {
        let span = &label.span;
        // One codespan label cannot describe two source identities
        if span.left.file != span.right.file {
            return Err(Self::invalid_span(span, "endpoints name different files"));
        }

        // Generated and file-only spans have no line to underline
        let file_only = span.left.line == 0 && span.left.column == 0 && span.left == span.right;
        if file_only {
            let loc = if span.left.file.is_empty() {
                "generated source".to_owned()
            } else {
                span.left.file.escape_debug().to_string()
            };
            Self::append_location_note(label, &loc, None, diagnostic);
            return Ok(());
        }

        // Missing source must not hide a responsible location behind related labels
        let Some(source) = self.resolve_source(&span.left.file) else {
            Self::append_location_note(
                label,
                &span_location(span),
                Some("source unavailable"),
                diagnostic,
            );
            return Ok(());
        };
        let start = self.resolve_offset(source.id, &span.left, span)?;
        let end = self.resolve_offset(source.id, &span.right, span)?;
        if start > end {
            return Err(Self::invalid_span(span, "end precedes start"));
        }

        // Preserve coordinates without sending invisible controls to the terminal
        if !source.printable {
            Self::append_location_note(
                label,
                &span_location(span),
                Some("snippet omitted: source contains control characters"),
                diagnostic,
            );
            return Ok(());
        }

        // Preserve the producer's role, range, and explanation
        diagnostic.labels.push(CodeLabel {
            style: label.style,
            file_id: source.id,
            range: start..end,
            message: label.message.clone(),
        });
        Ok(())
    }

    // - convert_*: codespan diagnostics

    /// Converts diagnostic data without inspecting the report tree.
    fn convert_diagnostic(
        &mut self,
        diagnostic: &Diagnostic,
    ) -> Result<CodeDiagnostic<usize>, RenderError> {
        let mut rendered = CodeDiagnostic::new(diagnostic.severity);
        rendered.code.clone_from(&diagnostic.code);
        rendered.message.clone_from(&diagnostic.message);
        rendered.notes.clone_from(&diagnostic.notes);
        // Uncoded failures still identify their author
        if diagnostic.code.is_none() && !diagnostic.source.is_empty() {
            rendered
                .notes
                .push(format!("source: {}", diagnostic.source));
        }
        // Convert each label independently to retain cross-file relationships
        for label in &diagnostic.labels {
            self.append_label(label, &mut rendered)?;
        }
        Ok(rendered)
    }

    /// Gives root and child nodes the same source-aware presentation.
    fn convert_report_kind(
        &mut self,
        kind: &ReportKind,
    ) -> Result<CodeDiagnostic<usize>, RenderError> {
        match kind {
            // Render context as a note with its own source location
            ReportKind::Frame { span, message } => {
                let mut rendered = CodeDiagnostic::new(Severity::Note).with_message(message);
                if *span != Span::default() {
                    self.append_label(
                        &Label {
                            style: LabelStyle::Secondary,
                            span: span.clone(),
                            message: String::new(),
                        },
                        &mut rendered,
                    )?;
                }
                Ok(rendered)
            }
            // Keep each cause's code, severity, labels, and notes
            ReportKind::Cause(diagnostic) => self.convert_diagnostic(diagnostic),
        }
    }

    // - Construction and source overrides

    /// Constructs a renderer with an empty source cache.
    pub fn new(config: RenderConfig) -> Self {
        Self { config, files: SimpleFiles::new(), cache: HashMap::new() }
    }

    /// Supplies source text that takes precedence over disk contents.
    pub fn insert_source(&mut self, file: impl Into<String>, text: impl Into<String>) {
        let file = file.into();
        let source = self.register_source(file.clone(), text.into());
        self.cache.insert(file, Some(source));
    }

    // - render_to_*: output destinations

    /// Emits the root and traverses visible causes in depth-first branch order.
    fn render_to_buffer(
        &mut self,
        buffer: &mut Buffer,
        report: &Report,
    ) -> Result<(), RenderError> {
        let diagnostic = self.convert_report_kind(&report.kind)?;
        term::emit_to_write_style(buffer, &self.config.snippet, &self.files, &diagnostic)?;

        // Store traversal cursors instead of recursing or cloning reports
        let mut pending = vec![(report.children.iter(), 0usize)];
        let mut count = 0;
        while let Some((children, depth)) = pending.last_mut() {
            let Some(child) = children.next() else {
                pending.pop();
                continue;
            };
            let depth = *depth;
            // Truncation affects output only, leaving every stored cause intact
            if count == self.config.trace_limit {
                writeln!(buffer, "trace truncated after {count} nodes")
                    .map_err(files::Error::from)?;
                break;
            }
            count += 1;
            writeln!(buffer, "trace[{depth}]:").map_err(files::Error::from)?;

            let diagnostic = self.convert_report_kind(&child.kind)?;
            term::emit_to_write_style(buffer, &self.config.snippet, &self.files, &diagnostic)?;
            pending.push((child.children.iter(), depth + 1));
        }
        Ok(())
    }

    /// Renders a report to a string without terminal color codes.
    pub fn render_to_string(&mut self, report: &Report) -> Result<String, RenderError> {
        let mut buffer = Buffer::no_color();
        self.render_to_buffer(&mut buffer, report)?;
        // Codespan and trace headings write only UTF-8 text
        Ok(String::from_utf8(buffer.into_inner()).expect("diagnostic output is UTF-8"))
    }

    /// Renders a complete report to stderr using the configured colors.
    pub fn render_to_stderr(&mut self, report: &Report) -> Result<(), RenderError> {
        let writer = BufferWriter::stderr(self.config.color);
        let mut buffer = writer.buffer();
        self.render_to_buffer(&mut buffer, report)?;
        writer.print(&buffer).map_err(files::Error::from)?;
        Ok(())
    }
}
