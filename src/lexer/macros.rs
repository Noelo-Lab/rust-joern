//! Preprocessor conditionals, source macros and retained invocations.

use super::*;

pub(super) fn source_macro(replacement: &str) -> SourceMacro<'_> {
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

pub(super) fn conditional_directive(
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

/// Whether a decimal, octal or hexadecimal integer literal is nonzero, or
/// `None` when the text is not such a literal.
pub(crate) fn integer_truth(literal: &str) -> Option<bool> {
    let number = literal.trim_end_matches(['u', 'U', 'l', 'L']);
    let (number, radix) = if number.starts_with("0x") || number.starts_with("0X") {
        (&number[2..], 16)
    } else if number.len() > 1 && number.starts_with('0') {
        (&number[1..], 8)
    } else {
        (number, 10)
    };
    u128::from_str_radix(number, radix)
        .ok()
        .map(|value| value != 0)
}

pub(super) fn macro_defined(name: &str, definitions: &MacroDefinitions<'_>, cpp: bool) -> bool {
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

pub(super) fn conditional_value(
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
    pub(super) fn take(&mut self, token: &str) -> bool {
        self.at = skip_trivia(self.text.as_bytes(), self.at);
        if self.text[self.at..].starts_with(token) {
            self.at += token.len();
            true
        } else {
            false
        }
    }

    pub(super) fn logical(&mut self, conjunction: bool) -> Option<bool> {
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

    pub(super) fn unary(&mut self) -> Option<bool> {
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
            return integer_truth(name);
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

pub(super) fn apply_empty_macros(
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
pub(super) fn retained_member_macro<'a>(
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

pub(super) fn retained_macro_invocation(
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
