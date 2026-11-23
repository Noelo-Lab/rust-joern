//! Tokens retain byte offsets into the original source; parsing never copies the input.
use crate::syntax::{ParseDiagnostic, RetainedMacroCall, Span};
use std::collections::HashMap;

mod macros;
mod trivia;
pub(crate) use macros::*;
use trivia::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Identifier,
    Literal,
    Punctuation,
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub macro_recovery: bool,
    pub macro_expansion: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct RetainedMacroInvocation {
    pub callee_end: usize,
    pub descriptor: RetainedMacroCall,
}

#[derive(Clone)]
struct MemberMacro<'a> {
    name: &'a str,
    definition: Span,
    parameters: Vec<&'a str>,
    receiver: &'a str,
}

#[cfg(test)]
pub(crate) fn lex(source: &str) -> (Vec<Token>, Vec<ParseDiagnostic>) {
    lex_preprocessed(source, false)
}

/// Compiler `-dD` output retains definitions after expansion. In raw input,
/// report uses of those definitions instead of silently parsing a macro as C.
pub(crate) fn lex_preprocessed(
    source: &str,
    preprocessed: bool,
) -> (Vec<Token>, Vec<ParseDiagnostic>) {
    let (tokens, diagnostics, _) = lex_preprocessed_with_macros(source, preprocessed);
    (tokens, diagnostics)
}

pub(crate) fn lex_preprocessed_with_macros(
    source: &str,
    preprocessed: bool,
) -> (
    Vec<Token>,
    Vec<ParseDiagnostic>,
    Vec<RetainedMacroInvocation>,
) {
    lex_preprocessed_with_macros_for_language(source, preprocessed, false)
}

pub(crate) fn lex_preprocessed_with_macros_for_language(
    source: &str,
    preprocessed: bool,
    cpp: bool,
) -> (
    Vec<Token>,
    Vec<ParseDiagnostic>,
    Vec<RetainedMacroInvocation>,
) {
    let bytes = source.as_bytes();
    let mut tokens = Vec::with_capacity(source.len() / 5);
    let mut diagnostics = Vec::new();
    let mut macros = HashMap::new();
    let mut member_macros = HashMap::new();
    let mut macro_uses = Vec::new();
    let possible_conditionals = preprocessed
        && source.match_indices('#').any(|(offset, _)| {
            let tail = &source[offset + 1..];
            let tail = &tail[skip_trivia(tail.as_bytes(), 0)..];
            let end = tail.bytes().take_while(|b| identifier_byte(*b)).count();
            matches!(&tail[..end], "if" | "ifdef" | "ifndef")
        });
    let mut definitions = HashMap::new();
    let mut conditionals = Vec::new();
    let mut conditional_mode = false;
    let mut expansion_uses = Vec::new();
    let (mut at, mut line, mut column, mut line_start) = (0, 1, 1, true);
    while at < bytes.len() {
        let active = conditionals
            .last()
            .is_none_or(|frame: &Conditional| frame.active);
        let start = at;
        let start_line = line;
        let start_column = column;
        let mut kind = TokenKind::Punctuation;
        let mut ignored = false;
        match bytes[at] {
            b if b.is_ascii_whitespace() => {
                at += 1;
                ignored = true;
            }
            b'`' => {
                // CDT skips this invalid character before parsing adjoining tokens.
                at += 1;
                ignored = true;
            }
            b'\\' if bytes.get(at + 1) == Some(&b'\n') => {
                at += 2;
                ignored = true;
            }
            b'\\' if bytes.get(at + 1) == Some(&b'\r') && bytes.get(at + 2) == Some(&b'\n') => {
                at += 3;
                ignored = true;
            }
            b'\\' if !matches!(bytes.get(at + 1), Some(b'u' | b'U')) => {
                // CDT diagnoses and skips a non-UCN backslash outside a token.
                at += 1;
                ignored = true;
            }
            b'/' if bytes.get(at + 1) == Some(&b'/') => {
                at += 2;
                while at < bytes.len() && bytes[at] != b'\n' {
                    at += 1;
                }
                ignored = true;
            }
            b'/' if bytes.get(at + 1) == Some(&b'*') => {
                at += 2;
                while at + 1 < bytes.len() && &bytes[at..at + 2] != b"*/" {
                    at += 1;
                }
                if at + 1 < bytes.len() {
                    at += 2;
                } else {
                    at = bytes.len();
                    diagnostics.push(ParseDiagnostic {
                        message: "unterminated block comment".into(),
                        span: Span {
                            start,
                            end: at,
                            line: start_line,
                            column: start_column,
                            end_line: line,
                        },
                    });
                }
                ignored = true;
            }
            b'#' if line_start => {
                at += 1;
                while at < bytes.len() {
                    if bytes[at] == b'\n'
                        && bytes[at - 1] != b'\\'
                        && !(bytes[at - 1] == b'\r' && bytes.get(at - 2) == Some(&b'\\'))
                    {
                        break;
                    }
                    at += 1;
                }
                let directive_bytes = &bytes[start + 1..at];
                let directive = &source[start + 1 + skip_trivia(directive_bytes, 0)..at];
                let keyword_end = directive
                    .bytes()
                    .take_while(|b| identifier_byte(*b))
                    .count();
                let (keyword, argument) = directive.split_at(keyword_end);
                let directive_span = Span {
                    start,
                    end: at,
                    line: start_line,
                    column: start_column,
                    end_line: start_line + directive_bytes.iter().filter(|&&b| b == b'\n').count(),
                };
                let conditional = if preprocessed {
                    conditional_directive(
                        keyword,
                        argument,
                        &definitions,
                        cpp,
                        &mut conditionals,
                        directive_span.clone(),
                    )
                } else {
                    None
                };
                conditional_mode |= conditional.is_some();
                let error = if let Some(error) = conditional {
                    error
                } else if !active {
                    None
                } else {
                    match keyword {
                        "define" | "undef" => {
                            let argument = &argument[skip_trivia(argument.as_bytes(), 0)..];
                            let name_end =
                                argument.bytes().take_while(|b| identifier_byte(*b)).count();
                            let (name, replacement) = argument.split_at(name_end);
                            if name.is_empty() || !identifier_start(name.as_bytes()[0]) {
                                Some("invalid macro name in preprocessor directive")
                            } else if keyword == "undef"
                                && skip_trivia(replacement.as_bytes(), 0) != replacement.len()
                            {
                                Some("unexpected tokens after macro name in #undef")
                            } else {
                                if possible_conditionals {
                                    definitions.insert(
                                        name,
                                        (keyword == "define").then(|| source_macro(replacement)),
                                    );
                                }
                                if preprocessed {
                                    member_macros.remove(name);
                                    if keyword == "define"
                                        && let Some(definition) = retained_member_macro(
                                            name,
                                            replacement,
                                            Span {
                                                start,
                                                end: at,
                                                line: start_line,
                                                column: start_column,
                                                end_line: start_line
                                                    + directive_bytes
                                                        .iter()
                                                        .filter(|&&b| b == b'\n')
                                                        .count(),
                                            },
                                        )
                                    {
                                        member_macros.insert(name, definition);
                                    }
                                } else if keyword == "undef" || replacement.trim() == name {
                                    macros.remove(name);
                                } else {
                                    // Whitespace before '(' distinguishes object-like macros.
                                    macros.insert(name, replacement.starts_with('('));
                                }
                                None
                            }
                        }
                        "" | "line" | "pragma" | "ident" => None,
                        "include" if preprocessed => None,
                        _ if directive.starts_with(|c: char| c.is_ascii_digit()) => None,
                        _ => Some(
                            "unexpanded preprocessor directive; pass a preprocessed translation unit",
                        ),
                    }
                };
                if let Some(message) = error {
                    diagnostics.push(ParseDiagnostic {
                        message: message.into(),
                        span: directive_span,
                    });
                }
                ignored = true;
            }
            _ => {
                // Prefixes belong to string/character tokens, including C++ raw strings.
                let mut quote = at;
                if bytes[at] == b'u' && bytes.get(at + 1) == Some(&b'8') {
                    quote += 2;
                } else if matches!(bytes[at], b'L' | b'u' | b'U') {
                    quote += 1;
                }
                let raw = bytes.get(quote) == Some(&b'R') && bytes.get(quote + 1) == Some(&b'"');
                if raw {
                    let delimiter_start = quote + 2;
                    let mut content = delimiter_start;
                    while content < bytes.len()
                        && bytes[content] != b'('
                        && content - delimiter_start <= 16
                    {
                        content += 1;
                    }
                    if bytes.get(content) == Some(&b'(') {
                        let closing = format!("){}\"", &source[delimiter_start..content]);
                        at = source[content + 1..]
                            .find(&closing)
                            .map(|off| content + 1 + off + closing.len())
                            .unwrap_or(bytes.len());
                        kind = TokenKind::Literal;
                    } else {
                        at += 1;
                    }
                } else if matches!(bytes.get(quote), Some(b'"' | b'\'')) {
                    let delimiter = bytes[quote];
                    at = quote + 1;
                    while at < bytes.len() {
                        if bytes[at] == b'\\'
                            && bytes.get(at + 1) == Some(&b'\r')
                            && bytes.get(at + 2) == Some(&b'\n')
                        {
                            at += 3;
                        } else if bytes[at] == b'\\' {
                            at = (at + 2).min(bytes.len());
                        } else if bytes[at] == delimiter {
                            at += 1;
                            break;
                        } else if matches!(bytes[at], b'\n' | b'\r') {
                            // CDT emits the literal and recovers its enclosing syntax.
                            break;
                        } else {
                            at += 1;
                        }
                    }
                    // A C++ user-defined literal suffix does not introduce another CFG expression.
                    if bytes.get(at) == Some(&b'_') {
                        while at < bytes.len() && identifier_byte(bytes[at]) {
                            at += 1;
                        }
                    }
                    kind = TokenKind::Literal;
                } else if identifier_start(bytes[at])
                    || (bytes[at] == b'\\' && matches!(bytes.get(at + 1), Some(b'u' | b'U')))
                {
                    at += 1;
                    while at < bytes.len() {
                        if identifier_byte(bytes[at]) {
                            at += 1;
                        } else if bytes[at] == b'\\' {
                            match bytes.get(at + 1) {
                                Some(b'u' | b'U' | b'\n') => at += 2,
                                Some(b'\r') if bytes.get(at + 2) == Some(&b'\n') => at += 3,
                                _ => break,
                            }
                        } else {
                            break;
                        }
                    }
                    kind = TokenKind::Identifier;
                } else if bytes[at].is_ascii_digit()
                    || (bytes[at] == b'.' && bytes.get(at + 1).is_some_and(u8::is_ascii_digit))
                {
                    at += 1;
                    while at < bytes.len() {
                        let b = bytes[at];
                        if identifier_byte(b)
                            || matches!(b, b'.' | b'\'')
                            || (matches!(b, b'+' | b'-')
                                && matches!(bytes[at - 1], b'e' | b'E' | b'p' | b'P'))
                        {
                            at += 1;
                        } else {
                            break;
                        }
                    }
                    kind = TokenKind::Literal;
                } else {
                    const OPERATORS: &[&str] = &[
                        "%:%:", ">>=", "<<=", "<=>", "...", "->*", "++", "--", "->", ".*", "&&",
                        "||", "<=", ">=", "==", "!=", "+=", "-=", "*=", "/=", "%=", "&=", "|=",
                        "^=", "<<", ">>", "::", "##", "<:", ":>", "<%", "%>", "%:",
                    ];
                    at += OPERATORS
                        .iter()
                        .find(|op| source[start..].starts_with(**op))
                        .map_or(1, |op| op.len());
                }
            }
        }
        for b in &bytes[start..at] {
            if *b == b'\n' {
                line += 1;
                column = 1;
                line_start = true;
            } else {
                column += 1;
                if !b.is_ascii_whitespace() && !ignored {
                    line_start = false;
                }
            }
        }
        if !ignored && active {
            if possible_conditionals
                && kind == TokenKind::Identifier
                && let Some(Some(definition)) = definitions.get(&source[start..at])
                && (!definition.function_like || next_nontrivia_byte(bytes, at) == Some(b'('))
            {
                expansion_uses.push((tokens.len(), *definition));
            }
            if preprocessed
                && kind == TokenKind::Identifier
                && tokens.last().is_some_and(|token: &Token| {
                    matches!(&source[token.span.start..token.span.end], "->" | ".")
                })
                && next_nontrivia_byte(bytes, at) == Some(b'(')
                && let Some(definition) = member_macros.get(&source[start..at])
            {
                macro_uses.push((tokens.len(), definition.clone()));
            }
            if !preprocessed
                && kind == TokenKind::Identifier
                && let Some(function_like) = macros.get(&source[start..at])
                && (!function_like || next_nontrivia_byte(bytes, at) == Some(b'('))
            {
                diagnostics.push(ParseDiagnostic {
                    message: "unexpanded macro use; pass a preprocessed translation unit".into(),
                    span: Span {
                        start,
                        end: at,
                        line: start_line,
                        column: start_column,
                        end_line: line,
                    },
                });
            }
            tokens.push(Token {
                kind,
                macro_recovery: false,
                macro_expansion: false,
                span: Span {
                    start,
                    end: at,
                    line: start_line,
                    column: start_column,
                    end_line: line,
                },
            });
        }
    }
    let invocations = macro_uses
        .into_iter()
        .filter_map(|(index, definition)| {
            retained_macro_invocation(source, &tokens, index, definition)
        })
        .collect();
    if conditional_mode {
        apply_empty_macros(source, &mut tokens, expansion_uses, &mut diagnostics);
    }
    for frame in conditionals {
        diagnostics.push(ParseDiagnostic {
            message: "unterminated preprocessor conditional".into(),
            span: frame.span,
        });
    }
    (tokens, diagnostics, invocations)
}

#[derive(Clone, Copy)]
struct SourceMacro<'a> {
    expansion: &'a str,
    function_like: bool,
    empty_arity: Option<usize>,
}

struct Conditional {
    parent: bool,
    active: bool,
    taken: bool,
    seen_else: bool,
    supported: bool,
    span: Span,
}

type MacroDefinitions<'a> = HashMap<&'a str, Option<SourceMacro<'a>>>;

struct ConditionalExpression<'a, 'd> {
    text: &'a str,
    at: usize,
    definitions: &'d MacroDefinitions<'d>,
    cpp: bool,
    recursion: usize,
}

#[cfg(test)]
mod tests;
