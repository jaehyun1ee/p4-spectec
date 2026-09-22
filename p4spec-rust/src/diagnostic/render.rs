//! Source resolution and codespan output at presentation boundaries
//!
//! A renderer caches successful and unavailable source loads for its lifetime.
//! Explicit text overrides replace cached disk contents.
//! Reports keep their original spans even when source text is unavailable.
//! Rendering prepares output before writing stderr so invalid spans cannot
//! leave a partially printed diagnostic.

use std::{collections::HashMap, fs, io::Write};

use codespan_reporting::{
    diagnostic::{Diagnostic, Label as CodeLabel},
    files::{self, Files, SimpleFiles},
    term::{
        self,
        termcolor::{Buffer, BufferWriter},
    },
};

use crate::lang::common::source::{Position, Span};

use super::{ColorChoice, Label, LabelStyle, Report, Severity, SnippetConfig, Trace};

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
    /// Constructs a renderer with an empty source cache.
    pub fn new(config: RenderConfig) -> Self {
        Self { config, files: SimpleFiles::new(), cache: HashMap::new() }
    }

    /// Supplies source text that takes precedence over disk contents.
    pub fn insert_source(&mut self, file: impl Into<String>, text: impl Into<String>) {
        let file = file.into();
        let source = self.add_source(file.clone(), text.into());
        self.cache.insert(file, Some(source));
    }

    /// Renders a report without terminal color codes.
    pub fn render_plain(&mut self, report: &Report) -> Result<String, RenderError> {
        let mut buffer = Buffer::no_color();
        self.render(&mut buffer, report)?;
        // Codespan and trace headings write only UTF-8 text
        Ok(String::from_utf8(buffer.into_inner()).expect("diagnostic output is UTF-8"))
    }

    /// Writes a complete diagnostic to stderr using the configured colors.
    pub fn emit_stderr(&mut self, report: &Report) -> Result<(), RenderError> {
        let writer = BufferWriter::stderr(self.config.color);
        let mut buffer = writer.buffer();
        self.render(&mut buffer, report)?;
        writer.print(&buffer).map_err(files::Error::from)?;
        Ok(())
    }

    /// Caches original bytes and suppresses snippets containing terminal controls.
    fn add_source(&mut self, file: String, text: String) -> Source {
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

    /// Caches both successful and unavailable reads under the original file name.
    fn resolve(&mut self, file: &str) -> Option<Source> {
        // An explicit override or previous load always wins
        if let Some(source) = self.cache.get(file) {
            return *source;
        }

        // Unreadable and non-UTF-8 files retain location-only diagnostics
        let source = fs::read_to_string(file)
            .ok()
            .map(|text| self.add_source(file.to_owned(), text));
        self.cache.insert(file.to_owned(), source);
        source
    }

    fn invalid(span: &Span, reason: &'static str) -> RenderError {
        RenderError::InvalidSpan { span: Box::new(span.clone()), reason }
    }

    /// Resolves a one-based line and byte column without clamping either.
    fn offset(&self, id: usize, pos: &Position, span: &Span) -> Result<usize, RenderError> {
        let text = self.files.source(id)?;
        // SimpleFiles permits an extra sentinel line; source spans do not
        let line_max = self.files.line_index(id, text.len())? + 1;
        if pos.line == 0 || pos.line > line_max {
            return Err(Self::invalid(span, "line is outside the source"));
        }

        // A newline starts the next line; CR remains an original source byte
        let range = self.files.line_range(id, pos.line - 1)?;
        let line = text[range.clone()]
            .strip_suffix('\n')
            .unwrap_or(&text[range.clone()]);
        if pos.column > line.len() {
            return Err(Self::invalid(span, "byte column is outside the line"));
        }

        // Coordinates must lie between complete UTF-8 characters
        let offset = range.start + pos.column;
        if !text.is_char_boundary(offset) {
            return Err(Self::invalid(span, "byte column splits a UTF-8 character"));
        }
        Ok(offset)
    }

    /// Converts a label or preserves its location as an explicit fallback note.
    fn label(
        &mut self,
        label: &Label,
        diagnostic: &mut Diagnostic<usize>,
    ) -> Result<(), RenderError> {
        let span = &label.span;
        // One codespan label cannot describe two source identities
        if span.left.file != span.right.file {
            return Err(Self::invalid(span, "endpoints name different files"));
        }

        // Generated and file-only spans have no line to underline
        let file_only = span.left.line == 0 && span.left.column == 0 && span.left == span.right;
        if file_only {
            let loc = if span.left.file.is_empty() {
                "generated source".to_owned()
            } else {
                span.left.file.escape_debug().to_string()
            };
            Self::fallback(label, &loc, None, diagnostic);
            return Ok(());
        }

        // Missing source must not hide a responsible location behind related labels
        let Some(source) = self.resolve(&span.left.file) else {
            Self::fallback(label, &span_location(span), Some("source unavailable"), diagnostic);
            return Ok(());
        };
        let start = self.offset(source.id, &span.left, span)?;
        let end = self.offset(source.id, &span.right, span)?;
        if start > end {
            return Err(Self::invalid(span, "end precedes start"));
        }

        // Preserve coordinates without sending invisible controls to the terminal
        if !source.printable {
            Self::fallback(
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

    /// Retains a label's role and location when a snippet cannot be produced.
    fn fallback(
        label: &Label,
        loc: &str,
        reason: Option<&str>,
        diagnostic: &mut Diagnostic<usize>,
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

    /// Constructs a temporary codespan view without copying recursive causes.
    fn diagnostic(&mut self, report: &Report) -> Result<Diagnostic<usize>, RenderError> {
        let mut diagnostic = Diagnostic::new(report.severity);
        diagnostic.code.clone_from(&report.code);
        diagnostic.message.clone_from(&report.message);
        diagnostic.notes.clone_from(&report.notes);
        // Uncoded failures still identify their author
        if report.code.is_none() && !report.source.is_empty() {
            diagnostic.notes.push(format!("source: {}", report.source));
        }
        // Convert each label independently to retain cross-file relationships
        for label in &report.labels {
            self.label(label, &mut diagnostic)?;
        }
        Ok(diagnostic)
    }

    /// Emits the root and traverses visible causes in depth-first branch order.
    fn render(&mut self, buffer: &mut Buffer, report: &Report) -> Result<(), RenderError> {
        let diagnostic = self.diagnostic(report)?;
        term::emit_to_write_style(buffer, &self.config.snippet, &self.files, &diagnostic)?;

        // Store traversal cursors instead of recursing or cloning reports
        let mut pending = vec![(report.traces.iter(), 0usize)];
        let mut count = 0;
        while let Some((traces, depth)) = pending.last_mut() {
            let Some(trace) = traces.next() else {
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

            // Frames supply context; child diagnostics retain their own metadata
            let (diagnostic, children) = match trace {
                // Render context as a note with its own source location
                Trace::Frame { span, message, children } => {
                    let mut diagnostic = Diagnostic::new(Severity::Note).with_message(message);
                    if *span != Span::default() {
                        self.label(
                            &Label {
                                style: LabelStyle::Secondary,
                                span: span.clone(),
                                message: String::new(),
                            },
                            &mut diagnostic,
                        )?;
                    }
                    (diagnostic, children)
                }
                // Keep each nested diagnostic's code and severity
                Trace::Diagnostic(report) => (self.diagnostic(report)?, &report.traces),
            };
            term::emit_to_write_style(buffer, &self.config.snippet, &self.files, &diagnostic)?;
            pending.push((children.iter(), depth + 1));
        }
        Ok(())
    }
}
