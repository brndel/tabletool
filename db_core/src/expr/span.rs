pub use chumsky::span::SimpleSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    pub value: T,
    pub span: SimpleSpan,
}

impl<T> Spanned<T> {
    pub fn new(span: SimpleSpan, value: T) -> Self {
        Self { value, span }
    }
}

impl<T> Spanned<T> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Spanned<U> {
        Spanned {
            value: f(self.value),
            span: self.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineColSpan {
    pub line_start: usize,
    pub line_end: usize,
    pub col_start: usize,
    pub col_end: usize,
}

impl LineColSpan {
    pub fn from_simple_span(span: &SimpleSpan, content: &str) -> Self {
        let mut indices = content.lines().enumerate().flat_map(|(line_idx, line)| {
            let char_iter = line
                .chars()
                .enumerate()
                .flat_map(|(i, c)| std::iter::repeat_n(i, c.len_utf8()));
            // let char_iter = 1..=line.len();
            char_iter.map(move |char_idx| (line_idx + 1, char_idx + 1))
        });

        let eof_idx = match indices.clone().last() {
            Some((line, col)) => (line, col + 1),
            None => {
                return Self {
                    line_start: 0,
                    line_end: 0,
                    col_start: 0,
                    col_end: 1,
                };
            }
        };

        if span.start >= span.end {
            let (line, col) = eof_idx;
            return Self {
                line_start: line,
                line_end: line,
                col_start: col,
                col_end: col,
            };
        }

        let start_nth = span.start;
        let start_to_end_nth = (span.end - span.start).saturating_sub(1);

        let (line_start, col_start) = indices.nth(start_nth).unwrap_or(eof_idx);

        let (line_end, col_end) = indices.nth(start_to_end_nth).unwrap_or(eof_idx);

        Self {
            line_start,
            line_end,
            col_start,
            col_end: col_end,
        }
    }

    pub fn to_simple_span(&self, content: &str) -> SimpleSpan {
        let lines = content.lines().scan(0_usize, |acc, line| {
            *acc += line.len() + 1;

            Some(*acc)
        });

        let start = lines.clone().nth(self.line_start - 1).unwrap_or_default() + self.col_start - 1;

        let end = lines.clone().nth(self.line_end - 1).unwrap_or_default() + self.col_end - 1;

        SimpleSpan::new(start, end)
    }
}

pub struct LineColPos {
    pub line: usize,
    pub col: usize,
}

impl LineColPos {
    pub fn to_index(&self, content: &str) -> usize {
        let lines = content.lines().scan(0_usize, |acc, line| {
            let acc_value = *acc;
            *acc += line.len() + 1;

            Some(acc_value)
        });

        let start = lines.clone().nth(self.line - 1).unwrap_or_default() + self.col - 1;

        start
    }
}
