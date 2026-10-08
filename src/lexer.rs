//! Tokens retain byte offsets into the original source; parsing never copies the input.
use crate::syntax::{ParseDiagnostic, RetainedMacroCall, Span};
use std::collections::HashMap;

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
    let bytes = source.as_bytes();
    let mut tokens = Vec::with_capacity(source.len() / 5);
    let mut diagnostics = Vec::new();
    let mut macros = HashMap::new();
    let mut member_macros = HashMap::new();
    let mut macro_uses = Vec::new();
    let (mut at, mut line, mut column, mut line_start) = (0, 1, 1, true);
    while at < bytes.len() {
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
            b'\\' if bytes.get(at + 1) == Some(&b'\n') => {
                at += 2;
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
                let error = match keyword {
                    "define" | "undef" => {
                        let argument = &argument[skip_trivia(argument.as_bytes(), 0)..];
                        let name_end = argument.bytes().take_while(|b| identifier_byte(*b)).count();
                        let (name, replacement) = argument.split_at(name_end);
                        if name.is_empty() || !identifier_start(name.as_bytes()[0]) {
                            Some("invalid macro name in preprocessor directive")
                        } else if keyword == "undef"
                            && skip_trivia(replacement.as_bytes(), 0) != replacement.len()
                        {
                            Some("unexpected tokens after macro name in #undef")
                        } else {
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
                    _ if directive.starts_with(|c: char| c.is_ascii_digit()) => None,
                    _ => Some(
                        "unexpanded preprocessor directive; pass a preprocessed translation unit",
                    ),
                };
                if let Some(message) = error {
                    diagnostics.push(ParseDiagnostic {
                        message: message.into(),
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
                    let mut closed = false;
                    while at < bytes.len() {
                        if bytes[at] == b'\\' {
                            at = (at + 2).min(bytes.len());
                        } else if bytes[at] == delimiter {
                            at += 1;
                            closed = true;
                            break;
                        } else {
                            at += 1;
                        }
                    }
                    if !closed {
                        diagnostics.push(ParseDiagnostic {
                            message: "unterminated string or character literal".into(),
                            span: Span {
                                start,
                                end: at,
                                line: start_line,
                                column: start_column,
                                end_line: line,
                            },
                        });
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
                    while at < bytes.len() && (identifier_byte(bytes[at]) || bytes[at] == b'\\') {
                        at += 1;
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
        if !ignored {
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
    (tokens, diagnostics, invocations)
}

/// A retained self-recursive member macro is already compiler-expanded. CDT
/// re-expands its name under the original member prefix and attaches only the
/// replacement receiver to an INLINED call. Keep that helper provenance without
/// repeating the scanner's corruption of the executable expression.
fn retained_member_macro<'a>(
    name: &'a str,
    replacement: &'a str,
    definition: Span,
) -> Option<MemberMacro<'a>> {
    let bytes = replacement.as_bytes();
    let mut at = 0;
    while bytes.get(at..at + 2) == Some(b"\\\n")
        || (bytes.get(at..at + 2) == Some(b"\\\r") && bytes.get(at + 2) == Some(&b'\n'))
    {
        at += if bytes.get(at + 1) == Some(&b'\r') {
            3
        } else {
            2
        };
    }
    if bytes.get(at) != Some(&b'(') {
        return None;
    }
    at += 1;
    let mut parameters = Vec::new();
    loop {
        at = skip_trivia(bytes, at);
        if bytes.get(at) == Some(&b')') {
            at += 1;
            break;
        }
        let start = at;
        if !bytes.get(at).is_some_and(|&b| identifier_start(b)) {
            return None;
        }
        while bytes.get(at).is_some_and(|&b| identifier_byte(b)) {
            at += 1;
        }
        parameters.push(&replacement[start..at]);
        at = skip_trivia(bytes, at);
        match bytes.get(at) {
            Some(b',') => at += 1,
            Some(b')') => {}
            _ => return None, // Variadic arity requires a different scanner contract.
        }
    }
    at = skip_trivia(bytes, at);
    let start = at;
    if !bytes.get(at).is_some_and(|&b| identifier_start(b)) {
        return None;
    }
    while bytes.get(at).is_some_and(|&b| identifier_byte(b)) {
        at += 1;
    }
    let receiver = &replacement[start..at];
    loop {
        at = skip_trivia(bytes, at);
        at += match bytes.get(at..at + 2) {
            Some(b"->") => 2,
            _ if bytes.get(at) == Some(&b'.') => 1,
            _ => return None,
        };
        at = skip_trivia(bytes, at);
        let field = at;
        if !bytes.get(at).is_some_and(|&b| identifier_start(b)) {
            return None;
        }
        while bytes.get(at).is_some_and(|&b| identifier_byte(b)) {
            at += 1;
        }
        if &replacement[field..at] == name && next_nontrivia_byte(bytes, at) == Some(b'(') {
            return Some(MemberMacro {
                name,
                definition,
                parameters,
                receiver,
            });
        }
    }
}

fn retained_macro_invocation(
    source: &str,
    tokens: &[Token],
    index: usize,
    definition: MemberMacro<'_>,
) -> Option<RetainedMacroInvocation> {
    let text = |i: usize| &source[tokens[i].span.start..tokens[i].span.end];
    if tokens.get(index + 1).is_none() || text(index + 1) != "(" {
        return None;
    }
    let mut arguments = Vec::new();
    let (mut from, mut depth) = (index + 2, 0usize);
    for i in index + 2..tokens.len() {
        match text(i) {
            ")" if depth == 0 => {
                if from != i {
                    arguments.push((from, i));
                }
                break;
            }
            "," if depth == 0 => {
                arguments.push((from, i));
                from = i + 1;
            }
            "(" | "[" | "{" | "<:" | "<%" => depth += 1,
            ")" | "]" | "}" | ":>" | "%>" if depth > 0 => depth -= 1,
            _ => {}
        }
    }
    let identifier = |range: (usize, usize)| {
        (range.1 == range.0 + 1 && tokens[range.0].kind == TokenKind::Identifier)
            .then(|| text(range.0))
    };
    let receiver = match definition
        .parameters
        .iter()
        .position(|&p| p == definition.receiver)
    {
        Some(parameter) => identifier(*arguments.get(parameter)?)?,
        None => definition.receiver,
    };
    let parameter_count = arguments
        .iter()
        .take(definition.parameters.len())
        .filter(|&&argument| identifier(argument) == Some(receiver))
        .count();
    Some(RetainedMacroInvocation {
        callee_end: tokens[index].span.end,
        descriptor: RetainedMacroCall {
            name: definition.name.into(),
            definition_span: definition.definition,
            formal_arity: definition.parameters.len(),
            parameter_count,
        },
    })
}

fn next_nontrivia_byte(bytes: &[u8], at: usize) -> Option<u8> {
    bytes.get(skip_trivia(bytes, at)).copied()
}

fn skip_trivia(bytes: &[u8], mut at: usize) -> usize {
    loop {
        while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
            at += 1;
        }
        match bytes.get(at..at.saturating_add(2)) {
            Some(b"//") => {
                while bytes.get(at).is_some_and(|b| *b != b'\n') {
                    at += 1;
                }
            }
            Some(b"/*") => {
                at += 2;
                while at + 1 < bytes.len() && &bytes[at..at + 2] != b"*/" {
                    at += 1;
                }
                at = (at + 2).min(bytes.len());
            }
            Some(b"\\\n") => at += 2,
            Some(b"\\\r") if bytes.get(at + 2) == Some(&b'\n') => at += 3,
            _ => return at,
        }
    }
}

fn identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$' || byte >= 128
}

fn identifier_byte(byte: u8) -> bool {
    identifier_start(byte) || byte.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_lines_and_literal_contents() {
        let source = "# 77 \"unit.c\"\n/* a\ncomment */ int f(){return '\\'' + 0x1p-2;}";
        let (tokens, errors) = lex(source);
        assert!(errors.is_empty());
        assert_eq!(tokens[0].span.line, 3);
        assert_eq!(&source[tokens[6].span.start..tokens[6].span.end], "'\\''");
        assert!(
            tokens
                .iter()
                .any(|t| &source[t.span.start..t.span.end] == "0x1p-2")
        );
    }

    #[test]
    fn raw_strings_and_longest_operators() {
        let source = "u8R\"tag(a \" b)tag\" >>= 1;";
        let (tokens, _) = lex(source);
        assert_eq!(tokens[0].kind, TokenKind::Literal);
        assert_eq!(&source[tokens[1].span.start..tokens[1].span.end], ">>=");
    }

    #[test]
    fn retained_compiler_directives_preserve_original_offsets() {
        // These forms occur in ChibiOS board.i and shadow chage.i after
        // DecBench strips system headers from compiler-preprocessed input.
        let source = "#define HAL_H \n#define CC_SECTION(s) __attribute__((section(s)))\n#undef STM32F407xx\n#ident \"$Id$\"\nint f(void) { return 1; }";
        let (tokens, diagnostics) = lex(source);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(&source[tokens[0].span.start..tokens[0].span.end], "int");
        assert_eq!(tokens[0].span.line, 5);
        assert_eq!(tokens[0].span.start, source.find("int f").unwrap());
    }

    #[test]
    fn raw_used_macros_are_explicit_diagnostics() {
        let source = "#define VALUE 3\n#define CALL(x) ((x) + 1)\nint f(void) { char *s = \"VALUE\"; return VALUE + CALL /* trivia */ \\\n(1); }\n#undef VALUE\nint VALUE;\nint (*not_called)(int) = CALL;";
        let (_, diagnostics) = lex(source);
        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
        let names: Vec<_> = diagnostics
            .iter()
            .map(|d| &source[d.span.start..d.span.end])
            .collect();
        assert_eq!(names, ["VALUE", "CALL"]);
        assert!(
            diagnostics
                .iter()
                .all(|d| d.message.starts_with("unexpanded macro use"))
        );
    }

    #[test]
    fn unused_and_identity_macros_do_not_require_expansion() {
        let source = "#/**/define/**/UNUSED 3\n#define IDENTITY IDENTITY\n#define OTHER 4\n#undef/**/OTHER\nint f(void) { return IDENTITY + OTHER; }";
        let (_, diagnostics) = lex(source);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn prepared_self_recursive_member_macro_remains_an_ordinary_call() {
        // The exact recursive macro and expanded call form retained in
        // Crazyflie's libdw1000.i. The compiler's expansion inhibition is
        // already reflected in these bytes, so re-expanding would be wrong.
        let source = "#define delayms(delay) dev->ops->delayms(dev, delay)\nvoid f(void) { dev->ops->delayms(dev, 5); }";
        let (tokens, diagnostics) = lex_preprocessed(source, true);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(
            tokens
                .iter()
                .any(|t| &source[t.span.start..t.span.end] == "delayms")
        );
        let (_, diagnostics) = lex(source);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            &source[diagnostics[0].span.start..diagnostics[0].span.end],
            "delayms"
        );
    }

    #[test]
    fn retained_macro_helpers_follow_active_definitions_and_cdt_argument_clones() {
        let source = "#define one(value) env->ops->one(env, value)\n\
            #define two(first, second) env->ops->two(env, first, second)\n\
            void f(void) { env->ops->one(env, 5); env->ops->one(other, 5);\n\
            env->ops->one((env), 5); env->ops->one(env + 1, 5);\n\
            env->ops->two(env, 5, 6); env->ops->two(other, 5, 6); }\n\
            #undef one\n\
            void g(void) { env->ops->one(env, 5); }\n\
            #define one(value) other->ops->one(other, value)\n\
            void h(void) { env->ops->one(env, 5); }\n\
            #define one 7\n\
            void j(void) { env->ops->one(env, 5); }";
        let (_, diagnostics, events) = lex_preprocessed_with_macros(source, true);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let summaries: Vec<_> = events
            .iter()
            .map(|event| {
                let descriptor = &event.descriptor;
                (
                    descriptor.name.as_str(),
                    descriptor.definition_span.line,
                    descriptor.formal_arity,
                    descriptor.parameter_count,
                )
            })
            .collect();
        assert_eq!(
            summaries,
            [
                ("one", 1, 1, 1),
                ("one", 1, 1, 0),
                ("one", 1, 1, 0),
                ("one", 1, 1, 0),
                ("two", 2, 2, 1),
                ("two", 2, 2, 0),
                ("one", 8, 1, 0),
            ]
        );
        assert!(
            events
                .windows(2)
                .all(|pair| pair[0].callee_end < pair[1].callee_end)
        );
        for event in events {
            assert!(source[..event.callee_end].ends_with(&event.descriptor.name));
            assert!(source[event.descriptor.definition_span.start..].starts_with("#define"));
        }
    }

    #[test]
    fn retained_macro_arguments_use_tokens_and_keep_continued_definition_spans() {
        let source = "#define relay(delay) dev->ops->\\\r\nrelay(dev,delay)\r\n\
            void f(void) { dev->ops->relay(/* ignored */ dev, 5);\n\
            dev->ops->relay(nested(dev, 1), 5);\n\
            dev->ops->relay(\"dev,5\", 5); }\n\
            #define receiver(input) input->ops->receiver(input, 5)\n\
            void g(void) { dev->ops->receiver(dev, 6); }\n\
            #define variadic(...) dev->ops->variadic(__VA_ARGS__)\n\
            void h(void) { dev->ops->variadic(dev, 5); }";
        let (_, diagnostics, events) = lex_preprocessed_with_macros(source, true);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(events.len(), 4);
        assert_eq!(
            events
                .iter()
                .map(|event| event.descriptor.parameter_count)
                .collect::<Vec<_>>(),
            [1, 0, 0, 1]
        );
        assert_eq!(events[0].descriptor.definition_span.line, 1);
        assert_eq!(events[0].descriptor.definition_span.end_line, 2);
    }

    #[test]
    fn retained_macro_metadata_preserves_cfg_and_raw_macro_uses_fail_strictly() {
        use crate::{analyze, graph::Options};

        let expanded =
            "void f(int n) { if (n) dev->ops->delayms(dev, 5); else dev->ops->delayms(dev, 10); }";
        let prepared = format!(
            "#define delayms(delay) dev->ops->delayms(dev, delay)\n#define UNUSED 1\n#undef UNUSED\n#ident \"$Id$\"\n{expanded}"
        );
        let options = Options {
            strict: true,
            preprocessed: true,
            ..Options::default()
        };
        let plain = analyze(expanded, "input.c", &options).unwrap();
        let retained = analyze(&prepared, "input.c", &options).unwrap();
        let retained_f = retained.functions.iter().find(|f| f.name == "f").unwrap();
        assert_eq!(
            serde_json::to_value(&plain.functions[0].cfg).unwrap(),
            serde_json::to_value(&retained_f.cfg).unwrap()
        );
        let helper = retained
            .functions
            .iter()
            .find(|f| f.name == "delayms")
            .unwrap();
        assert_eq!(helper.fullname, "input.c:1:1:delayms:1");
        assert_eq!(helper.start_line, 1);
        assert_eq!(helper.end_line, 1);
        assert_eq!(
            helper
                .cpg
                .nodes
                .iter()
                .filter(|node| node.kind == "METHOD_PARAMETER_IN")
                .count(),
            1
        );
        assert_eq!(helper.cfg.nodes.len(), 1);
        assert!(helper.cfg.nodes[0].is_entrypoint);
        assert!(!helper.cfg.nodes[0].is_exitpoint);
        let raw = Options {
            strict: true,
            ..Options::default()
        };
        assert!(
            analyze(&prepared, "input.c", &raw)
                .unwrap_err()
                .contains("unexpanded macro use")
        );
    }

    #[test]
    fn directive_names_are_exact_and_conditionals_require_preparation() {
        let source = "#lineage 10\n#if ENABLED\nint f(void) { return 1; }\n#endif\n#define 12 bad\n#undef VALUE + 1\n";
        let (_, diagnostics) = lex_preprocessed(source, true);
        assert_eq!(diagnostics.len(), 5, "{diagnostics:?}");
    }

    #[test]
    fn crlf_continuations_do_not_expose_directive_contents_as_code() {
        let source = "#define UNUSED \\\r\n  ignored_tokens\r\nint f(void) { return 1; }";
        let (tokens, diagnostics) = lex(source);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(&source[tokens[0].span.start..tokens[0].span.end], "int");
        assert_eq!(tokens[0].span.line, 3);
    }
}
