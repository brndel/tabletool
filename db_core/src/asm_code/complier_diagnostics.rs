use std::{fmt::Display, mem};

use chumsky::span::SimpleSpan;

use crate::{asm_code::AsmCompileErr, expr::Spanned};

#[derive(Default)]
pub struct CompilerDiagnostics {
    markers: Vec<Spanned<CompilerMarker>>,
    highlights: SpanMap<CompilerHighlight>,
    completions: SpanMap<CompletionHint>,
}

pub enum CompilerMarker {
    Error(AsmCompileErr),
    Custom { message: String, kind: MarkerKind },
}

impl Display for CompilerMarker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompilerMarker::Error(asm_compile_err) => asm_compile_err.fmt(f),
            CompilerMarker::Custom { message, kind: _ } => message.fmt(f),
        }
    }
}

impl CompilerMarker {
    pub fn kind(&self) -> MarkerKind {
        match self {
            CompilerMarker::Error(_) => MarkerKind::Error,
            CompilerMarker::Custom { kind, .. } => *kind,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MarkerKind {
    Error,
    Warning,
}

#[derive(Debug)]
pub struct CompletionHint {
    pub options: Vec<String>,
}

pub struct CompilerHighlight {
    pub message: String,
}

impl CompilerDiagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_marker(&mut self, marker: Spanned<CompilerMarker>) {
        self.markers.push(marker);
    }

    pub fn add_error(&mut self, span: SimpleSpan, error: AsmCompileErr) {
        self.markers
            .push(Spanned::new(span, CompilerMarker::Error(error)));
    }

    pub fn markers(&self) -> &[Spanned<CompilerMarker>] {
        &self.markers
    }

    pub fn add_highlight(&mut self, highlight: Spanned<CompilerHighlight>) {
        self.highlights.insert(highlight);
    }

    pub fn highlights(&self) -> &SpanMap<CompilerHighlight> {
        &self.highlights
    }

    pub fn add_completion(&mut self, completion: Spanned<CompletionHint>) {
        self.completions.insert(completion);
    }

    pub fn completions(&self) -> &SpanMap<CompletionHint> {
        &self.completions
    }
}

#[derive(Debug)]
pub struct SpanMap<T> {
    spans: Vec<Spanned<T>>,
}

impl<T> Default for SpanMap<T> {
    fn default() -> Self {
        Self {
            spans: Default::default(),
        }
    }
}

impl<T> SpanMap<T> {
    pub fn insert(&mut self, value: Spanned<T>) -> Option<Spanned<T>> {
        let insert_idx = self
            .spans
            .binary_search_by(|entry| entry.span.start.cmp(&value.span.start));

        match insert_idx {
            Ok(idx) => {
                let mut span_value = value;
                mem::swap(&mut span_value, &mut self.spans[idx]);

                Some(span_value)
            }
            Err(idx) => {
                self.spans.insert(idx, value);
                None
            }
        }
    }

    pub fn get(&self, span: SimpleSpan) -> Option<&Spanned<T>> {
        self.get_at(span.start)
    }

    pub fn get_at(&self, offset: usize) -> Option<&Spanned<T>> {
        let idx = self
            .spans
            .binary_search_by(|entry| entry.span.start.cmp(&offset));

        match idx {
            Ok(idx) => Some(&self.spans[idx]),
            Err(idx) => {
                let idx = idx.checked_sub(1)?;

                let found_span = &self.spans[idx];
                if offset < found_span.span.end {
                    Some(found_span)
                } else {
                    None
                }
            }
        }
    }

    pub fn get_before(&self, offset: usize) -> Option<&Spanned<T>> {
        let idx = self
            .spans
            .binary_search_by(|entry| entry.span.start.cmp(&offset));

        match idx {
            Ok(idx) => Some(&self.spans[idx]),
            Err(idx) => {
                let idx = idx.checked_sub(1)?;

                let found_span = &self.spans[idx];
                Some(found_span)
            }
        }
    }
}
