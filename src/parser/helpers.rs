//! Keyword tables and syntax-tree helpers shared by the parser.

use super::*;

pub(super) fn closure_expression<'a>(
    expression: &'a Expr,
    closures: &'a HashMap<String, Closure>,
) -> Option<&'a Closure> {
    match &expression.kind {
        ExprKind::Lambda(closure) => Some(closure),
        ExprKind::Identifier(name) => closures.get(name),
        ExprKind::Bracketed(inner)
        | ExprKind::Generated {
            expression: inner, ..
        } => closure_expression(inner, closures),
        ExprKind::Unary { op, argument, .. } if matches!(op.as_str(), "*" | "&") => {
            closure_expression(argument, closures)
        }
        _ => None,
    }
}

pub(super) fn common_type(left: String, right: String) -> String {
    if left == right {
        return left;
    }
    if left == "ANY" || right == "ANY" {
        return "ANY".into();
    }
    for ty in [
        "long double",
        "double",
        "float",
        "unsigned long long",
        "long long",
        "unsigned long",
        "long",
        "unsigned int",
        "int",
    ] {
        if left == ty || right == ty {
            return ty.into();
        }
    }
    left
}

pub(super) fn infer_value_type(
    expression: &Expr,
    types: &HashMap<String, String>,
    closures: &HashMap<String, Closure>,
    functions: &HashMap<String, String>,
) -> String {
    match &expression.kind {
        ExprKind::Identifier(name) => types
            .get(name)
            .map(|ty| ty.trim_end_matches('&').to_string())
            .unwrap_or_else(|| "ANY".into()),
        ExprKind::Literal(value) => {
            if matches!(value.as_str(), "true" | "false") {
                "bool".into()
            } else if value.starts_with('"') {
                "char*".into()
            } else if value.starts_with('\'') {
                "char".into()
            } else if value.contains('.')
                || (!value.starts_with("0x") && value.to_lowercase().contains('e'))
            {
                if value.ends_with(['f', 'F']) {
                    "float".into()
                } else {
                    "double".into()
                }
            } else if value.ends_with(['l', 'L']) {
                "long".into()
            } else if value.ends_with(['u', 'U']) {
                "unsigned int".into()
            } else {
                "int".into()
            }
        }
        ExprKind::Cast { type_name, .. } => type_name.clone(),
        ExprKind::Bracketed(inner)
        | ExprKind::Generated {
            expression: inner, ..
        } => infer_value_type(inner, types, closures, functions),
        ExprKind::Unary { op, argument, .. } => {
            let ty = infer_value_type(argument, types, closures, functions);
            match op.as_str() {
                "!" => "bool".into(),
                "*" => ty.strip_suffix('*').unwrap_or("ANY").into(),
                "&" => format!("{ty}*"),
                "sizeof" | "alignof" => "unsigned long".into(),
                // C++ size_t varies with the target ABI; no target is available here.
                "sizeof..." => "ANY".into(),
                _ => ty,
            }
        }
        ExprKind::Binary { op, left, right } => {
            let left = infer_value_type(left, types, closures, functions);
            let right = infer_value_type(right, types, closures, functions);
            match op.as_str() {
                "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||" => "bool".into(),
                "," => right,
                "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" => left,
                "+" | "-" if left.ends_with('*') => left,
                _ => common_type(left, right),
            }
        }
        ExprKind::Conditional {
            consequence,
            alternative,
            ..
        } => common_type(
            infer_value_type(consequence, types, closures, functions),
            infer_value_type(alternative, types, closures, functions),
        ),
        ExprKind::Call { callee, .. } => {
            if let Some(closure) = closure_expression(callee, closures) {
                closure.return_type.clone()
            } else if let ExprKind::Identifier(name) = &callee.kind {
                functions
                    .get(name.rsplit("::").next().unwrap())
                    .cloned()
                    .unwrap_or_else(|| "ANY".into())
            } else {
                "ANY".into()
            }
        }
        ExprKind::Index { base, .. } => {
            let ty = infer_value_type(base, types, closures, functions);
            ty.strip_suffix('*')
                .or_else(|| ty.split_once('[').map(|(ty, _)| ty))
                .unwrap_or("ANY")
                .into()
        }
        ExprKind::List(expressions) | ExprKind::Block(expressions) => expressions
            .last()
            .map(|value| infer_value_type(value, types, closures, functions))
            .unwrap_or_else(|| "void".into()),
        _ => "ANY".into(),
    }
}

pub(super) fn infer_return_types(
    statement: &Stmt,
    types: &mut HashMap<String, String>,
    closures: &mut HashMap<String, Closure>,
    functions: &HashMap<String, String>,
    returns: &mut Vec<String>,
) {
    match &statement.kind {
        StmtKind::Block(statements) => {
            let mut types = types.clone();
            let mut closures = closures.clone();
            for statement in statements {
                infer_return_types(statement, &mut types, &mut closures, functions, returns);
            }
        }
        StmtKind::Sequence(statements) => {
            for statement in statements {
                infer_return_types(statement, types, closures, functions, returns);
            }
        }
        StmtKind::Declaration(declarations) => {
            for declaration in declarations {
                let ty = if declaration.type_name.starts_with("auto") {
                    declaration
                        .initializer
                        .as_ref()
                        .map(|value| infer_value_type(value, types, closures, functions))
                        .unwrap_or_else(|| "ANY".into())
                } else {
                    declaration.type_name.clone()
                };
                if let Some(closure) = declaration
                    .initializer
                    .as_ref()
                    .and_then(|value| closure_expression(value, closures))
                    .cloned()
                {
                    closures.insert(declaration.name.clone(), closure);
                }
                types.insert(declaration.name.clone(), ty);
            }
        }
        StmtKind::Return(value) => returns.push(
            value
                .as_ref()
                .map(|value| infer_value_type(value, types, closures, functions))
                .unwrap_or_else(|| "void".into()),
        ),
        StmtKind::If {
            consequence,
            alternative,
            ..
        } => {
            infer_return_types(
                consequence,
                &mut types.clone(),
                &mut closures.clone(),
                functions,
                returns,
            );
            if let Some(alternative) = alternative {
                infer_return_types(
                    alternative,
                    &mut types.clone(),
                    &mut closures.clone(),
                    functions,
                    returns,
                );
            }
        }
        StmtKind::RangeFor {
            initializer: Some(initializer),
            body,
            ..
        } => {
            let mut types = types.clone();
            let mut closures = closures.clone();
            infer_return_types(initializer, &mut types, &mut closures, functions, returns);
            infer_return_types(body, &mut types, &mut closures, functions, returns);
        }
        StmtKind::While { body, .. }
        | StmtKind::DoWhile { body, .. }
        | StmtKind::Switch { body, .. }
        | StmtKind::For { body, .. }
        | StmtKind::RangeFor { body, .. }
        | StmtKind::Label {
            statement: body, ..
        } => infer_return_types(
            body,
            &mut types.clone(),
            &mut closures.clone(),
            functions,
            returns,
        ),
        StmtKind::Try { body, catches } => {
            infer_return_types(
                body,
                &mut types.clone(),
                &mut closures.clone(),
                functions,
                returns,
            );
            for catch in catches {
                infer_return_types(
                    catch,
                    &mut types.clone(),
                    &mut closures.clone(),
                    functions,
                    returns,
                );
            }
        }
        _ => (),
    }
}

// CDT uses a formatted AST signature for expanded syntax, rather than the
// original macro call's source. This also governs which original arguments
// MacroHandler can find and clone in the expansion AST.
pub(super) fn expanded_code(expression: &Expr, source: &str) -> String {
    let code = |e: &Expr| {
        if let ExprKind::Bracketed(inner) = &e.kind {
            format!("({})", expanded_code(inner, source))
        } else {
            expanded_code(e, source)
        }
    };
    match &expression.kind {
        ExprKind::Identifier(name) | ExprKind::Literal(name) => name.clone(),
        ExprKind::TypeSpecifier { code, .. } => code.clone(),
        ExprKind::Binary { op, left, right } => format!("{} {op} {}", code(left), code(right)),
        ExprKind::Conditional {
            condition,
            consequence,
            alternative,
        } => format!(
            "{} ? {} : {}",
            code(condition),
            code(consequence),
            code(alternative)
        ),
        ExprKind::Cast {
            type_name,
            argument,
        } => {
            let raw = &source[expression.span.start..expression.span.end];
            let cast = [
                "static_cast",
                "reinterpret_cast",
                "const_cast",
                "dynamic_cast",
            ]
            .into_iter()
            .find(|cast| raw.starts_with(cast));
            if let Some(cast) = cast {
                format!("{cast}<{}>({})", macro_type_code(type_name), code(argument))
            } else {
                format!("({}){}", macro_type_code(type_name), code(argument))
            }
        }
        ExprKind::Unary {
            op,
            argument,
            postfix,
        } => {
            if *postfix {
                format!("{}{op}", code(argument))
            } else if matches!(
                op.as_str(),
                "sizeof"
                    | "alignof"
                    | "_Alignof"
                    | "__alignof__"
                    | "__alignof"
                    | "typeof"
                    | "__typeof__"
            ) {
                let raw = &source[expression.span.start..expression.span.end];
                // ASTSignatureUtil supplies the parentheses for a type-id;
                // an expression operand already retains its bracketed syntax.
                if !matches!(argument.kind, ExprKind::TypeSpecifier { .. }) {
                    return format!("{op} {}", code(argument));
                }
                let value = if matches!(&argument.kind,ExprKind::TypeSpecifier {code,..} if code=="typeof"||code=="__typeof__"||code=="decltype")
                {
                    code(argument)
                } else if let Some(inner) = raw
                    .strip_prefix(op)
                    .and_then(|s| s.trim().strip_prefix('('))
                    .and_then(|s| s.strip_suffix(')'))
                {
                    macro_type_code(inner)
                } else {
                    code(argument)
                };
                format!("{op} ({value})")
            } else {
                format!("{op}{}", code(argument))
            }
        }
        ExprKind::Call { callee, arguments } => format!(
            "{}({})",
            code(callee),
            arguments.iter().map(code).collect::<Vec<_>>().join(", ")
        ),
        ExprKind::Bracketed(inner) => expanded_code(inner, source),
        ExprKind::Generated { code, .. } => code.clone(),
        ExprKind::Member {
            base,
            name,
            indirect,
        } => format!("{}{}{name}", code(base), if *indirect { "->" } else { "." }),
        ExprKind::Index { base, index } => format!("{}[{}]", code(base), code(index)),
        ExprKind::List(values) | ExprKind::Block(values) => {
            values.iter().map(code).collect::<Vec<_>>().join(", ")
        }
        ExprKind::ArrayInitializer(values) => format!(
            "{{{}}}",
            values.iter().map(code).collect::<Vec<_>>().join(", ")
        ),
        _ => source[expression.span.start..expression.span.end].into(),
    }
}

pub(super) fn macro_type_code(type_name: &str) -> String {
    let type_name = type_name.trim();
    let type_name = type_name
        .strip_prefix("struct ")
        .or_else(|| type_name.strip_prefix("union "))
        .or_else(|| type_name.strip_prefix("enum "))
        .unwrap_or(type_name);
    without_typeof_operand(type_name)
        .replace('*', " *")
        .replace('&', " &")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// ASTSignatureUtil prints a typeof specifier as the keyword `typeof`,
/// without its operand: `typeof(*p) *` is `typeof *`.
pub(super) fn without_typeof_operand(type_name: &str) -> String {
    let mut out = String::new();
    let mut rest = type_name;
    while let Some(at) = ["__typeof__", "__typeof", "typeof"]
        .into_iter()
        .filter_map(|keyword| {
            let at = rest.find(keyword)?;
            let word = |c: char| c.is_alphanumeric() || c == '_';
            let after = rest[at + keyword.len()..].trim_start();
            (!rest[..at].ends_with(word) && after.starts_with('(')).then_some((at, keyword))
        })
        .min()
    {
        let (at, keyword) = at;
        let group = &rest[at + keyword.len()..];
        let open = group.find('(').unwrap_or(0);
        let mut depth = 0usize;
        let Some(close) = group[open..].char_indices().find_map(|(i, c)| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(open + i)
        }) else {
            break;
        };
        out.push_str(&rest[..at]);
        out.push_str("typeof");
        rest = &group[close + 1..];
    }
    out.push_str(rest);
    out
}

pub(super) fn expanded_argument(expression: &Expr, source: &str, argument: &str) -> Option<Expr> {
    if expanded_code(expression, source) == argument {
        return Some(expression.clone());
    }
    let children: Vec<&Expr> = match &expression.kind {
        ExprKind::Unary { argument, .. }
        | ExprKind::Cast { argument, .. }
        | ExprKind::Bracketed(argument)
        | ExprKind::Generated {
            expression: argument,
            ..
        } => vec![argument],
        ExprKind::Binary { left, right, .. } => vec![left, right],
        ExprKind::Conditional {
            condition,
            consequence,
            alternative,
        } => vec![condition, consequence, alternative],
        // Static call names and FIELD_IDENTIFIERs have no expression AST.
        ExprKind::Call { arguments, .. } => arguments.iter().collect(),
        ExprKind::Member { base, .. } => vec![base],
        ExprKind::Index { base, index } => vec![base, index],
        ExprKind::List(values) | ExprKind::Block(values) | ExprKind::ArrayInitializer(values) => {
            values.iter().collect()
        }
        _ => Vec::new(),
    };
    children
        .into_iter()
        .find_map(|child| expanded_argument(child, source, argument))
}

pub(super) fn generated_expression(expression: &mut Expr, source: &str, span: &Span) {
    let code = expanded_code(expression, source);
    match &mut expression.kind {
        ExprKind::Unary { argument, .. }
        | ExprKind::Bracketed(argument)
        | ExprKind::Generated {
            expression: argument,
            ..
        } => generated_expression(argument, source, span),
        ExprKind::Cast {
            argument,
            type_name,
        } => {
            *type_name = macro_type_code(type_name);
            generated_expression(argument, source, span);
        }
        ExprKind::Binary { left, right, .. } => {
            generated_expression(left, source, span);
            generated_expression(right, source, span);
        }
        ExprKind::Conditional {
            condition,
            consequence,
            alternative,
        } => {
            generated_expression(condition, source, span);
            generated_expression(consequence, source, span);
            generated_expression(alternative, source, span);
        }
        ExprKind::Call { callee, arguments } => {
            if matches!(callee.kind, ExprKind::Identifier(_)) {
                callee.span = span.clone();
            } else {
                generated_expression(callee, source, span);
            }
            for argument in arguments {
                generated_expression(argument, source, span);
            }
        }
        ExprKind::MacroCall {
            arguments,
            expansion,
            ..
        } => {
            for argument in arguments {
                generated_expression(argument, source, span);
            }
            generated_expression(expansion, source, span);
        }
        ExprKind::Member { base, .. } => generated_expression(base, source, span),
        ExprKind::Index { base, index } => {
            generated_expression(base, source, span);
            generated_expression(index, source, span);
        }
        ExprKind::List(values) | ExprKind::Block(values) | ExprKind::ArrayInitializer(values) => {
            for value in values {
                generated_expression(value, source, span);
            }
        }
        ExprKind::Statement(statement) => generated_statement(statement, source, span),
        _ => (),
    }
    let kind = std::mem::replace(&mut expression.kind, ExprKind::Unknown(String::new()));
    expression.kind = ExprKind::Generated {
        expression: Box::new(Expr {
            kind,
            span: span.clone(),
        }),
        code,
    };
    expression.span = span.clone();
}

pub(super) fn generated_statement(statement: &mut Stmt, source: &str, span: &Span) {
    match &mut statement.kind {
        StmtKind::Block(statements) | StmtKind::Sequence(statements) => {
            for statement in statements {
                generated_statement(statement, source, span);
            }
        }
        StmtKind::Expression(expression) => generated_expression(expression, source, span),
        StmtKind::Declaration(declarations) => {
            for declaration in declarations {
                declaration.span = span.clone();
                if let Some(expression) = &mut declaration.initializer {
                    generated_expression(expression, source, span);
                }
                for expression in &mut declaration.dimensions {
                    generated_expression(expression, source, span);
                }
            }
        }
        StmtKind::If {
            condition,
            consequence,
            alternative,
        } => {
            generated_expression(condition, source, span);
            generated_statement(consequence, source, span);
            if let Some(statement) = alternative {
                generated_statement(statement, source, span);
            }
        }
        StmtKind::While { condition, body }
        | StmtKind::Switch { condition, body }
        | StmtKind::DoWhile { condition, body } => {
            generated_expression(condition, source, span);
            generated_statement(body, source, span);
        }
        StmtKind::For {
            initializer,
            condition,
            update,
            body,
        } => {
            if let Some(statement) = initializer {
                generated_statement(statement, source, span);
            }
            for expression in [condition, update].into_iter().flatten() {
                generated_expression(expression, source, span);
            }
            generated_statement(body, source, span);
        }
        StmtKind::RangeFor {
            initializer,
            declaration,
            iterable,
            body,
        } => {
            if let Some(statement) = initializer {
                generated_statement(statement, source, span);
            }
            declaration.span = span.clone();
            generated_expression(iterable, source, span);
            generated_statement(body, source, span);
        }
        StmtKind::Case(expression) | StmtKind::Return(expression) | StmtKind::Throw(expression) => {
            if let Some(expression) = expression {
                generated_expression(expression, source, span);
            }
        }
        StmtKind::Label { statement, .. } => generated_statement(statement, source, span),
        StmtKind::Try { body, catches } => {
            generated_statement(body, source, span);
            for statement in catches {
                generated_statement(statement, source, span);
            }
        }
        StmtKind::Unknown(expressions) => {
            for expression in expressions {
                generated_expression(expression, source, span);
            }
        }
        _ => (),
    }
    statement.span = span.clone();
}

pub(super) fn qualifier(t: &str) -> bool {
    matches!(
        t,
        "const"
            | "volatile"
            | "restrict"
            | "static"
            | "extern"
            | "inline"
            | "register"
            | "typedef"
            | "constexpr"
            | "consteval"
            | "constinit"
            | "friend"
            | "virtual"
            | "mutable"
            | "thread_local"
            | "_Thread_local"
            | "__inline"
            | "__inline__"
            | "__volatile"
            | "__volatile__"
            | "__const"
            | "__const__"
            | "__restrict"
            | "__restrict__"
            | "__extension__"
            | "__cdecl"
            | "__fastcall"
            | "__stdcall"
            | "__thiscall"
            | "_Noreturn"
    )
}

pub(super) fn string_literal_token(text: &str) -> bool {
    text.find('"')
        .is_some_and(|quote| !text[..quote].contains('\''))
}

pub(super) fn recovered_statement_siblings(statement: &Stmt) -> bool {
    matches!(&statement.kind, StmtKind::Sequence(statements) if statements.first().is_some_and(|s| matches!(s.kind, StmtKind::Problem)))
}

pub(super) fn unclosed_quoted_literal(text: &str) -> bool {
    let Some(quote) = text.find(['\"', '\'']) else {
        return false;
    };
    if !matches!(&text[..quote], "" | "L" | "u" | "U" | "u8") {
        return false;
    }
    let bytes = text.as_bytes();
    let mut at = quote + 1;
    while at < bytes.len() {
        if bytes[at] == b'\\' {
            at += 2;
        } else if bytes[at] == bytes[quote] {
            return false;
        } else {
            at += 1;
        }
    }
    true
}

pub(super) fn expression_type_keyword(t: &str, cpp: bool) -> bool {
    (type_word(t)
        && (cpp || !matches!(t, "bool" | "wchar_t" | "char8_t" | "char16_t" | "char32_t")))
        || matches!(
            t,
            "struct"
                | "union"
                | "enum"
                | "const"
                | "volatile"
                | "restrict"
                | "__const"
                | "__const__"
                | "__volatile"
                | "__volatile__"
                | "__restrict"
                | "__restrict__"
        )
        || (cpp && matches!(t, "class" | "typename"))
}

pub(super) fn type_word(t: &str) -> bool {
    matches!(
        t,
        "void"
            | "char"
            | "short"
            | "int"
            | "long"
            | "float"
            | "double"
            | "signed"
            | "unsigned"
            | "_Bool"
            | "bool"
            | "wchar_t"
            | "char8_t"
            | "char16_t"
            | "char32_t"
            | "auto"
            | "__auto_type"
            | "__int8"
            | "__int16"
            | "__int32"
            | "__int64"
            | "__int128"
            | "__uint128_t"
            | "__float128"
            | "_Float16"
            | "_Float32"
            | "_Float64"
            | "_Float128"
            | "_Complex"
            | "_Imaginary"
            | "_Atomic"
            | "__complex__"
    )
}

/// Tokens of CDT's C scanner, with its GNU extensions, that are not identifiers.
pub(super) fn cdt_c_keyword(word: &str) -> bool {
    matches!(
        word,
        "auto"
            | "break"
            | "case"
            | "char"
            | "const"
            | "continue"
            | "default"
            | "do"
            | "double"
            | "else"
            | "enum"
            | "extern"
            | "float"
            | "for"
            | "goto"
            | "if"
            | "inline"
            | "int"
            | "long"
            | "register"
            | "return"
            | "short"
            | "signed"
            | "sizeof"
            | "static"
            | "struct"
            | "switch"
            | "typedef"
            | "union"
            | "unsigned"
            | "void"
            | "volatile"
            | "while"
            | "asm"
            | "restrict"
            | "_Alignas"
            | "_Alignof"
            | "_Bool"
            | "_Complex"
            | "_Imaginary"
            | "__alignof"
            | "__alignof__"
            | "__asm"
            | "__asm__"
            | "__attribute"
            | "__attribute__"
            | "__const"
            | "__const__"
            | "__declspec"
            | "__decltype"
            | "__inline"
            | "__inline__"
            | "__restrict"
            | "__restrict__"
            | "__volatile"
            | "__volatile__"
            | "__signed"
            | "__signed__"
            | "__typeof"
            | "__typeof__"
            | "typeof"
            | "_Decimal32"
            | "_Decimal64"
            | "_Decimal128"
            | "__float128"
            | "__int128"
            | "__complex__"
            | "__extension__"
            | "__stdcall"
            | "__thread"
            | "__cdecl"
            | "__fastcall"
            | "__pascal"
    )
}

/// A token that starts an attribute specifier for CDT as c2cpg configures it.
pub(super) fn cdt_attribute(word: &str) -> bool {
    matches!(
        word,
        "__attribute__" | "__attribute" | "__declspec" | "__cdecl" | "__fastcall" | "__pascal"
    )
}

/// C++ type keywords that are ordinary typedef names in C.
pub(super) fn c_typedef_name(word: &str) -> bool {
    matches!(
        word,
        "wchar_t" | "char8_t" | "char16_t" | "char32_t" | "bool"
    )
}

pub(super) fn binary_precedence(op: &str) -> Option<(u8, bool)> {
    Some(match op {
        "," => (0, false),
        "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "<<=" | ">>=" | "&=" | "^=" | "|=" => (1, true),
        "||" => (3, false),
        "&&" => (4, false),
        "|" => (5, false),
        "^" => (6, false),
        "&" => (7, false),
        "==" | "!=" => (8, false),
        "<" | ">" | "<=" | ">=" | "<=>" | "..." => (9, false),
        "<<" | ">>" => (10, false),
        "+" | "-" => (11, false),
        "*" | "/" | "%" => (12, false),
        ".*" | "->*" => (13, false),
        _ => return None,
    })
}

// CDT's ASTStringUtil renders simple return specifiers from their flags, so
// base qualifiers and signedness precede the type regardless of source order.
// Qualifiers on pointer operators keep their position after the pointer.
pub(super) fn canonical_primitive_specifiers(code: &str) -> String {
    let split = code.find(['*', '&', '[']).unwrap_or(code.len());
    let words: Vec<_> = code[..split].split_whitespace().collect();
    if words.is_empty() || !words.iter().all(|word| type_word(word)) {
        return code.into();
    }
    let mut result = Vec::new();
    for modifier in ["signed", "unsigned", "short"] {
        if words.contains(&modifier) {
            result.push(modifier);
        }
    }
    result.extend(std::iter::repeat_n(
        "long",
        words.iter().filter(|&&word| word == "long").count().min(2),
    ));
    for word in words {
        if !matches!(word, "signed" | "unsigned" | "short" | "long") && !result.contains(&word) {
            result.push(word);
        }
    }
    format!("{}{}", result.join(" "), &code[split..])
}

/// AstCreatorHelper.cleanType, including its substring keyword removal.
pub(super) fn joern_clean_type(raw: &str) -> String {
    if raw == "ANY" {
        return raw.into();
    }
    let mut cleaned = raw.to_string();
    for keyword in [
        "const",
        "static",
        "restrict",
        "extern",
        "typedef",
        "inline",
        "constexpr",
        "auto",
        "virtual",
        "enum",
        "struct",
        "interface",
        "class",
    ] {
        cleaned = cleaned.replace(&format!("{keyword} "), "");
    }
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let keep = |text: &str| text.starts_with("unsigned ") || text.starts_with("volatile ");
    // replaceWhitespaceAfterTypeKeyword keeps only the space after a leading
    // `unsigned` or `volatile`.
    let compact = |text: &str| {
        if !keep(text) {
            return text.replace(' ', "");
        }
        let mut text = text.to_string();
        for prefix in ["unsigned ", "volatile "] {
            if let Some(rest) = text.strip_prefix(prefix) {
                text = format!("{prefix}{}", rest.replace(' ', ""));
            }
        }
        text
    };
    let qualified = |text: &str| {
        text.trim_start_matches("::")
            .replace("::", ".")
            .trim_start_matches('.')
            .to_string()
    };
    if cleaned.is_empty() {
        "ANY".into()
    } else if cleaned.starts_with('[') && cleaned.ends_with(']') {
        "std.array".into()
    } else if cleaned.contains("->") {
        "std.function".into()
    } else if cleaned.contains(['?', '#', '{', '}']) {
        "ANY".into()
    } else if let Some(at) = cleaned.find("( ") {
        compact(&qualified(&cleaned[..at]))
    } else if cleaned.contains("::") {
        compact(&qualified(&cleaned))
    } else if keep(&cleaned)
        || cleaned.contains('[') && cleaned.contains(']')
        || cleaned.contains('*')
    {
        compact(&cleaned)
    } else {
        cleaned
    }
}

/// AstCreatorHelper.registerType's normalization of a type name.
pub(super) fn joern_register_type(name: &str) -> String {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    name.trim_start_matches("::")
        .replace("::", ".")
        .trim_start_matches('.')
        .to_string()
}

pub(super) fn primitive_return_display(code: &str) -> String {
    let split = code.find(['*', '&', '[']).unwrap_or(code.len());
    let words: Vec<_> = code[..split].split_whitespace().collect();
    let modifiers = ["const", "volatile", "signed", "unsigned"];
    let mut base: Vec<_> = modifiers
        .iter()
        .copied()
        .filter(|modifier| words.contains(modifier))
        .collect();
    for word in words {
        // A repeated specifier such as dewolf's `void void` sets one flag.
        if !modifiers.contains(&word) && (word == "long" || !base.contains(&word)) {
            base.push(word);
        }
    }
    format!("{}{}", base.join(" "), &code[split..]).replace("_Bool", "bool")
}

pub(super) fn simple_type_name(name: &str) -> &str {
    let mut depth = 0usize;
    let mut start = 0;
    for (at, ch) in name.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ':' if depth == 0 && name[at..].starts_with("::") => start = at + 2,
            _ => {}
        }
    }
    &name[start..]
}

pub(super) fn alternative_operator(op: &str) -> &str {
    match op {
        "and" => "&&",
        "or" => "||",
        "not" => "!",
        "bitand" => "&",
        "bitor" => "|",
        "xor" => "^",
        "compl" => "~",
        "and_eq" => "&=",
        "or_eq" => "|=",
        "xor_eq" => "^=",
        "not_eq" => "!=",
        other => other,
    }
}
