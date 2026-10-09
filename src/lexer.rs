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

fn source_macro(replacement: &str) -> SourceMacro<'_> {
    let bytes = replacement.as_bytes();
    if !replacement.starts_with('(') {
        return SourceMacro {
            expansion: replacement,
            function_like: false,
            empty_arity: None,
        };
    }
    let (mut at, mut arity) = (1, 0);
    loop {
        at = skip_trivia(bytes, at);
        if bytes.get(at) == Some(&b')') {
            return SourceMacro {
                expansion: &replacement[at + 1..],
                function_like: true,
                empty_arity: Some(arity),
            };
        }
        if !bytes.get(at).is_some_and(|&byte| identifier_start(byte)) {
            break;
        }
        while bytes.get(at).is_some_and(|&byte| identifier_byte(byte)) {
            at += 1;
        }
        arity += 1;
        at = skip_trivia(bytes, at);
        match bytes.get(at) {
            Some(b',') => at += 1,
            Some(b')') => {}
            _ => break,
        }
    }
    SourceMacro {
        expansion: replacement,
        function_like: true,
        empty_arity: None,
    }
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

fn conditional_directive(
    keyword: &str,
    argument: &str,
    definitions: &MacroDefinitions<'_>,
    cpp: bool,
    frames: &mut Vec<Conditional>,
    span: Span,
) -> Option<Option<&'static str>> {
    const ERROR: &str = "unsupported or invalid preprocessor conditional";
    match keyword {
        "if" | "ifdef" | "ifndef" => {
            let parent = frames.last().is_none_or(|frame| frame.active);
            let selected = if !parent {
                Some(false)
            } else if keyword == "if" {
                conditional_value(argument, definitions, cpp, 0)
            } else {
                let bytes = argument.as_bytes();
                let from = skip_trivia(bytes, 0);
                let end = from
                    + bytes[from..]
                        .iter()
                        .take_while(|&&byte| identifier_byte(byte))
                        .count();
                (end > from
                    && identifier_start(bytes[from])
                    && skip_trivia(bytes, end) == bytes.len())
                .then(|| {
                    macro_defined(&argument[from..end], definitions, cpp) == (keyword == "ifdef")
                })
            };
            frames.push(Conditional {
                parent,
                active: parent && selected.unwrap_or(true),
                taken: selected == Some(true),
                seen_else: false,
                supported: selected.is_some(),
                span,
            });
            Some(selected.is_none().then_some(ERROR))
        }
        "elif" | "else" => {
            let Some(frame) = frames.last_mut() else {
                return Some(Some(ERROR));
            };
            if frame.seen_else {
                return Some(Some(ERROR));
            }
            let selected = if keyword == "else" {
                frame.seen_else = true;
                (skip_trivia(argument.as_bytes(), 0) == argument.len()).then_some(true)
            } else if !frame.parent || frame.taken {
                Some(false)
            } else {
                conditional_value(argument, definitions, cpp, 0)
            };
            frame.supported &= selected.is_some();
            frame.active =
                frame.parent && (!frame.supported || (!frame.taken && selected == Some(true)));
            frame.taken |= selected == Some(true);
            Some((!frame.supported).then_some(ERROR))
        }
        "endif" => {
            let frame = frames.pop();
            Some(
                (frame.is_none_or(|frame| !frame.supported)
                    || skip_trivia(argument.as_bytes(), 0) != argument.len())
                .then_some(ERROR),
            )
        }
        _ => None,
    }
}

fn macro_defined(name: &str, definitions: &MacroDefinitions<'_>, cpp: bool) -> bool {
    if let Some(value) = definitions.get(name) {
        return value.is_some();
    }
    // Default bindings from the exact CDT scanner bundled with Joern v4.0.150.
    match name {
        "__cplusplus" => cpp,
        "__STDC_HOSTED__" | "__STDC_VERSION__" => !cpp,
        "__CDT_PARSER__"
        | "__COUNTER__"
        | "__DATE__"
        | "__FILE__"
        | "__LINE__"
        | "__STDC__"
        | "__TIME__"
        | "__builtin_offsetof"
        | "__builtin_types_compatible_p"
        | "__builtin_va_arg"
        | "__complex__"
        | "__extension__"
        | "__has_include"
        | "__has_include_next"
        | "__imag__"
        | "__null"
        | "__offsetof__"
        | "__real__"
        | "__stdcall"
        | "__thread" => true,
        _ => false,
    }
}

struct ConditionalExpression<'a, 'd> {
    text: &'a str,
    at: usize,
    definitions: &'d MacroDefinitions<'d>,
    cpp: bool,
    recursion: usize,
}

fn conditional_value(
    text: &str,
    definitions: &MacroDefinitions<'_>,
    cpp: bool,
    recursion: usize,
) -> Option<bool> {
    if recursion > 16 {
        return None;
    }
    let mut expression = ConditionalExpression {
        text,
        at: 0,
        definitions,
        cpp,
        recursion,
    };
    let value = expression.logical(false)?;
    (skip_trivia(text.as_bytes(), expression.at) == text.len()).then_some(value)
}

impl ConditionalExpression<'_, '_> {
    fn take(&mut self, token: &str) -> bool {
        self.at = skip_trivia(self.text.as_bytes(), self.at);
        if self.text[self.at..].starts_with(token) {
            self.at += token.len();
            true
        } else {
            false
        }
    }

    fn logical(&mut self, conjunction: bool) -> Option<bool> {
        let mut value = if conjunction {
            self.unary()?
        } else {
            self.logical(true)?
        };
        while self.take(if conjunction { "&&" } else { "||" }) {
            let right = if conjunction {
                self.unary()?
            } else {
                self.logical(true)?
            };
            value = if conjunction {
                value && right
            } else {
                value || right
            };
        }
        Some(value)
    }

    fn unary(&mut self) -> Option<bool> {
        if self.take("!") {
            return self.unary().map(|value| !value);
        }
        if self.take("(") {
            let value = self.logical(false)?;
            return self.take(")").then_some(value);
        }
        let bytes = self.text.as_bytes();
        let from = skip_trivia(bytes, self.at);
        self.at = from
            + bytes[from..]
                .iter()
                .take_while(|&&byte| identifier_byte(byte))
                .count();
        let name = &self.text[from..self.at];
        if name == "defined" {
            let grouped = self.take("(");
            let from = skip_trivia(self.text.as_bytes(), self.at);
            self.at = from
                + self.text.as_bytes()[from..]
                    .iter()
                    .take_while(|&&byte| identifier_byte(byte))
                    .count();
            if self.at == from || !identifier_start(self.text.as_bytes()[from]) {
                return None;
            }
            let value = macro_defined(&self.text[from..self.at], self.definitions, self.cpp);
            return (!grouped || self.take(")")).then_some(value);
        }
        if name.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            let number = name.trim_end_matches(['u', 'U', 'l', 'L']);
            let (number, radix) = if number.starts_with("0x") || number.starts_with("0X") {
                (&number[2..], 16)
            } else if number.len() > 1 && number.starts_with('0') {
                (&number[1..], 8)
            } else {
                (number, 10)
            };
            return u128::from_str_radix(number, radix)
                .ok()
                .map(|value| value != 0);
        }
        if let Some(definition) = self.definitions.get(name) {
            return definition
                .as_ref()
                .filter(|definition| !definition.function_like)
                .and_then(|definition| {
                    conditional_value(
                        definition.expansion,
                        self.definitions,
                        self.cpp,
                        self.recursion + 1,
                    )
                });
        }
        match name {
            "__CDT_PARSER__" | "__STDC__" => Some(true),
            "__cplusplus" if self.cpp => Some(true),
            "__STDC_HOSTED__" | "__STDC_VERSION__" if !self.cpp => Some(true),
            _ => None,
        }
    }
}

fn apply_empty_macros(
    source: &str,
    tokens: &mut Vec<Token>,
    uses: Vec<(usize, SourceMacro<'_>)>,
    diagnostics: &mut Vec<ParseDiagnostic>,
) {
    if uses.is_empty() {
        return;
    }
    let mut removed = vec![false; tokens.len()];
    let text = |index: usize| &source[tokens[index].span.start..tokens[index].span.end];
    let mut recovered = Vec::new();
    let mut expanded = Vec::new();
    for (index, definition) in uses {
        if removed[index] {
            continue;
        }
        let mut range = None;
        if skip_trivia(definition.expansion.as_bytes(), 0) == definition.expansion.len() {
            if !definition.function_like {
                range = Some((index + 1, index + 1));
            } else if let Some(arity) = definition.empty_arity {
                let (mut depth, mut commas, mut consumed) = (0, 0, None);
                for at in index + 2..tokens.len() {
                    match text(at) {
                        "(" => depth += 1,
                        ")" if depth > 0 => depth -= 1,
                        ")" => {
                            range = Some((consumed.unwrap_or(at + 1), at + 1));
                            break;
                        }
                        "," if depth == 0 => {
                            commas += 1;
                            if commas == arity {
                                consumed = Some(at + 1);
                            }
                        }
                        _ => {}
                    }
                }
                // CDT's invalid zero-parameter invocation has a separate scanner path.
                if arity == 0 && range.is_some_and(|(_, end)| end != index + 3) {
                    range = None;
                }
            }
        } else if definition.expansion.trim() == text(index) && !definition.function_like {
            continue;
        }
        if let Some((end, closing)) = range {
            removed[index..end].fill(true);
            expanded.push(if end < tokens.len() {
                end
            } else {
                index.saturating_sub(1)
            });
            if end != closing {
                recovered.push(end..closing);
            }
        } else {
            diagnostics.push(ParseDiagnostic {
                message: "unsupported macro expansion in conditional translation unit".into(),
                span: tokens[index].span.clone(),
            });
        }
    }
    for range in recovered {
        for token in &mut tokens[range] {
            token.macro_recovery = true;
        }
    }
    for index in expanded {
        tokens[index].macro_expansion = true;
    }
    let mut index = 0;
    let mut expansion = false;
    tokens.retain_mut(|token| {
        let keep = !removed[index];
        expansion |= token.macro_expansion;
        if keep && expansion {
            token.macro_expansion = true;
            expansion = false;
        }
        index += 1;
        keep
    });
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
    fn cdt_skips_outside_literal_backticks_and_keeps_original_token_spans() {
        let source = "x=x`+1; work(`x);\ngettext(\"before \"`\" after\");";
        for cpp in [false, true] {
            let (tokens, diagnostics, events) =
                lex_preprocessed_with_macros_for_language(source, true, cpp);
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            assert!(events.is_empty());
            let texts: Vec<_> = tokens
                .iter()
                .map(|token| &source[token.span.start..token.span.end])
                .collect();
            assert_eq!(
                texts,
                [
                    "x",
                    "=",
                    "x",
                    "+",
                    "1",
                    ";",
                    "work",
                    "(",
                    "x",
                    ")",
                    ";",
                    "gettext",
                    "(",
                    "\"before \"",
                    "\" after\"",
                    ")",
                    ";"
                ]
            );
            assert_eq!(tokens[3].span.start, source.find('+').unwrap());
            assert_eq!(tokens[8].span.start, source.find("`x").unwrap() + 1);
            assert_eq!(tokens[14].span.start, source.find("\" after\"").unwrap());
            assert_eq!(tokens[14].span.line, 2);
            assert_eq!(tokens[14].span.column, 19);
        }
    }

    #[test]
    fn cdt_backtick_recovery_preserves_quoted_raw_and_commented_backticks() {
        let source = "\"`\" '`' R\"tag(`)tag\" /* ` */ // `\n`int value;";
        let (tokens, diagnostics, _) =
            lex_preprocessed_with_macros_for_language(source, true, true);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let texts: Vec<_> = tokens
            .iter()
            .map(|token| &source[token.span.start..token.span.end])
            .collect();
        assert_eq!(
            texts,
            ["\"`\"", "'`'", "R\"tag(`)tag\"", "int", "value", ";"]
        );
        assert!(
            tokens[..3]
                .iter()
                .all(|token| token.kind == TokenKind::Literal)
        );
        assert_eq!(tokens[3].span.line, 2);
        assert_eq!(tokens[3].span.column, 2);
    }

    #[test]
    fn cdt_naked_backslashes_split_identifiers_and_preserve_original_offsets() {
        let source = "left\\right \\x \\n\\n";
        for cpp in [false, true] {
            let (tokens, diagnostics, events) =
                lex_preprocessed_with_macros_for_language(source, true, cpp);
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            assert!(events.is_empty());
            let texts: Vec<_> = tokens
                .iter()
                .map(|token| &source[token.span.start..token.span.end])
                .collect();
            assert_eq!(texts, ["left", "right", "x", "n", "n"]);
            assert_eq!(tokens[1].span.start, source.find("right").unwrap());
            assert_eq!(tokens[2].span.start, source.find("\\x").unwrap() + 1);
            assert_eq!(tokens[4].span.start, source.len() - 1);
        }
    }

    #[test]
    fn cdt_backslash_recovery_preserves_ucns_splices_and_quoted_backslashes() {
        let source =
            "\\u00e9 a\\u00e9 \\U000000e9 a\\\nb a\\\r\nb +\\\n1 +\\\r\n2 \"\\\\n\" '\\\\'";
        let (tokens, diagnostics, _) =
            lex_preprocessed_with_macros_for_language(source, true, true);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let texts: Vec<_> = tokens
            .iter()
            .map(|token| &source[token.span.start..token.span.end])
            .collect();
        assert_eq!(
            texts,
            [
                "\\u00e9",
                "a\\u00e9",
                "\\U000000e9",
                "a\\\nb",
                "a\\\r\nb",
                "+",
                "1",
                "+",
                "2",
                "\"\\\\n\"",
                "'\\\\'"
            ]
        );
        assert_eq!(tokens[3].span.end_line, 2);
        assert_eq!(tokens[4].span.end_line, 3);
    }

    #[test]
    fn cdt_format_escape_recovery_keeps_two_names_and_the_unclosed_literal() {
        let source = "format = \"\";\\n\\n\";\nwork(x);";
        let (tokens, diagnostics) = lex(source);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let texts: Vec<_> = tokens
            .iter()
            .map(|token| &source[token.span.start..token.span.end])
            .collect();
        assert_eq!(
            texts,
            [
                "format", "=", "\"\"", ";", "n", "n", "\";", "work", "(", "x", ")", ";"
            ]
        );
        assert_eq!(tokens[6].kind, TokenKind::Literal);
        assert_eq!(tokens[7].span.line, 2);
    }

    #[test]
    fn malformed_literals_end_at_unspliced_newlines_like_cdt() {
        let source = "\"broken\nnext;\n'broken\r\nlast;\nL\"wide\nend;";
        let (tokens, diagnostics) = lex(source);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let texts: Vec<_> = tokens
            .iter()
            .map(|token| &source[token.span.start..token.span.end])
            .collect();
        assert_eq!(
            texts,
            [
                "\"broken", "next", ";", "'broken", "last", ";", "L\"wide", "end", ";"
            ]
        );
        assert_eq!(tokens[0].kind, TokenKind::Literal);
        assert_eq!(tokens[3].kind, TokenKind::Literal);
        assert_eq!(tokens[6].kind, TokenKind::Literal);
        assert_eq!(tokens[4].span.line, 4);
        assert_eq!(tokens[4].span.start, source.find("last").unwrap());
    }

    #[test]
    fn valid_literal_escapes_and_spliced_newlines_remain_single_tokens() {
        let source = "\"a\\\"b\" 'c' '\\'' \"first\\\nsecond\" \"first\\\r\nsecond\"";
        let (tokens, diagnostics) = lex(source);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(tokens.len(), 5);
        assert!(tokens.iter().all(|token| token.kind == TokenKind::Literal));
        assert_eq!(
            &source[tokens[3].span.start..tokens[3].span.end],
            "\"first\\\nsecond\""
        );
        assert_eq!(
            &source[tokens[4].span.start..tokens[4].span.end],
            "\"first\\\r\nsecond\""
        );
        assert_eq!(tokens[3].span.end_line, 2);
        assert_eq!(tokens[4].span.end_line, 3);
    }

    #[test]
    fn incomplete_ordinary_literals_at_eof_preserve_exact_source_tokens() {
        for source in ["\"broken", "'broken", "L\"broken", "u8\"broken"] {
            let (tokens, diagnostics) = lex(source);
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            assert_eq!(tokens.len(), 1);
            assert_eq!(tokens[0].kind, TokenKind::Literal);
            assert_eq!(&source[tokens[0].span.start..tokens[0].span.end], source);
        }
    }

    #[test]
    fn conditional_default_bindings_select_the_language_and_keep_spans() {
        let source = "#if defined(_MSC_VER) || defined(__GNUC__) || defined(__i386__) || defined(_WIN32) || defined(__clang__)\nint inactive;\n#elif defined(__STDC__) && !defined(UNKNOWN)\nint active;\n#endif\n#if defined(__cplusplus)\nint cpp;\n#else\nint c;\n#endif\n";
        for cpp in [false, true] {
            let (tokens, diagnostics, _) =
                lex_preprocessed_with_macros_for_language(source, true, cpp);
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            let texts: Vec<_> = tokens
                .iter()
                .map(|token| &source[token.span.start..token.span.end])
                .collect();
            assert_eq!(
                texts,
                [
                    "int",
                    "active",
                    ";",
                    "int",
                    if cpp { "cpp" } else { "c" },
                    ";"
                ]
            );
            assert_eq!(tokens[0].span.line, 4);
            assert_eq!(
                tokens[1].span.start,
                source.find("int active;").unwrap() + 4
            );
            assert_eq!(tokens[4].span.line, if cpp { 7 } else { 9 });
        }
    }

    #[test]
    fn conditional_source_definitions_undef_and_nested_inactive_code_are_scoped() {
        let source = "#define YES 1\n#define NO 0\n#if YES && !NO\nint selected;\n#else\nint inactive;\n#endif\n#undef YES\n#ifdef YES\n#define API bad\nint rejected;\n#else\n#if 0\n#define API bad\nint nested_inactive;\n#else\n#define API\n#endif\nint API kept;\n#endif\n";
        let (tokens, diagnostics) = lex_preprocessed(source, true);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let texts: Vec<_> = tokens
            .iter()
            .map(|token| &source[token.span.start..token.span.end])
            .collect();
        assert_eq!(texts, ["int", "selected", ";", "int", "kept", ";"]);
        assert_eq!(tokens[4].span.line, 19);
    }

    #[test]
    fn conditional_empty_macros_use_definition_snapshots_and_expand_prefix_uses() {
        let source = "#define BEFORE(x)\nint BEFORE(1) f();\n#if 1\n#define API\nint API first;\n#undef API\nint API;\n#define API\nint API last;\n#endif\n";
        let (tokens, diagnostics) = lex_preprocessed(source, true);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let texts: Vec<_> = tokens
            .iter()
            .map(|token| &source[token.span.start..token.span.end])
            .collect();
        assert_eq!(
            texts,
            [
                "int", "f", "(", ")", ";", "int", "first", ";", "int", "API", ";", "int", "last",
                ";"
            ]
        );
        assert_eq!(tokens[1].span.start, source.find("f();").unwrap());
    }

    #[test]
    fn empty_macro_wrong_arity_keeps_the_exact_cdt_suffix_and_provenance() {
        let source = "#if 1\n#define ONE(a)\n#define TWO(a,b)\nONE(x); ONE(); ONE((x,y)); ONE(x,1); TWO(x,y,z); TWO(x); ONE({x,y});\n#endif\n";
        let (tokens, diagnostics) = lex_preprocessed(source, true);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let texts: Vec<_> = tokens
            .iter()
            .map(|token| &source[token.span.start..token.span.end])
            .collect();
        assert_eq!(
            texts,
            [
                ";", ";", ";", "1", ")", ";", "z", ")", ";", ";", "y", "}", ")", ";"
            ]
        );
        assert_eq!(
            tokens.iter().filter(|token| token.macro_recovery).count(),
            7
        );
        assert!(tokens[3].macro_recovery && tokens[4].macro_recovery);
        assert!(!tokens[5].macro_recovery);
        assert_eq!(tokens[3].span.start, source.find(",1").unwrap() + 1);
    }

    #[test]
    fn conditional_raw_input_and_unsupported_expansions_remain_explicit() {
        let source = "#define VALUE 3\n#if 1\nint f(){return VALUE;}\n#endif\n";
        let (_, diagnostics) = lex(source);
        assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
        let (_, diagnostics) = lex_preprocessed(source, true);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(
            &source[diagnostics[0].span.start..diagnostics[0].span.end],
            "VALUE"
        );
        let (_, diagnostics) = lex_preprocessed("#if sizeof(int)\nint f();\n#endif\n", true);
        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
        let (_, diagnostics) = lex_preprocessed("#if 1\n#define V(...)\nV(1);\n#endif\n", true);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        let (_, diagnostics) = lex_preprocessed("#if 1\n#define ZERO()\nZERO(x);\n#endif\n", true);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    }

    #[test]
    fn conditional_text_inside_literals_does_not_expand_prepared_metadata() {
        let source = "#define KEEP\nchar *s=\"#if 1\"; int KEEP;";
        let (tokens, diagnostics) = lex_preprocessed(source, true);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(
            tokens
                .iter()
                .any(|token| &source[token.span.start..token.span.end] == "KEEP")
        );
        assert!(tokens.iter().all(|token| !token.macro_recovery));
        assert!(tokens.iter().all(|token| !token.macro_expansion));
    }

    #[test]
    fn empty_macro_expansion_provenance_is_distinct_from_failed_recovery() {
        let source = "#if 1\n#define ANNOTATE(n)\nint ANNOTATE(1) f(int x){int y=ANNOTATE(x); return x;}\n#endif\n";
        let (tokens, diagnostics) = lex_preprocessed(source, true);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(tokens.iter().all(|token| !token.macro_recovery));
        let marked: Vec<_> = tokens
            .iter()
            .filter(|token| token.macro_expansion)
            .map(|token| &source[token.span.start..token.span.end])
            .collect();
        assert_eq!(marked, ["f", ";"]);
    }

    #[test]
    fn unmatched_conditionals_report_original_directive_spans() {
        for source in [
            "#endif\n",
            "#else\n",
            "#if 1\nint f();",
            "#if 1\n#else\n#else\n#endif\n",
        ] {
            let (_, diagnostics) = lex_preprocessed(source, true);
            assert_eq!(diagnostics.len(), 1, "{source:?}: {diagnostics:?}");
            assert!(source[diagnostics[0].span.start..diagnostics[0].span.end].starts_with('#'));
        }
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
    fn prepared_unresolved_includes_preserve_original_token_offsets() {
        let source = include_str!("../tests/fixtures/unresolved-includes/includes_unresolved.c");
        let (tokens, diagnostics) = lex_preprocessed(source, true);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(&source[tokens[0].span.start..tokens[0].span.end], "int");
        assert_eq!(tokens[0].span.line, 4);
        assert_eq!(
            tokens[0].span.start,
            source.find("int conditional").unwrap()
        );
        let body = tokens
            .iter()
            .find(|token| &source[token.span.start..token.span.end] == "while")
            .unwrap();
        assert_eq!(body.span.line, 7);
        assert_eq!(body.span.start, source.find("while").unwrap());
    }

    #[test]
    fn raw_includes_and_prepared_unknown_directives_still_require_expansion() {
        let source = include_str!("../tests/fixtures/unresolved-includes/includes_unresolved.c");
        let (_, diagnostics) = lex(source);
        assert_eq!(diagnostics.len(), 4, "{diagnostics:?}");
        assert_eq!(
            diagnostics.iter().map(|d| d.span.line).collect::<Vec<_>>(),
            [1, 2, 3, 6]
        );
        let (_, diagnostics) = lex_preprocessed("#includex <header.h>\n#if VALUE\n#endif\n", true);
        assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
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
