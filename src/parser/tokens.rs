//! Token navigation and delimiter helpers.

use super::*;

impl Parser<'_> {
    pub(super) fn text(&self, i: usize) -> &str {
        self.tokens
            .get(i)
            .map_or("", |t| &self.source[t.span.start..t.span.end])
    }

    pub(super) fn peek(&self) -> &str {
        if self.pos < self.limit {
            self.text(self.pos)
        } else {
            ""
        }
    }

    pub(super) fn at(&self, text: &str) -> bool {
        self.peek() == text
    }

    pub(super) fn eat(&mut self, text: &str) -> bool {
        if self.at(text) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    pub(super) fn expect(&mut self, text: &str) {
        if !self.eat(text) {
            self.diagnose(
                self.pos,
                self.pos.saturating_add(1),
                format!("expected '{text}', found '{}'", self.peek()),
            );
        }
    }

    pub(super) fn span(&self, start: usize, end: usize) -> Span {
        if let Some(first) = self.tokens.get(start) {
            if end > start
                && let Some(last) = self.tokens.get(end - 1)
            {
                return Span {
                    start: first.span.start,
                    end: last.span.end,
                    line: first.span.line,
                    column: first.span.column,
                    end_line: last.span.end_line,
                };
            }
            return Span {
                end: first.span.start,
                ..first.span.clone()
            };
        }
        self.tokens.last().map_or(Span::default(), |last| Span {
            start: self.source.len(),
            end: self.source.len(),
            line: last.span.end_line,
            column: last.span.column + last.span.end - last.span.start,
            end_line: last.span.end_line,
        })
    }

    pub(super) fn raw(&self, start: usize, end: usize) -> String {
        let span = self.span(start, end);
        self.source[span.start..span.end].to_string()
    }

    pub(super) fn diagnose(&mut self, start: usize, end: usize, message: impl Into<String>) {
        self.diagnostics.push(ParseDiagnostic {
            message: message.into(),
            span: self.span(start, end),
        });
    }

    pub(super) fn matching(&self, start: usize, limit: usize) -> Option<usize> {
        let close = match self.text(start) {
            "(" => ")",
            "[" | "<:" => "]",
            "{" | "<%" => "}",
            _ => return None,
        };
        let mut stack = vec![close];
        for i in start + 1..limit {
            match self.text(i) {
                "(" => stack.push(")"),
                "[" | "<:" => stack.push("]"),
                "{" | "<%" => stack.push("}"),
                ")" | "]" | "}" | ":>" | "%>" => {
                    let t = match self.text(i) {
                        ":>" => "]",
                        "%>" => "}",
                        other => other,
                    };
                    if stack.last() != Some(&t) {
                        return None;
                    }
                    stack.pop();
                    if stack.is_empty() {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(super) fn angle_close(&self, start: usize, end: usize) -> Option<usize> {
        let mut depth = 0;
        for i in start..end {
            match self.text(i) {
                "<" => depth += 1,
                ">" => depth -= 1,
                ">>" => depth -= 2,
                _ => {}
            }
            if depth <= 0 {
                return Some(i);
            }
        }
        None
    }

    pub(super) fn split_ranges(
        &self,
        start: usize,
        end: usize,
        separator: &str,
    ) -> Vec<(usize, usize)> {
        let mut result = Vec::new();
        let (mut from, mut i) = (start, start);
        while i < end {
            if matches!(self.text(i), "(" | "[" | "{") {
                i = self.matching(i, end).map_or(i + 1, |n| n + 1);
            } else if self.cpp
                && self.text(i) == "<"
                && i > start
                && (!self.variables.contains(self.text(i - 1))
                    || (self.function_returns.contains_key(self.text(i - 1))
                        && !self.variable_types.contains_key(self.text(i - 1))))
                && (self.types.contains(self.text(i - 1))
                    || self.text(i - 1) == "template"
                    || (i >= 2 && self.text(i - 2) == "::"))
            {
                i = self.angle_close(i, end).map_or(i + 1, |n| n + 1);
            } else if self.text(i) == separator {
                result.push((from, i));
                from = i + 1;
                i += 1;
            } else {
                i += 1;
            }
        }
        result.push((from, end));
        result
    }
}
