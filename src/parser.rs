//! The C/C++ syntax used by the CDT-to-CPG conversion, without a parser runtime.
//! Unsupported executable syntax is reported, rather than silently dropping its CFG.
use std::collections::{HashMap, HashSet};

use crate::lexer::{Token, TokenKind, lex_preprocessed};
use crate::syntax::*;

#[cfg(test)]
pub fn parse(source: &str, cpp: bool) -> TranslationUnit {
    parse_preprocessed(source, cpp, false)
}

pub fn parse_preprocessed(source: &str, cpp: bool, preprocessed: bool) -> TranslationUnit {
    let (tokens, diagnostics) = lex_preprocessed(source, preprocessed);
    let limit = tokens.len();
    let mut parser = Parser {
        source,
        tokens,
        pos: 0,
        limit,
        cpp,
        diagnostics,
        types: HashSet::new(),
        explicit_types: HashSet::new(),
        type_aliases: HashMap::new(),
        extern_c: false,
        extern_c_names: HashSet::new(),
        class_scopes: HashSet::new(),
        class_fields: HashMap::new(),
        function_fields: Vec::new(),
        function_member_cv: false,
        variables: HashSet::new(),
        variable_types: HashMap::new(),
        variable_closures: HashMap::new(),
        function_returns: HashMap::new(),
        function_full_name: String::new(),
        lambda_counter: 0,
        imports: HashMap::new(),
        using_namespaces: Vec::new(),
        lexical_scope: String::new(),
        asm_problem_recovery: false,
        in_function_body: false,
        function_declarations: Vec::new(),
        global_expressions: Vec::new(),
        macro_expansion: false,
    };
    let mut functions = Vec::new();
    parser.translation_scope("", &mut functions);
    functions.append(&mut parser.function_declarations);
    let declaration_count = functions.iter().filter(|function| matches!(function.body.kind, StmtKind::Empty))
        .map(|function| function.full_name.as_str()).collect::<HashSet<_>>().len();
    // Joern collects declarations separately, and emits only those without a
    // matching definition after visiting every translation-unit declaration.
    let definitions: HashSet<_> = functions.iter()
        .filter(|f| !matches!(f.body.kind, StmtKind::Empty))
        .map(|f| f.full_name.clone())
        .collect();
    let mut declarations = HashSet::new();
    functions.retain(|f| !matches!(f.body.kind, StmtKind::Empty)
        || !definitions.contains(&f.full_name) && declarations.insert(f.full_name.clone()));
    let declaration_names: Vec<_> = functions.iter().filter(|function| matches!(function.body.kind, StmtKind::Empty))
        .map(|function| function.full_name.clone()).collect();
    let ranks = joern_declaration_order(&declaration_names, declaration_count);
    functions.sort_by_key(|f| if matches!(f.body.kind, StmtKind::Empty) {
        (true, ranks[&f.full_name])
    } else { (false, 0) });
    TranslationUnit {
        functions,
        global_expressions: parser.global_expressions,
        diagnostics: parser.diagnostics,
    }
}

fn joern_declaration_order(names: &[String], registered_count: usize) -> HashMap<String, usize> {
    // AstCreationPass copies its ConcurrentHashMap into Scala's immutable Map,
    // then removes definitions. Small Maps retain Java's bucket order; larger
    // Maps iterate the hash trie, visiting inline data before child nodes.
    fn java_hash(name: &str) -> u32 {
        name.encode_utf16().fold(0u32, |hash, unit| hash.wrapping_mul(31).wrapping_add(unit as u32))
    }
    fn improved_hash(name: &str) -> u32 {
        let hash = java_hash(name);
        let hash = hash.wrapping_add(!(hash << 9));
        let hash = hash ^ (hash >> 14);
        let hash = hash.wrapping_add(hash << 4);
        hash ^ (hash >> 10)
    }
    fn trie_order<'a>(names: Vec<&'a String>, shift: u32, output: &mut Vec<&'a String>) {
        if shift >= 32 { output.extend(names); return; }
        let mut slots: [Vec<&String>; 32] = std::array::from_fn(|_| Vec::new());
        for name in names { slots[((improved_hash(name) >> shift) & 31) as usize].push(name); }
        output.extend(slots.iter().filter(|slot| slot.len() == 1).map(|slot| slot[0]));
        for slot in slots.into_iter().filter(|slot| slot.len() > 1) { trie_order(slot, shift + 5, output); }
    }
    let mut order: Vec<_> = names.iter().collect();
    if registered_count <= 4 {
        order.sort_by_key(|name| { let hash = java_hash(name); (hash ^ (hash >> 16)) & 15 });
    } else {
        let mut sorted = Vec::with_capacity(order.len());
        trie_order(order, 0, &mut sorted);
        order = sorted;
    }
    order.into_iter().enumerate().map(|(index, name)| (name.clone(), index)).collect()
}

#[cfg(test)]
mod expression_port_tests {
    use super::*;

    #[test]
    fn indirect_calls_retain_cast_and_dereference_receiver() {
        let unit = parse("typedef void code(void); int f(int *p) { (*(code *)(p))(); return *p; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions.iter().find(|f| f.name == "f").unwrap().body.kind else { panic!() };
        assert!(matches!(&body[0].kind, StmtKind::Expression(Expr {
            kind: ExprKind::Call { callee, .. }, ..
        }) if matches!(&callee.kind, ExprKind::Unary {op, argument,..} if op == "*" && matches!(argument.kind, ExprKind::Cast {..}))));
    }

    #[test]
    fn nested_designators_repeat_operand_as_cdt_assignments() {
        let unit = parse("struct B { int n; }; struct A { struct B b; }; int f(void) { struct A a = { .b.n = 3 }; return 0; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else { panic!() };
        let StmtKind::Declaration(declarations) = &body[0].kind else { panic!() };
        let ExprKind::ArrayInitializer(values) = &declarations[0].initializer.as_ref().unwrap().kind else { panic!() };
        let ExprKind::Block(assignments) = &values[0].kind else { panic!() };
        assert_eq!(assignments.len(), 2);
        for (assignment, name) in assignments.iter().zip(["b", "n"]) {
            assert!(matches!(&assignment.kind, ExprKind::Binary {op, left, right} if op == "=" && matches!(&left.kind, ExprKind::Identifier(n) if n == name) && matches!(&right.kind, ExprKind::Literal(n) if n == "3")));
        }
    }

    #[test]
    fn terminal_problem_label_keeps_closing_brace_for_enclosing_block() {
        let unit = parse("int f(int n) { while (n--) { goto end; end: } return n; } int g(void) { return 1; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.len(), 2);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else { panic!() };
        assert_eq!(body.len(), 2);
        assert!(matches!(body[1].kind, StmtKind::Return(_)));
    }

    #[test]
    fn assembly_is_explicit_opaque_cdt_node() {
        let unit = parse("void f(void) { __asm__ __volatile__(\"x\" : : : \"memory\"); asm goto(\"y\" : : : : end); end: ; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else { panic!() };
        for statement in &body[..2] {
            assert!(matches!(&statement.kind, StmtKind::Expression(Expr {kind: ExprKind::Assembly(_),..})));
        }
    }

    #[test]
    fn following_cast_disambiguates_unknown_type_alias() {
        let unit = parse("int f(int n) { return (outer_type)(inner_type)(long double)n; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else { panic!() };
        let StmtKind::Return(Some(expression)) = &body[0].kind else { panic!() };
        let mut expression = expression;
        for name in ["outer_type", "inner_type", "long double"] {
            let ExprKind::Cast {type_name,argument} = &expression.kind else {panic!("{expression:?}")};
            assert_eq!(type_name,name);
            expression = argument;
        }
        assert!(matches!(&expression.kind,ExprKind::Identifier(name) if name == "n"));
    }

    #[test]
    fn gnu_initializer_designators_are_assignment_blocks() {
        let unit = parse("struct A {int x;}; int f(void) { struct A a={ x: 1 }; int b[8] = { [1 ... 3] = 7 }; return 0; }",false);
        assert!(unit.diagnostics.is_empty(), "{:?}",unit.diagnostics);
        let StmtKind::Block(body)=&unit.functions[0].body.kind else {panic!()};
        let StmtKind::Declaration(declarations)=&body[1].kind else {panic!()};
        let ExprKind::ArrayInitializer(values)=&declarations[0].initializer.as_ref().unwrap().kind else {panic!()};
        let ExprKind::Block(assignments)=&values[0].kind else {panic!()};
        assert!(matches!(&assignments[0].kind,ExprKind::Binary {left,..} if matches!(&left.kind,ExprKind::ArrayInitializer(bounds) if bounds.len()==2)));
    }

    #[test]
    fn microsoft_assembly_recovers_cdt_compound_boundary() {
        let source="int f(int x) { x++; __asm { mov eax, ebx } x+=2; return x; } int g(void) {return 1;}";
        for cpp in [false,true] {
            let unit=parse(source,cpp);
            assert!(unit.diagnostics.is_empty(),"{:?}",unit.diagnostics);
            assert_eq!(unit.functions.len(),2);
            let StmtKind::Block(body)=&unit.functions[0].body.kind else {panic!()};
            assert_eq!(body.len(),2);
            assert!(matches!(&body[0].kind,StmtKind::Expression(Expr {kind:ExprKind::Unary {op,..},..}) if op=="++"));
            assert!(matches!(body[1].kind,StmtKind::Problem));
            assert_eq!(unit.functions[0].span.end,source.find(" } x+=2").unwrap()+2);
        }
    }

    #[test]
    fn c_qualified_names_follow_problem_recovery_context() {
        let unit=parse("int f(int x) { ::global=x; if (::global && x) return 1; return ::foo(x); }",false);
        assert!(unit.diagnostics.is_empty(),"{:?}",unit.diagnostics);
        let StmtKind::Block(body)=&unit.functions[0].body.kind else {panic!()};
        assert_eq!(body.len(),3);
        assert!(matches!(body[0].kind,StmtKind::Problem));
        assert!(matches!(&body[1].kind,StmtKind::If {condition,..} if matches!(&condition.kind,ExprKind::Problem(text) if text=="::global && x")));
        assert!(matches!(body[2].kind,StmtKind::Problem));
    }
}

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    limit: usize,
    cpp: bool,
    diagnostics: Vec<ParseDiagnostic>,
    types: HashSet<String>,
    explicit_types: HashSet<String>,
    type_aliases: HashMap<String, String>,
    extern_c: bool,
    extern_c_names: HashSet<String>,
    class_scopes: HashSet<String>,
    class_fields: HashMap<String, Vec<(String, String)>>,
    function_fields: Vec<(String, String)>,
    function_member_cv: bool,
    variables: HashSet<String>,
    variable_types: HashMap<String, String>,
    variable_closures: HashMap<String, Closure>,
    function_returns: HashMap<String, String>,
    function_full_name: String,
    lambda_counter: usize,
    imports: HashMap<String, String>,
    using_namespaces: Vec<String>,
    lexical_scope: String,
    asm_problem_recovery: bool,
    in_function_body: bool,
    function_declarations: Vec<Function>,
    global_expressions: Vec<Expr>,
    macro_expansion: bool,
}

struct Header {
    name: String,
    name_start: usize,
    parameters_start: usize,
    parameters_end: usize,
    return_type: Option<String>,
}

impl Parser<'_> {
    fn text(&self, i: usize) -> &str {
        self.tokens
            .get(i)
            .map_or("", |t| &self.source[t.span.start..t.span.end])
    }

    fn peek(&self) -> &str {
        if self.pos < self.limit {
            self.text(self.pos)
        } else {
            ""
        }
    }

    fn at(&self, text: &str) -> bool {
        self.peek() == text
    }

    fn eat(&mut self, text: &str) -> bool {
        if self.at(text) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, text: &str) {
        if !self.eat(text) {
            self.diagnose(
                self.pos,
                self.pos.saturating_add(1),
                format!("expected '{text}', found '{}'", self.peek()),
            );
        }
    }

    fn span(&self, start: usize, end: usize) -> Span {
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

    fn raw(&self, start: usize, end: usize) -> String {
        let span = self.span(start, end);
        self.source[span.start..span.end].to_string()
    }

    fn diagnose(&mut self, start: usize, end: usize, message: impl Into<String>) {
        self.diagnostics.push(ParseDiagnostic {
            message: message.into(),
            span: self.span(start, end),
        });
    }

    fn matching(&self, start: usize, limit: usize) -> Option<usize> {
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

    fn translation_scope(&mut self, prefix: &str, functions: &mut Vec<Function>) {
        let old_scope = std::mem::replace(&mut self.lexical_scope, prefix.to_string());
        let old_using_namespaces = self.using_namespaces.clone();
        while self.pos < self.limit && !self.at("}") {
            if self.eat(";") {
                continue;
            }
            // This CDT version treats a leading _Noreturn as a separate
            // non-executable problem token, before parsing the declaration.
            if self.eat("_Noreturn") {
                continue;
            }
            if matches!(self.peek(), "asm" | "__asm" | "__asm__") {
                let start = self.pos;
                self.pos += 1;
                while matches!(self.peek(), "volatile" | "__volatile" | "__volatile__" | "inline" | "goto") {
                    self.pos += 1;
                }
                if matches!(self.peek(), "(" | "{") && let Some(close) = self.matching(self.pos, self.limit) {
                    self.pos = close + 1;
                    self.eat(";");
                } else {
                    self.diagnose(start, self.pos, "cannot recover assembly declaration operand");
                }
                continue;
            }
            if matches!(self.peek(), "public" | "private" | "protected")
                && self.text(self.pos + 1) == ":"
            {
                self.pos += 2;
                continue;
            }
            let start = self.pos;
            let mut cursor = start;
            while cursor < self.limit {
                match self.text(cursor) {
                    "(" | "[" => {
                        if let Some(close) = self.matching(cursor, self.limit) {
                            cursor = close + 1;
                        } else {
                            break;
                        }
                    }
                    ";" | "{" | "}" => break,
                    _ => cursor += 1,
                }
            }
            if cursor == self.limit {
                if start < cursor {
                    self.diagnose(start, cursor, "unfinished top-level declaration");
                }
                self.pos = cursor;
                break;
            }
            if self.text(cursor) == "}" {
                break;
            }
            let is_typedef = (start..cursor).any(|i| self.text(i) == "typedef");
            let header = if is_typedef {
                None
            } else {
                self.function_header(start, cursor)
            };
            // CDT accepts K&R definitions. Their parameter declarations contain
            // semicolons before the function body, unlike ordinary prototypes.
            if self.text(cursor) == ";"
                && let Some(ref header) = header
                && let Some(body) = self.knr_body_start(header)
            {
                cursor = body;
            }
            if self.text(cursor) == ";" {
                // CDT's elaborated-type declaration path creates declarator
                // locals, including function declarators, rather than methods.
                let elaborated = (start..self.specifier_end(start, cursor))
                    .any(|i| matches!(self.text(i), "struct" | "union" | "enum" | "class"));
                self.remember_types(start, cursor);
                if self.cpp && self.text(start) == "using" {
                    self.pos = cursor + 1;
                    continue;
                }
                if !is_typedef {
                    self.collect_global_expressions(start, cursor);
                }
                if !is_typedef && !elaborated {
                    // A simple declaration shares its specifiers across every
                    // declarator; Joern converts each function declarator.
                    // A constructor's name is also a known type. It belongs to
                    // the declarator, rather than the shared type specifiers.
                    let base_end = header.as_ref().map_or_else(
                        || self.specifier_end(start, cursor),
                        |header| self.specifier_end(start, cursor).min(header.name_start),
                    );
                    let base_type = self.clean_type(start, base_end);
                    for (a, b) in self.split_ranges(base_end, cursor, ",") {
                        if let Some(mut header) = self.function_header(a, b) {
                            let declarator_type = header.return_type.take()
                                .unwrap_or_else(|| self.clean_type(a, header.name_start));
                            header.return_type = Some(format!("{base_type}{declarator_type}"));
                            self.add_function(start, cursor, header, prefix, false, functions);
                        } else if let (Some(name), _) = self.declarator_name(a, b) {
                            self.variables.insert(self.text(name).to_string());
                        }
                    }
                } else {
                    self.remember_types(start, cursor);
                    if !is_typedef {
                        let base_end = self.specifier_end(start, cursor);
                        for (a, b) in self.split_ranges(base_end, cursor, ",") {
                            if let (Some(name), _) = self.declarator_name(a, b) {
                                self.variables.insert(self.text(name).to_string());
                            }
                        }
                    }
                }
                self.pos = cursor + 1;
            } else if let Some(header) = header {
                self.add_function(start, cursor, header, prefix, true, functions);
            } else {
                let namespace = (start..cursor).find(|i| self.cpp && self.text(*i) == "namespace");
                let class = self.record_definition(start, cursor);
                let linkage = self.text(start) == "extern"
                    && self
                        .tokens
                        .get(start + 1)
                        .is_some_and(|t| t.kind == TokenKind::Literal);
                if namespace.is_some() || class.is_some() || linkage {
                    let introducer = namespace.or(class);
                    let name = introducer
                        .map(|i| {
                            let mut after = i + 1;
                            while let Some(next) = self.skip_attribute(after, cursor) {
                                after = next;
                            }
                            after
                        })
                        .filter(|i| {
                            self.tokens
                                .get(*i)
                                .is_some_and(|t| t.kind == TokenKind::Identifier)
                        })
                        .map(|i| self.text(i).to_string())
                        .unwrap_or_default();
                    if self.cpp && class.is_some() && !name.is_empty() {
                        self.types.insert(name.clone());
                        self.explicit_types.insert(name.clone());
                    }
                    let nested = if name.is_empty() {
                        prefix.to_string()
                    } else if prefix.is_empty() {
                        name
                    } else {
                        format!("{prefix}::{name}")
                    };
                    if self.cpp && class.is_some() && !nested.is_empty() {
                        self.class_scopes.insert(nested.clone());
                        let close = self.matching(cursor, self.limit).unwrap_or(self.limit);
                        let fields = self.collect_class_fields(cursor + 1, close);
                        self.class_fields.insert(nested.clone(), fields);
                    }
                    let previous_linkage = self.extern_c;
                    if self.cpp && linkage {
                        self.extern_c = self.text(start + 1) == "\"C\"";
                    }
                    self.pos = cursor + 1;
                    self.translation_scope(&nested, functions);
                    self.extern_c = previous_linkage;
                    self.expect("}");
                    // Names after an anonymous record, including typedef aliases.
                    let after = self.pos;
                    if class.is_some() {
                        self.pos = self.declaration_end(after);
                        if !is_typedef {
                            self.collect_global_expressions(after, self.pos);
                        }
                        for (a, b) in self.split_ranges(after, self.pos, ",") {
                            if let (Some(name), _) = self.declarator_name(a, b) {
                                if is_typedef {
                                    self.types.insert(self.text(name).to_string());
                                    self.explicit_types.insert(self.text(name).to_string());
                                } else {
                                    self.variables.insert(self.text(name).to_string());
                                }
                            }
                        }
                        self.eat(";");
                    }
                } else if let Some(close) = self.matching(cursor, self.limit) {
                    // Aggregate initializer, enum, or non-executable record data.
                    if !(start..cursor).any(|i| matches!(self.text(i), "=" | "enum")) {
                        self.diagnose(
                            start,
                            close + 1,
                            "unrecognized top-level brace; possible omitted function definition",
                        );
                    }
                    self.pos = self.declaration_end(close + 1);
                    self.remember_types(start, self.pos);
                    if !is_typedef {
                        self.collect_global_expressions(start, self.pos);
                    }
                    self.eat(";");
                } else {
                    self.diagnose(cursor, cursor + 1, "unclosed top-level brace");
                    self.pos = self.limit;
                }
            }
            if self.pos <= start {
                self.diagnose(start, start + 1, "cannot recover top-level syntax");
                self.pos = start + 1;
            }
        }
        self.lexical_scope = old_scope;
        self.using_namespaces = old_using_namespaces;
    }

    fn declaration_end(&self, start: usize) -> usize {
        let mut i = start;
        while i < self.limit && !matches!(self.text(i), ";" | "}") {
            if matches!(self.text(i), "(" | "[" | "{") {
                i = self.matching(i, self.limit).map_or(i + 1, |close| close + 1);
            } else {
                i += 1;
            }
        }
        i
    }

    fn knr_body_start(&self, header: &Header) -> Option<usize> {
        let parameters = self.split_ranges(header.parameters_start, header.parameters_end, ",");
        if parameters.is_empty() || parameters.iter().any(|&(a, b)| b != a + 1
            || self.tokens[a].kind != TokenKind::Identifier
            || type_word(self.text(a)) || qualifier(self.text(a))) {
            return None;
        }
        let mut from = header.parameters_end + 1;
        let mut declared = false;
        while from < self.limit {
            if self.text(from) == "{" {
                return declared.then_some(from);
            }
            let mut end = from;
            while end < self.limit && !matches!(self.text(end), ";" | "{" | "}" | "=") {
                if matches!(self.text(end), "(" | "[") {
                    end = self.matching(end, self.limit)? + 1;
                } else {
                    end += 1;
                }
            }
            if self.text(end) != ";" {
                return None;
            }
            let base_end = self.specifier_end(from, end);
            if base_end == from || base_end == end || self.split_ranges(base_end, end, ",").iter()
                .any(|&(a, b)| self.declarator_name(a, b).0
                    .is_none_or(|name| !parameters.iter().any(|&(p, _)| self.text(p) == self.text(name)))) {
                return None;
            }
            declared = true;
            from = end + 1;
        }
        None
    }

    fn record_definition(&self, start: usize, end: usize) -> Option<usize> {
        if (start..end).any(|i| self.text(i) == "=") {
            return None;
        }
        let introducer = (start..end).find(|i| matches!(self.text(*i), "struct" | "class" | "union"))?;
        let mut after = introducer + 1;
        while let Some(next) = self.skip_attribute(after, end) {
            after = next;
        }
        if after < end && self.tokens[after].kind == TokenKind::Identifier {
            after += 1;
        }
        while after < end {
            if let Some(next) = self.skip_attribute(after, end) {
                after = next;
            } else if self.text(after) == "final" {
                after += 1;
            } else {
                break;
            }
        }
        (after == end || self.text(after) == ":").then_some(introducer)
    }

    fn function_header(&self, start: usize, end: usize) -> Option<Header> {
        if matches!(self.text(start), "static_assert" | "_Static_assert")
            || (self.cpp && self.text(start) == "using") {
            return None;
        }
        if let Some(header) = self.grouped_function_header(start, end) {
            return Some(header);
        }
        let mut i = start;
        while i < end {
            if matches!(
                self.text(i),
                "__attribute__"
                    | "__attribute"
                    | "__declspec"
                    | "alignas"
                    | "__asm__"
                    | "asm"
                    | "typeof"
                    | "__typeof__"
                    | "decltype"
                    | "sizeof"
                    | "alignof"
                    | "_Alignof"
                    | "__alignof__"
            ) {
                i += 1;
                if self.text(i) == "(" {
                    i = self.matching(i, end).map_or(i + 1, |n| n + 1);
                }
                continue;
            }
            if self.cpp && matches!(self.text(i), "throw" | "noexcept") {
                i += 1;
                if self.text(i) == "(" {
                    i = self.matching(i, end).map_or(i + 1, |close| close + 1);
                }
                continue;
            }
            if self.text(i) == "[" {
                i = self.matching(i, end).map_or(i + 1, |n| n + 1);
                continue;
            }
            if self.text(i) != "(" || i == start {
                i += 1;
                continue;
            }
            let close = self.matching(i, end)?;
            // A typedef followed by a grouped object declarator is not an
            // implicit function name (e.g. byte (mask)[8]).
            if i == start + 1 && self.explicit_types.contains(self.text(start))
                && (!self.cpp || self.text(close + 1) == "[") {
                i = close + 1;
                continue;
            }
            if self.text(i - 1) == "operator" && close == i + 1 && self.text(close + 1) == "(" {
                i = close + 1;
                continue;
            }
            let mut name_end = i;
            let mut name_start = i - 1;
            if name_start > start && self.text(name_start - 1) == "operator" {
                name_start -= 1;
            } else if matches!(self.text(name_start), ")" | "]")
                && name_start > start
                && matches!(self.text(name_start - 1), "(" | "[")
                && name_start >= 2
                && self.text(name_start - 2) == "operator"
            {
                name_start -= 2;
            } else if self.text(name_start) == "]" && name_start >= start + 3
                && self.text(name_start - 1) == "["
                && matches!(self.text(name_start - 2), "new" | "delete")
                && self.text(name_start - 3) == "operator"
            {
                name_start -= 3;
            } else if self.tokens[name_start].kind != TokenKind::Identifier
                || type_word(self.text(name_start))
                || qualifier(self.text(name_start))
            {
                i += 1;
                continue;
            }
            if name_start > start && self.text(name_start - 1) == "~" {
                name_start -= 1;
            }
            while name_start >= start + 2
                && self.text(name_start - 1) == "::"
                && self.tokens[name_start - 2].kind == TokenKind::Identifier
            {
                name_start -= 2;
            }
            if (start..name_start).any(|n| self.text(n) == "=" || self.text(n) == ":") {
                i = close + 1;
                continue;
            }
            // A direct initializer with values is not a function prototype.
            if self.split_ranges(i + 1, close, ",").iter().any(|(a, b)| {
                a < b
                    && (self.tokens[*a].kind == TokenKind::Literal
                        || matches!(self.text(*a), "{" | "+" | "-" | "!" | "~" | "*" | "&" | "&&"))
            }) {
                i = close + 1;
                continue;
            }
            if self.cpp && close == i + 2 && self.variables.contains(self.text(i + 1)) {
                i = close + 1;
                continue;
            }
            // Handle operator names without spaces; ordinary qualified names use ::.
            if name_start > start && self.text(name_start - 1) == "operator" {
                name_start -= 1;
            }
            if name_start >= name_end {
                name_end = i;
            }
            let name = (name_start..name_end)
                .map(|n| self.text(n))
                .collect::<String>();
            return Some(Header {
                name,
                name_start,
                parameters_start: i + 1,
                parameters_end: close,
                return_type: None,
            });
        }
        None
    }

    fn grouped_function_header(&self, start: usize, end: usize) -> Option<Header> {
        let base_end = self.specifier_end(start, end);
        let mut group = base_end;
        while group < end && (matches!(self.text(group), "*" | "&" | "&&") || qualifier(self.text(group))) {
            group += 1;
        }
        if self.text(group) != "(" {
            return None;
        }
        let close = self.matching(group, end)?;
        let mut header = if let Some(header) = self.function_header(group + 1, close) {
            header
        } else {
            let (name, suffix) = self.declarator_name(group + 1, close);
            let name = name?;
            if suffix != close || (group + 1..name).any(|i| matches!(self.text(i), "*" | "&" | "&&"))
                || self.text(close + 1) != "(" {
                return None;
            }
            let mut name_start = name;
            while name_start >= group + 3 && self.text(name_start - 1) == "::" {
                name_start -= 2;
            }
            Header {
                name: (name_start..=name).map(|i| self.text(i)).collect(),
                name_start,
                parameters_start: close + 2,
                parameters_end: self.matching(close + 1, end)?,
                return_type: None,
            }
        };
        let mut return_type = self.clean_type(start, base_end);
        for i in base_end..header.name_start {
            if matches!(self.text(i), "*" | "&" | "&&") {
                return_type.push_str(self.text(i));
            }
        }
        let mut i = close + 1;
        while i < end {
            if self.text(i) == "[" {
                let close = self.matching(i, end)?;
                return_type.push_str(&self.raw(i, close + 1));
                i = close + 1;
            } else if self.text(i) == "(" {
                i = self.matching(i, end)? + 1;
            } else {
                i += 1;
            }
        }
        header.return_type = Some(return_type);
        Some(header)
    }

    fn collect_class_fields(&self, start: usize, end: usize) -> Vec<(String, String)> {
        let mut fields = Vec::new();
        let mut at = start;
        while at < end {
            if matches!(self.text(at), "public" | "private" | "protected") && self.text(at + 1) == ":" {
                at += 2;
                continue;
            }
            let from = at;
            while at < end && !matches!(self.text(at), ";" | "{") {
                if matches!(self.text(at), "(" | "[") {
                    at = self.matching(at, end).map_or(end, |close| close + 1);
                } else {
                    at += 1;
                }
            }
            if at >= end { break; }
            let method = self.function_header(from, at).is_some();
            if self.text(at) == "{" {
                at = self.matching(at, end).map_or(end, |close| close + 1);
                if method { continue; }
                while at < end && self.text(at) != ";" { at += 1; }
            }
            if !method && !(from..at).any(|i| matches!(self.text(i), "static" | "typedef" | "using" | "struct" | "class" | "union" | "enum")) {
                let base = self.specifier_end(from, at);
                for (a, b) in self.split_ranges(base, at, ",") {
                    if let (Some(name), _) = self.declarator_name(a, b) {
                        fields.push((self.text(name).into(), self.declarator_type(from, base, Some(name), b)));
                    }
                }
            }
            at += 1;
        }
        fields
    }

    fn add_function(
        &mut self,
        start: usize,
        body_start: usize,
        header: Header,
        prefix: &str,
        definition: bool,
        functions: &mut Vec<Function>,
    ) {
        if self.cdt_problem_declaration(start, body_start) {
            // The original CDT version recovers these as problem declarations;
            // Joern's declaration conversion emits no method for them.
            if definition {
                self.pos = self.matching(body_start, self.limit).map_or(self.limit, |close| close + 1);
            }
            return;
        }
        let mut return_start = start;
        if self.text(return_start) == "template"
            && self.text(return_start + 1) == "<"
            && let Some(close) = self.angle_close(return_start + 1, header.name_start)
        {
            return_start = close + 1;
        }
        let return_type = header.return_type.clone().unwrap_or_else(|| self.clean_type(return_start, header.name_start));
        let return_binding_type = if return_type.is_empty() && self.cpp { "ANY".into() }
            else if return_type.is_empty() { "int".into() } else { return_type.clone() };
        let return_type = if return_type.is_empty() {
            if !definition && self.cpp && (return_start..header.name_start).any(|i| self.text(i) == "virtual") {
                "virtual".into()
            } else { return_binding_type.clone() }
        } else if definition {
            let base = return_type.trim_end_matches(['*', '&']).trim();
            if base.split_whitespace().all(type_word) {
                primitive_return_display(&self.type_code(return_start, header.name_start, true))
            } else if self.explicit_types.contains(base) || self.class_scopes.contains(base)
                || base.contains("::") {
                simple_type_name(base).to_string()
            } else { return_type }
        } else { return_type };
        let old_variables = self.variables.clone();
        let old_variable_types = self.variable_types.clone();
        let old_variable_closures = self.variable_closures.clone();
        let old_function_full_name = self.function_full_name.clone();
        let old_function_fields = self.function_fields.clone();
        let old_function_member_cv = self.function_member_cv;
        let old_body_context = self.in_function_body;
        self.in_function_body = false;
        let parameters =
            if definition && (header.parameters_end + 1..body_start).any(|i| self.text(i) == ";") {
                let mut declared = Vec::new();
                for (a, b) in self.split_ranges(header.parameters_end + 1, body_start, ";") {
                    if a < b {
                        let span = self.span(a, b + 1);
                        declared.extend(self.declarations_range(a, b).into_iter().map(|d| (d, span.clone())));
                    }
                }
                self.split_ranges(header.parameters_start, header.parameters_end, ",")
                    .into_iter()
                    .filter(|(a, b)| a < b)
                    .map(|(a, b)| {
                        let name = self.raw(a, b);
                        let declaration = declared.iter().find(|(d, _)| d.name == name);
                        let type_name = declaration.map_or("int".into(), |(d, _)| d.type_name.clone());
                        Parameter {
                            name,
                            type_name,
                            function_pointer: false,
                            span: declaration.map_or_else(|| self.span(a, b), |(_, span)| span.clone()),
                        }
                    })
                    .collect()
            } else {
                self.parameters(header.parameters_start, header.parameters_end)
            };
        let signature = format!(
            "{}({})",
            return_type,
            parameters
                .iter()
                .map(|p| p.type_name.as_str())
                .collect::<Vec<_>>()
                .join(",")
        );
        for parameter in &parameters {
            self.variables.insert(parameter.name.clone());
            self.variable_types.insert(parameter.name.clone(), parameter.type_name.clone());
        }
        self.variables.insert(header.name.clone());
        if definition && (header.parameters_end + 1..body_start).any(|i| self.text(i) == ":") {
            self.diagnose(
                header.parameters_end + 1,
                body_start,
                "constructor member-initializer CFG is unsupported",
            );
        }
        let lexical_name = header
            .name
            .rsplit("::")
            .next()
            .unwrap_or(&header.name)
            .to_string();
        let name = if let Some(operator) = lexical_name.strip_prefix("operator") {
            if definition { operator.to_string() } else { format!("operator {operator}") }
        } else {
            lexical_name
        };
        let qualified_name = if let Some((owner, method)) = header.name.rsplit_once("::")
            && let Some(owner) = self.resolve_class_name(owner)
        {
            format!("{owner}::{method}")
        } else if prefix.is_empty() { header.name.clone() }
        else { format!("{prefix}::{}", header.name) };
        let extern_c = self.cpp && (self.extern_c
            || (start..header.name_start).any(|i| self.text(i) == "extern" && self.text(i + 1) == "\"C\"")
            || self.extern_c_names.contains(&qualified_name));
        if extern_c {
            self.extern_c_names.insert(qualified_name.clone());
        }
        let implicit_this = (self.cpp && definition).then(|| {
            qualified_name.rsplit_once("::")
                .filter(|(owner, _)| self.class_scopes.contains(*owner))
                .map(|(owner, _)| owner.replace("::", "."))
        }).flatten();
        let implicit_fields = qualified_name.rsplit_once("::")
            .and_then(|(owner, _)| self.class_fields.get(owner)).cloned().unwrap_or_default();
        let full_name = if self.cpp && !extern_c {
            let parameter_types = self.split_ranges(header.parameters_start, header.parameters_end, ",")
                .into_iter().filter(|(a,b)| a < b)
                .map(|(a,b)| self.binding_parameter_type(a,b))
                .filter(|t| t != "void" && t != "...")
                .collect::<Vec<_>>().join(",");
            format!("{}:{}({parameter_types})", qualified_name.replace("::", "."), self.binding_type(&return_binding_type))
        } else if extern_c {
            qualified_name.rsplit("::").next().unwrap_or(&qualified_name).to_string()
        } else {
            qualified_name
        };
        self.function_full_name.clone_from(&full_name);
        self.function_fields.clone_from(&implicit_fields);
        let mut suffix = header.parameters_end + 1;
        let mut member_cv_qualified = false;
        while suffix < body_start && !matches!(self.text(suffix), "->" | ":") {
            if matches!(self.text(suffix), "const" | "volatile") {
                member_cv_qualified = true;
            }
            suffix = if matches!(self.text(suffix), "(" | "[") {
                self.matching(suffix, body_start).map_or(body_start, |close| close + 1)
            } else { suffix + 1 };
        }
        self.function_member_cv = member_cv_qualified;
        if let Some(owner) = &implicit_this {
            self.variables.insert("this".into());
            self.variable_types.insert("this".into(), owner.clone());
        }
        self.function_returns.insert(name.clone(), return_type.clone());
        self.pos = body_start;
        self.in_function_body = definition;
        let original_body_end = definition.then(|| self.matching(body_start, self.limit)).flatten();
        self.asm_problem_recovery = false;
        let body = if definition {
            self.statement()
        } else {
            Stmt {
                kind: StmtKind::Empty,
                span: self.span(body_start, body_start),
            }
        };
        let end = if definition { self.pos } else { body_start + 1 };
        if self.asm_problem_recovery && let Some(body_end) = original_body_end {
            // CDT closes the enclosing compound at a problem assembly brace.
            // The balanced source body still delimits recovery of later methods.
            self.pos = body_end + 1;
        }
        self.variables = old_variables;
        self.variable_types = old_variable_types;
        self.variable_closures = old_variable_closures;
        self.function_full_name = old_function_full_name;
        self.function_fields = old_function_fields;
        self.function_member_cv = old_function_member_cv;
        self.in_function_body = old_body_context;
        self.variables.insert(name.clone());
        let mut span = self.span(start, end);
        if !definition {
            let name_span = self.span(header.name_start, header.name_start + 1);
            span.line = name_span.line;
            span.column = name_span.column;
        }
        functions.push(Function {
            name,
            full_name,
            binding_return_type: self.binding_type(&return_binding_type),
            return_type,
            signature,
            implicit_this,
            implicit_fields,
            member_cv_qualified,
            lambda: false,
            is_static: (start..header.name_start).any(|i| self.text(i) == "static"),
            inherited_bindings: Vec::new(),
            inherited_closures: Vec::new(),
            parameters,
            body,
            span,
        });
    }

    fn cdt_problem_declaration(&self, start: usize, end: usize) -> bool {
        (start..end).any(|i| {
            (self.text(i) == "_Noreturn"
                && (self.cpp || self.tokens[start].span.line == self.tokens[i + 1].span.line))
                || (!self.cpp && self.text(i) == "[" && self.text(i + 1) == "[")
                || (matches!(self.text(i), "__attribute__" | "__attribute")
                    && self.text(i + 1) == "(" && self.text(i + 2) == "("
                    && self.text(i + 3) == ")" && self.text(i + 4) == ")")
        })
    }

    fn collect_global_expressions(&mut self, start: usize, end: usize) {
        let base_end = self.specifier_end(start, end);
        for (a, b) in self.split_ranges(base_end, end, ",") {
            let mut cursor = a;
            let mut ranges = Vec::new();
            while cursor < b {
                if self.text(cursor) == "=" {
                    ranges.push((cursor + 1, b));
                    break;
                }
                if matches!(self.text(cursor), "(" | "[" | "{") {
                    let Some(close) = self.matching(cursor, b) else { break };
                    if self.text(cursor) == "[" {
                        ranges.push((cursor + 1, close));
                    }
                    if self.text(cursor) == "{" {
                        ranges.push((cursor, b));
                        break;
                    }
                    cursor = close + 1;
                } else {
                    cursor += 1;
                }
            }
            for (a, b) in ranges {
                if (a..b).any(|i| matches!(self.text(i),
                    "__builtin_va_arg" | "__builtin_offsetof" |
                    "__builtin_types_compatible_p" | "__offsetof__"))
                {
                    let expression = self.expression_range(a, b);
                    self.global_expressions.push(expression);
                }
            }
        }
    }

    fn remember_types(&mut self, start: usize, end: usize) {
        for i in start..end {
            if self.cpp && matches!(self.text(i), "struct" | "union" | "enum" | "class")
                && self
                    .tokens
                    .get(i + 1)
                    .is_some_and(|t| t.kind == TokenKind::Identifier)
            {
                self.types.insert(self.text(i + 1).to_string());
                self.explicit_types.insert(self.text(i + 1).to_string());
            }
        }
        if self.text(start) == "using" && self.text(start + 2) == "=" {
            self.types.insert(self.text(start + 1).to_string());
            self.explicit_types.insert(self.text(start + 1).to_string());
            self.type_aliases.insert(self.text(start + 1).to_string(), self.clean_type(start + 3, end));
        }
        if self.cpp && self.text(start) == "using" && self.text(start + 1) != "namespace"
            && self.text(start + 2) != "=" && end > start + 2 && self.text(end - 2) == "::" {
            let name = self.text(end - 1).to_string();
            let qualified = (start + 1..end).map(|i| self.text(i)).collect::<String>();
            self.imports.insert(name, qualified);
        }
        if self.cpp && self.text(start) == "using" && self.text(start + 1) == "namespace" {
            self.using_namespaces.push((start + 2..end).map(|i| self.text(i)).collect());
        }
        if (start..end).any(|i| self.text(i) == "typedef") {
            let base_end = self.specifier_end(start, end);
            self.remember_inferred_type(start, base_end);
            for (a, b) in self.split_ranges(base_end, end, ",") {
                if let (Some(name), _) = self.declarator_name(a, b) {
                    self.types.insert(self.text(name).to_string());
                    self.explicit_types.insert(self.text(name).to_string());
                    self.type_aliases.insert(self.text(name).to_string(), self.declarator_type(start, base_end, Some(name), b));
                }
            }
        }
    }

    fn parameters(&mut self, start: usize, end: usize) -> Vec<Parameter> {
        let mut result = Vec::new();
        for (a, b) in self.split_ranges(start, end, ",") {
            if a == b {
                continue;
            }
            if self.text(a) == "..." {
                result.push(Parameter {
                    name: "...".into(),
                    type_name: "...".into(),
                    function_pointer: false,
                    span: self.span(a, b),
                });
                continue;
            }
            let base_end = self.specifier_end(a, b);
            self.remember_inferred_type(a, base_end);
            let (name_index, suffix) = self.declarator_name(base_end, b);
            let mut name = name_index
                .map(|i| self.text(i).to_string())
                .unwrap_or_default();
            let mut type_name = if self.text(suffix) == "(" {
                // CDT's parameter type helper reads only the outer
                // declarator's pointer operators, not those nested in a
                // function-pointer declarator or its parameter list.
                let mut type_name = self.clean_type(a, base_end);
                let mut i = base_end;
                while i < b && (matches!(self.text(i), "*" | "&" | "&&") || qualifier(self.text(i))) {
                    if matches!(self.text(i), "*" | "&" | "&&") {
                        type_name.push_str(self.text(i));
                    }
                    i += 1;
                }
                type_name
            } else {
                self.declarator_type(a, base_end, name_index, b)
            };
            // Without a typedef binding, CDT's C parser interprets a bare
            // identifier parameter as an old-style parameter name.
            if !self.cpp && name_index.is_none() && b > a
                && self.tokens[b - 1].kind == TokenKind::Identifier
                && !type_word(self.text(b - 1))
                && !self.explicit_types.contains(self.text(b - 1))
                && (a..b - 1).all(|i| qualifier(self.text(i))) {
                name = self.text(b - 1).into();
                type_name = if b == a + 1 { "ANY".into() }
                    else { self.type_code(a, b - 1, true) };
            }
            result.push(Parameter {
                name,
                type_name,
                function_pointer: self.text(suffix) == "(",
                span: self.span(a, b),
            });
        }
        result
    }

    fn binding_parameter_type(&self, start: usize, end: usize) -> String {
        if self.text(start) == "..." {
            return "...".into();
        }
        let base_end = self.specifier_end(start, end);
        let (name, suffix) = self.declarator_name(base_end, end);
        let mut group = base_end;
        while group < end && (matches!(self.text(group), "*" | "&" | "&&") || qualifier(self.text(group))) {
            group += 1;
        }
        let grouped = self.text(group) == "(";
        let pointer = if grouped {
            (group + 1..name.unwrap_or(group + 1))
                .filter(|i| matches!(self.text(*i), "*" | "&" | "&&"))
                .map(|i| self.text(i)).collect::<String>()
        } else { String::new() };
        if self.text(suffix) == "(" {
            let close = self.matching(suffix, end).unwrap_or(end);
            let parameters = self.split_ranges(suffix + 1, close, ",").into_iter()
                .filter(|(a,b)| a < b).map(|(a,b)| self.binding_parameter_type(a,b))
                .filter(|t| t != "void" && t != "...").collect::<Vec<_>>().join(",");
            let return_type = format!("{}{}", self.clean_type(start, base_end),
                (base_end..group).filter(|i| matches!(self.text(*i), "*" | "&" | "&&"))
                    .map(|i| self.text(i)).collect::<String>());
            return format!("{}({})({parameters})", self.binding_type(&return_type), if pointer.is_empty() {"*"} else {&pointer});
        }
        if self.text(suffix) == "[" {
            let mut after = suffix;
            while self.text(after) == "[" {
                after = self.matching(after, end).map_or(end, |close| close + 1);
            }
            let base = self.binding_type(&self.clean_type(start, base_end));
            if grouped {
                return format!("{base}({pointer}){}", self.raw(suffix, after).replace(' ', ""));
            }
            let first_end = self.matching(suffix, end).map_or(after, |close| close + 1);
            return if first_end == after { format!("{base}*") }
                else { format!("{base}(*){}", self.raw(first_end, after).replace(' ', "")) };
        }
        self.binding_type(&self.declarator_type(start, base_end, name, end))
    }

    fn binding_type(&self, raw_type: &str) -> String {
        let mut raw = raw_type.trim().to_string();
        for _ in 0..16 {
            let split = raw.find(['*', '&', '[', '(']).unwrap_or(raw.len());
            let base = raw[..split].trim();
            let Some(alias) = self.type_aliases.get(base) else { break };
            let replaced = format!("{alias}{}", &raw[split..]);
            if replaced == raw { break; }
            raw = replaced;
        }
        let split = raw.find(['*', '&', '[', '(']).unwrap_or(raw.len());
        let mut base = raw[..split].trim().to_string();
        for keyword in ["struct ", "class ", "union ", "enum ", "const "] {
            if let Some(rest) = base.strip_prefix(keyword) { base = rest.to_string(); }
        }
        let class_type = self.cpp.then(|| self.resolve_class_name(&base)).flatten();
        if let Some(qualified) = &class_type { base.clone_from(qualified); }
        base = match base.as_str() {
            "signed" | "signed int" => "int".into(),
            "unsigned" => "unsigned int".into(),
            "short" | "signed short" | "signed short int" => "short int".into(),
            "unsigned short" => "unsigned short int".into(),
            "long" | "signed long" | "signed long int" => "long int".into(),
            "unsigned long" => "unsigned long int".into(),
            "long long" | "signed long long" | "signed long long int" => "long long int".into(),
            "unsigned long long" => "unsigned long long int".into(),
            _ => base,
        };
        if !base.split_whitespace().all(type_word) && !self.explicit_types.contains(&base)
            && !self.class_scopes.contains(&base) && !base.contains("::") {
            return "ANY".into();
        }
        let mut result = format!("{base}{}", &raw[split..]);
        if result.starts_with("unsigned ") || result.starts_with("volatile ") {
            let space = result.find(' ').unwrap();
            result = format!("{}{}", &result[..=space], result[space + 1..].replace(' ', ""));
        } else if result.contains(['*', '[']) || base.contains("::") {
            result = result.replace(' ', "");
        } else if raw[split..].contains('&') {
            result = format!("{base} {}", raw[split..].trim());
        }
        result.replace("::", ".")
    }

    fn resolve_class_name(&self, name: &str) -> Option<String> {
        let raw = name.trim_start_matches("::");
        if name.starts_with("::") {
            return self.class_scopes.contains(raw).then(|| raw.to_string());
        }
        let mut scope = self.lexical_scope.as_str();
        while !scope.is_empty() {
            let candidate = format!("{scope}::{raw}");
            if self.class_scopes.contains(&candidate) { return Some(candidate); }
            scope = scope.rsplit_once("::").map_or("", |(owner, _)| owner);
        }
        if self.class_scopes.contains(raw) { return Some(raw.to_string()); }
        let (head, suffix) = raw.split_once("::").map_or((raw, ""), |(head, tail)| (head, tail));
        if let Some(imported) = self.imports.get(head) {
            let candidate = if suffix.is_empty() { imported.clone() } else { format!("{imported}::{suffix}") };
            if self.class_scopes.contains(&candidate) { return Some(candidate); }
        }
        let candidates: HashSet<_> = self.using_namespaces.iter()
            .map(|namespace| format!("{namespace}::{raw}"))
            .filter(|candidate| self.class_scopes.contains(candidate)).collect();
        (candidates.len() == 1).then(|| candidates.into_iter().next().unwrap())
    }

    fn angle_close(&self, start: usize, end: usize) -> Option<usize> {
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

    fn split_ranges(&self, start: usize, end: usize, separator: &str) -> Vec<(usize, usize)> {
        let mut result = Vec::new();
        let (mut from, mut i) = (start, start);
        while i < end {
            if matches!(self.text(i), "(" | "[" | "{") {
                i = self.matching(i, end).map_or(i + 1, |n| n + 1);
            } else if self.text(i) == "<"
                && i > start
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

    fn skip_attribute(&self, i: usize, end: usize) -> Option<usize> {
        if matches!(
            self.text(i),
            "__attribute__" | "__attribute" | "__declspec" | "alignas" | "_Alignas"
        ) {
            return Some(if self.text(i + 1) == "(" {
                self.matching(i + 1, end).map_or(i + 1, |n| n + 1)
            } else {
                i + 1
            });
        }
        if self.text(i) == "[" && self.text(i + 1) == "[" {
            return self.matching(i, end).map(|n| n + 1);
        }
        None
    }

    fn specifier_end(&self, start: usize, end: usize) -> usize {
        let mut i = start;
        let mut has_type = false;
        while i < end {
            if let Some(after) = self.skip_attribute(i, end) {
                i = after;
                continue;
            }
            let t = self.text(i);
            if i > start && self.text(i - 1) == "extern" && self.tokens[i].kind == TokenKind::Literal {
                i += 1;
                continue;
            }
            if qualifier(t) {
                i += 1;
                continue;
            }
            if matches!(t, "struct" | "union" | "enum" | "class") {
                has_type = true;
                i += 1;
                while let Some(after) = self.skip_attribute(i, end) {
                    i = after;
                }
                if self
                    .tokens
                    .get(i)
                    .is_some_and(|t| t.kind == TokenKind::Identifier)
                {
                    i += 1;
                }
                if self.text(i) == "{" {
                    i = self.matching(i, end).map_or(i + 1, |n| n + 1);
                }
                continue;
            }
            if type_word(t) {
                has_type = true;
                i += 1;
                continue;
            }
            if matches!(
                t,
                "typeof" | "__typeof__" | "__typeof" | "decltype" | "_Atomic"
            ) && self.text(i + 1) == "("
            {
                has_type = true;
                i = self.matching(i + 1, end).map_or(i + 1, |n| n + 1);
                continue;
            }
            if !has_type && (self.tokens[i].kind == TokenKind::Identifier || t == "::") {
                has_type = true;
                i += 1;
                while i + 1 < end && self.text(i) == "::" {
                    i += 2;
                }
                if self.cpp && self.text(i) == "<" {
                    i = self.angle_close(i, end).map_or(i, |n| n + 1);
                }
                continue;
            }
            break;
        }
        i
    }

    fn declarator_name(&self, start: usize, end: usize) -> (Option<usize>, usize) {
        let mut i = start;
        while i < end {
            if let Some(after) = self.skip_attribute(i, end) {
                i = after;
                continue;
            }
            if matches!(self.text(i), "*" | "&" | "&&") || qualifier(self.text(i)) {
                i += 1;
                continue;
            }
            if self.text(i) == "(" {
                if let Some(close) = self.matching(i, end) {
                    let (name, _) = self.declarator_name(i + 1, close);
                    return (name, close + 1);
                }
                return (None, i);
            }
            if self.tokens[i].kind == TokenKind::Identifier {
                while i + 2 < end && self.text(i + 1) == "::" {
                    i += 2;
                }
                return (Some(i), i + 1);
            }
            break;
        }
        (None, i)
    }

    fn clean_type(&self, start: usize, end: usize) -> String {
        self.type_code(start, end, false)
    }

    fn type_code(&self, start: usize, end: usize, keep_qualifiers: bool) -> String {
        let mut pieces = Vec::new();
        let mut i = start;
        while i < end {
            if let Some(after) = self.skip_attribute(i, end) {
                i = after;
                continue;
            }
            let t = self.text(i);
            if i > start && self.text(i - 1) == "extern" && self.tokens[i].kind == TokenKind::Literal {
                i += 1;
                continue;
            }
            if !matches!(
                t,
                "static"
                    | "extern"
                    | "inline"
                    | "__inline"
                    | "__inline__"
                    | "register"
                    | "typedef"
                    | "constexpr"
                    | "consteval"
                    | "friend"
                    | "virtual"
                    | "__extension__"
                    | "__cdecl"
                    | "__fastcall"
                    | "__stdcall"
                    | "__thiscall"
                    | "__noreturn"
                    | "_Noreturn"
                    | "__restrict"
                    | "__restrict__"
                    | "restrict"
            ) && (keep_qualifiers || !matches!(t, "const" | "volatile" | "__volatile__")) {
                pieces.push(if t == "__volatile__" { "volatile" } else { t });
            }
            i += 1;
        }
        let mut out = String::new();
        for t in pieces {
            if !out.is_empty()
                && !matches!(t, "*" | "&" | "&&" | "[" | "]" | "::" | ">" | "," | ")")
                && !out.ends_with([':', '<', '(', '['])
            {
                out.push(' ');
            }
            out.push_str(t);
        }
        out
    }

    fn declarator_type(
        &self,
        start: usize,
        base_end: usize,
        name: Option<usize>,
        end: usize,
    ) -> String {
        let mut out = self.clean_type(start, base_end);
        let suffix_end = (base_end..end)
            .find(|i| matches!(self.text(*i), "=" | "{"))
            .unwrap_or(end);
        for i in base_end..suffix_end {
            if Some(i) != name && matches!(self.text(i), "*" | "&" | "&&") {
                out.push_str(self.text(i));
            }
        }
        let mut i = name.map_or(suffix_end, |n| n + 1);
        while i < suffix_end {
            if self.text(i) == "["
                && let Some(close) = self.matching(i, suffix_end)
            {
                out.push_str(&self.raw(i, close + 1));
                i = close + 1;
                continue;
            }
            i += 1;
        }
        if out.is_empty() { "ANY".into() } else { out }
    }

    fn declarations_range(&mut self, start: usize, end: usize) -> Vec<Declaration> {
        if start == end {
            return Vec::new();
        }
        let base_end = self.specifier_end(start, end);
        self.remember_inferred_type(start, base_end);
        let ranges = self.split_ranges(base_end, end, ",");
        if self.in_function_body && let Some(&(a, b)) = ranges.first() {
            let (name, suffix) = self.declarator_name(a, b);
            let header = self.function_header(a, b);
            let function_pointer = self.text(a) == "(" && self.text(suffix) == "("
                && (a..suffix).any(|i| matches!(self.text(i), "*" | "&" | "&&"));
            if self.text(suffix) == "(" && (header.is_some() || function_pointer) {
                // AstForStatementsCreator routes a function-shaped first
                // declarator separately and ignores every following declarator.
                if (start..base_end).any(|i| self.text(i) == "typedef") {
                    self.remember_types(start, end);
                    return Vec::new();
                }
                if let Some(mut header) = header {
                    let base_type = self.clean_type(start, base_end);
                    let pointer_type = self.clean_type(a, header.name_start);
                    header.return_type = Some(format!("{base_type}{pointer_type}"));
                    let saved_pos = self.pos;
                    let mut declarations = Vec::new();
                    self.add_function(start, b, header, "", false, &mut declarations);
                    self.function_declarations.extend(declarations);
                    self.pos = saved_pos;
                    return Vec::new();
                }
                if let Some(name) = name {
                    let type_name = self.declarator_type(start, base_end, Some(name), b);
                    let name = self.text(name).to_string();
                    self.variables.insert(name.clone());
                    self.variable_types.insert(name.clone(), type_name.clone());
                    return vec![Declaration {
                        name, type_name, initializer: None, dimensions: Vec::new(),
                        problem: self.cpp, span: self.span(a, b),
                    }];
                }
            }
        }
        let mut result = Vec::new();
        for (a, b) in ranges {
            if a == b {
                continue;
            }
            let (name_index, suffix_start) = self.declarator_name(a, b);
            let Some(name_index) = name_index else {
                // An unnamed struct/enum declaration has no executable declarator.
                if base_end < end {
                    self.diagnose(a, b, "cannot recover declaration name");
                }
                continue;
            };
            let name = self.text(name_index).to_string();
            let mut i = suffix_start;
            let mut dimension_ranges = Vec::new();
            let mut valid_dimensions = true;
            let mut initializer_start = None;
            while i < b {
                if let Some(after) = self.skip_attribute(i, b) {
                    i = after;
                    continue;
                }
                match self.text(i) {
                    "[" => {
                        if let Some(close) = self.matching(i, b) {
                            if close > i + 1 && self.text(i + 1) != "*" {
                                dimension_ranges.push((i + 1, close));
                            } else {
                                valid_dimensions = false;
                            }
                            i = close + 1;
                        } else {
                            break;
                        }
                    }
                    "=" => {
                        initializer_start = Some(i + 1);
                        break;
                    }
                    "{" => {
                        initializer_start = Some(i);
                        break;
                    }
                    "(" => {
                        let close = self.matching(i, b).unwrap_or(b.saturating_sub(1));
                        // C++ direct initialization; a pointer declarator's suffix is a type.
                        if self.cpp && a == name_index && self.function_header(a, b).is_none() {
                            initializer_start = Some(i + 1);
                        }
                        i = close + 1;
                        if initializer_start.is_some() {
                            break;
                        }
                    }
                    _ => i += 1,
                }
            }
            let dimensions = if valid_dimensions {
                dimension_ranges.into_iter().map(|(a, b)| self.expression_range(a, b)).collect()
            } else {
                Vec::new()
            };
            let initializer = initializer_start.map(|init| {
                let init_end = if init > a && self.text(init - 1) == "(" {
                    b.saturating_sub(1)
                } else {
                    b
                };
                self.expression_range(init, init_end)
            });
            let type_name = self.declarator_type(start, base_end, Some(name_index), b);
            self.variables.insert(name.clone());
            self.variable_types.insert(name.clone(), type_name.clone());
            if let Some(closure) = initializer.as_ref().and_then(|value| self.closure_expression(value)).cloned() {
                self.variable_closures.insert(name.clone(), closure);
            } else {
                self.variable_closures.remove(&name);
            }
            result.push(Declaration {
                name,
                type_name,
                initializer,
                dimensions,
                problem: false,
                span: self.span(a, b),
            });
        }
        result
    }

    fn looks_declaration(&self, start: usize, end: usize) -> bool {
        let t = self.text(start);
        if self.cpp && matches!(t, "new" | "delete" | "noexcept") {
            return false;
        }
        if (self.cpp || t != "[") && let Some(after) = self.skip_attribute(start, end) {
            return self.looks_declaration(after, end);
        }
        if t == "__extension__" {
            return self.looks_declaration(start + 1, end);
        }
        if self.cpp {
            let mut after = start + 1;
            while after + 1 < end && self.text(after) == "::" {
                after += 2;
            }
            if after > start + 1 && matches!(self.text(after), "=" | "+=" | "-=" | "*=" | "/="
                | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" | "(" | "[" | "++" | "--") {
                return false;
            }
        }
        // A typedef and an object may share a spelling after CDT recovery.
        // Assignment, member selection and subscripting still start expressions.
        if start + 1 < end && matches!(self.text(start + 1), "=" | "." | "->" | "["
            | "++" | "--" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>="
            | "+" | "-" | "/" | "%" | "==" | "!=" | "<=" | ">=" | "||" | "?") {
            return false;
        }
        if start + 1 == end && self.variables.contains(t) {
            return false;
        }
        if qualifier(t)
            || type_word(t)
            || matches!(
                t,
                "struct"
                    | "union"
                    | "enum"
                    | "class"
                    | "typeof"
                    | "__typeof__"
                    | "decltype"
                    | "using"
            )
            || self.types.contains(t)
        {
            return true;
        }
        if start < end
            && self.tokens[start].kind == TokenKind::Identifier
            && !self.variables.contains(t)
        {
            let mut i = start + 1;
            while i + 1 < end && self.text(i) == "::" {
                i += 2;
            }
            if self.cpp && self.text(i) == "<" {
                i = self.angle_close(i, end).map_or(i, |n| n + 1);
            }
            let mut pointer = i + 1;
            while let Some(after) = self.skip_attribute(pointer, end) {
                pointer = after;
            }
            let grouped_declarator = self.text(i) == "("
                && (self.text(pointer) == "*"
                    || (self.cpp && matches!(self.text(pointer), "&" | "&&")))
                && self
                    .matching(i, end)
                    .is_some_and(|close| matches!(self.text(close + 1), "(" | "["))
                && self
                    .split_ranges(i + 1, self.matching(i, end).unwrap_or(end), ",")
                    .len()
                    == 1;
            return i < end
                && (self.tokens[i].kind == TokenKind::Identifier
                    || matches!(self.text(i), "*" | "&" | "&&")
                    || grouped_declarator);
        }
        false
    }

    fn statement(&mut self) -> Stmt {
        let start = self.pos;
        let kind = match self.peek() {
            "{" | "<%" => {
                self.pos += 1;
                let old_variables = self.variables.clone();
                let old_variable_types = self.variable_types.clone();
                let old_variable_closures = self.variable_closures.clone();
                let mut statements = Vec::new();
                while self.pos < self.limit && !matches!(self.peek(), "}" | "%>") {
                    let before = self.pos;
                    statements.push(self.statement());
                    if self.pos == before {
                        self.diagnose(before, before + 1, "cannot recover statement");
                        self.pos += 1;
                    }
                }
                if !self.eat("}") {
                    self.expect("%>");
                }
                self.variables = old_variables;
                self.variable_types = old_variable_types;
                self.variable_closures = old_variable_closures;
                StmtKind::Block(statements)
            }
            ";" => {
                self.pos += 1;
                StmtKind::Empty
            }
            "[" if self.cpp && self.text(start+1)=="[" => {
                while let Some(after)=self.skip_attribute(self.pos,self.limit) {
                    self.pos=after;
                }
                return self.statement();
            }
            "if" => {
                self.pos += 1;
                self.eat("constexpr");
                let condition = self.condition();
                let consequence = Box::new(self.statement());
                let alternative = if self.eat("else") {
                    Some(Box::new(self.statement()))
                } else {
                    None
                };
                if matches!(consequence.kind,StmtKind::Problem)
                    || alternative.as_ref().is_some_and(|s| matches!(s.kind,StmtKind::Problem))
                {
                    StmtKind::Problem
                } else {
                    StmtKind::If {
                        condition,
                        consequence,
                        alternative,
                    }
                }
            }
            "while" => {
                self.pos += 1;
                let condition = self.condition();
                let body = Box::new(self.statement());
                if matches!(body.kind,StmtKind::Problem) {
                    StmtKind::Problem
                } else {
                    StmtKind::While { condition, body }
                }
            }
            "do" => {
                self.pos += 1;
                let body = Box::new(self.statement());
                if self.asm_problem_recovery && matches!(body.kind,StmtKind::Block(_)) && !self.at("while") {
                    // A brace-assembly problem prematurely closes the do
                    // body. CDT drops that incomplete do and recovers across
                    // the following statement, then resumes in its parent.
                    self.pos=self.problem_statement_end(self.pos);
                    return Stmt {kind:StmtKind::Problem,span:self.span(start,self.pos)};
                }
                if matches!(body.kind,StmtKind::Problem) && !self.at("while") {
                    return Stmt {kind:StmtKind::Problem,span:self.span(start,self.pos)};
                }
                self.expect("while");
                let condition = self.condition();
                self.expect(";");
                if matches!(body.kind,StmtKind::Problem) {
                    // After a bad scalar body CDT recovers the trailing
                    // while as a new statement, with an empty body.
                    StmtKind::While {body:Box::new(Stmt {kind:StmtKind::Empty,span:body.span.clone()}),condition}
                } else {
                    StmtKind::DoWhile { body, condition }
                }
            }
            "for" => return self.for_statement(start),
            "switch" => {
                self.pos += 1;
                let condition = self.condition();
                let body = Box::new(self.statement());
                StmtKind::Switch { condition, body }
            }
            "case" => {
                self.pos += 1;
                let expression = self.expression(0);
                self.expect(":");
                StmtKind::Case(Some(expression))
            }
            "default" => {
                self.pos += 1;
                self.expect(":");
                StmtKind::Case(None)
            }
            "goto" => {
                self.pos += 1;
                let name = if self.eat("*") {
                    let operand = self.expression(0);
                    if !matches!(operand.kind, ExprKind::Identifier(_)) {
                        self.diagnose(start, self.pos, "computed-goto operand CFG is unsupported; sanitize decompiled computed gotos");
                    }
                    "*".into()
                } else {
                    let name = self.peek().to_string();
                    if !name.is_empty() {
                        self.pos += 1;
                    }
                    name
                };
                self.expect(";");
                StmtKind::Goto(name)
            }
            "break" => {
                self.pos += 1;
                self.expect(";");
                StmtKind::Break
            }
            "continue" => {
                self.pos += 1;
                self.expect(";");
                StmtKind::Continue
            }
            "return" | "throw" if self.cpp || self.at("return") => {
                let throwing = self.at("throw");
                self.pos += 1;
                let end = self.statement_end(self.pos);
                if self.c_qualification_problem(self.pos, end) || self.c_attribute_problem(self.pos,end) {
                    self.pos = end;
                    self.finish_statement(start);
                    return Stmt {
                        kind: StmtKind::Problem,
                        span: self.span(start, self.pos),
                    };
                }
                let expression = if self.at(";") {
                    None
                } else {
                    Some(self.expression(0))
                };
                self.finish_statement(start);
                if throwing {
                    StmtKind::Throw(expression)
                } else {
                    StmtKind::Return(expression)
                }
            }
            "try" if self.cpp => {
                self.pos += 1;
                let body = Box::new(self.statement());
                let mut catches = Vec::new();
                while self.eat("catch") {
                    if self.at("(") {
                        self.pos = self
                            .matching(self.pos, self.limit)
                            .map_or(self.pos + 1, |n| n + 1);
                    }
                    catches.push(self.statement());
                }
                StmtKind::Try { body, catches }
            }
            "co_await" | "co_yield" | "co_return" if self.cpp => {
                self.pos += 1;
                let operands = if self.at(";") {
                    Vec::new()
                } else {
                    vec![self.expression(0)]
                };
                self.diagnose(start, self.pos, "coroutine control flow is unsupported");
                self.finish_statement(start);
                StmtKind::Unknown(operands)
            }
            "asm" | "__asm" | "__asm__" => {
                self.pos += 1;
                while qualifier(self.peek()) || self.at("goto") {
                    self.pos += 1;
                }
                let braced = self.at("{");
                if self.at("(") || braced {
                    if let Some(close) = self.matching(self.pos, self.limit) {
                        if braced {
                            // CDT rejects Microsoft assembly syntax. Its
                            // problem-statement recovery leaves this brace to
                            // close the current enclosing compound statement.
                            self.asm_problem_recovery = true;
                            self.pos = close;
                            return Stmt {
                                kind: StmtKind::Problem,
                                span: self.span(start, self.pos),
                            };
                        }
                        self.pos = close + 1;
                    } else {
                        self.diagnose(start, self.pos + 1, "unterminated assembly declaration");
                        self.pos += 1;
                    }
                } else {
                    self.diagnose(start, self.pos, "assembly declaration requires an operand block");
                }
                if braced {
                    self.eat(";");
                } else {
                    self.finish_statement(start);
                }
                StmtKind::Expression(Expr {
                    kind: ExprKind::Assembly(self.raw(start, self.pos)),
                    span: self.span(start, self.pos),
                })
            }
            "" => StmtKind::Empty,
            _ if self.tokens[self.pos].kind == TokenKind::Identifier
                && self.text(self.pos + 1) == ":" =>
            {
                let name = self.peek().to_string();
                self.pos += 2;
                if matches!(self.peek(), "}" | "%>" | "") {
                    // CDT represents a label without a following statement as
                    // a problem statement, which Joern omits from the AST.
                    StmtKind::Problem
                } else {
                    let statement = Box::new(self.statement());
                    if matches!(statement.kind,StmtKind::Problem) {
                        StmtKind::Problem
                    } else {
                        StmtKind::Label { name, statement }
                    }
                }
            }
            _ => {
                let end = self.statement_end(self.pos);
                if self.c_qualification_problem(self.pos, end) || self.c_attribute_problem(self.pos,end) {
                    self.pos = end;
                    self.finish_statement(start);
                    StmtKind::Problem
                } else if self.looks_declaration(self.pos, end) {
                    self.remember_types(self.pos, end);
                    let alias =
                        (self.pos..end).any(|i| matches!(self.text(i), "typedef" | "using"));
                    let declarations = if alias {
                        Vec::new()
                    } else {
                        self.declarations_range(self.pos, end)
                    };
                    self.pos = end;
                    self.expect(";");
                    if alias {
                        StmtKind::Empty
                    } else {
                        StmtKind::Declaration(declarations)
                    }
                } else {
                    let expression = self.expression(0);
                    if !self.at(";") && !matches!(self.peek(), "}" | "") {
                        let mut expressions = vec![expression];
                        self.diagnose(start, end, "unsupported expression statement syntax");
                        while self.pos < end {
                            let before = self.pos;
                            expressions.push(self.expression(0));
                            if self.pos <= before {
                                self.pos += 1;
                            }
                        }
                        self.finish_statement(start);
                        StmtKind::Unknown(expressions)
                    } else {
                        self.finish_statement(start);
                        StmtKind::Expression(expression)
                    }
                }
            }
        };
        Stmt {
            kind,
            span: self.span(start, self.pos),
        }
    }

    fn statement_end(&self, start: usize) -> usize {
        let mut i = start;
        while i < self.limit {
            if matches!(self.text(i), ";" | "}") {
                break;
            }
            if matches!(self.text(i), "(" | "[" | "{") {
                i = self.matching(i, self.limit).map_or(i + 1, |n| n + 1);
            } else {
                i += 1;
            }
        }
        i
    }

    fn finish_statement(&mut self, start: usize) {
        if !self.eat(";") {
            self.diagnose(start, self.pos, "missing semicolon after statement");
            while self.pos < self.limit && !matches!(self.peek(), ";" | "}") {
                self.pos += 1;
            }
            self.eat(";");
        }
    }

    fn condition(&mut self) -> Expr {
        self.expect("(");
        let start = self.pos;
        let end = self
            .matching(start.saturating_sub(1), self.limit)
            .unwrap_or(self.limit);
        let ranges = self.split_ranges(start, end, ";");
        let declaration_end = ranges[0].1;
        let condition_declaration = self.cpp
            && self.looks_declaration(start, declaration_end)
            && {
                let base_end = self.specifier_end(start, declaration_end);
                let (_, after_name) = self.declarator_name(base_end, declaration_end);
                matches!(self.text(after_name), "=" | "{")
            };
        let expression = if condition_declaration {
            let first = self.declarations_range(start, ranges[0].1);
            let declaration = Stmt {
                kind: StmtKind::Declaration(first),
                span: self.span(start, ranges[0].1),
            };
            if ranges.len() == 1 {
                Expr {
                    kind: ExprKind::Statement(Box::new(declaration)),
                    span: self.span(start, end),
                }
            } else {
                let value = self.expression_range(ranges[1].0, end);
                Expr {
                    kind: ExprKind::Block(vec![
                        Expr {
                            span: declaration.span.clone(),
                            kind: ExprKind::Statement(Box::new(declaration)),
                        },
                        value,
                    ]),
                    span: self.span(start, end),
                }
            }
        } else {
            self.expression_range(start, end)
        };
        self.pos = end;
        self.expect(")");
        expression
    }

    fn for_statement(&mut self, start: usize) -> Stmt {
        self.pos += 1;
        self.expect("(");
        let inside = self.pos;
        let end = self
            .matching(inside.saturating_sub(1), self.limit)
            .unwrap_or(self.limit);
        let clauses = self.split_ranges(inside, end, ";");
        if clauses.len() == 1 && self.cpp {
            let range = self.split_ranges(inside, end, ":");
            if range.len() == 2 {
                let mut declarations = self.declarations_range(range[0].0, range[0].1);
                let declaration = if declarations.is_empty() {
                    self.diagnose(inside, end, "cannot recover range-for declaration");
                    Declaration {
                        name: "<unknown>".into(),
                        type_name: "ANY".into(),
                        initializer: None,
                        dimensions: Vec::new(),
                        problem: false,
                        span: self.span(range[0].0, range[0].1),
                    }
                } else {
                    declarations.remove(0)
                };
                let iterable = self.expression_range(range[1].0, range[1].1);
                self.pos = end;
                self.expect(")");
                let body = Box::new(self.statement());
                return Stmt {
                    kind: StmtKind::RangeFor {
                        declaration,
                        iterable,
                        body,
                    },
                    span: self.span(start, self.pos),
                };
            }
        }
        if clauses.len() != 3 {
            self.diagnose(inside, end, "for header must have three clauses");
        }
        if let Some(problem) = clauses.iter().rposition(|&(a,b)| self.c_qualification_problem(a,b)) {
            // CDT's expression-error recovery crosses the invalid for header
            // up to its last problem clause. Later clauses survive as plain
            // expression statements; the for control structure and body do
            // not. Retain that recovery rather than inventing loop edges.
            let mut statements = Vec::new();
            for &(a,b) in &clauses[problem+1..] {
                if a < b {
                    statements.push(Stmt {
                        kind: StmtKind::Expression(self.expression_range(a,b)),
                        span: self.span(a,b),
                    });
                }
            }
            self.pos = self.problem_statement_end(end.saturating_add(1));
            return Stmt {
                kind: StmtKind::Sequence(statements),
                span: self.span(start,self.pos),
            };
        }
        let initializer = clauses.first().filter(|(a, b)| a < b).map(|(a, b)| {
            let kind = if self.looks_declaration(*a, *b) {
                StmtKind::Declaration(self.declarations_range(*a, *b))
            } else {
                StmtKind::Expression(self.expression_range(*a, *b))
            };
            Box::new(Stmt {
                kind,
                span: self.span(*a, *b),
            })
        });
        let condition = clauses
            .get(1)
            .filter(|(a, b)| a < b)
            .map(|(a, b)| self.expression_range(*a, *b));
        let update = clauses
            .get(2)
            .filter(|(a, b)| a < b)
            .map(|(a, b)| self.expression_range(*a, *b));
        self.pos = end;
        self.expect(")");
        let body = Box::new(self.statement());
        Stmt {
            kind: StmtKind::For {
                initializer,
                condition,
                update,
                body,
            },
            span: self.span(start, self.pos),
        }
    }

    fn expression_range(&mut self, start: usize, end: usize) -> Expr {
        if self.c_qualification_problem(start, end) {
            return Expr {
                kind: ExprKind::Problem(self.raw(start, end)),
                span: self.span(start, end),
            };
        }
        let (old_pos, old_limit) = (self.pos, self.limit);
        self.pos = start;
        self.limit = end;
        let expression = self.expression(0);
        if self.pos < end {
            self.diagnose(self.pos, end, "unparsed executable expression tokens");
        }
        self.pos = old_pos;
        self.limit = old_limit;
        expression
    }

    fn c_qualification_problem(&self, start: usize, end: usize) -> bool {
        if self.cpp {
            return false;
        }
        let mut i = start;
        while i < end {
            if self.text(i) == "::" || (self.text(i)=="_Generic" && self.text(i+1)=="(") {
                return true;
            }
            // A GNU statement-expression has its own recovery boundaries.
            // An invalid inner statement does not turn the enclosing
            // expression into a problem expression.
            if self.text(i) == "{"
                && i > start && self.text(i-1) == "("
                && let Some(close) = self.matching(i, end)
            {
                i = close + 1;
            } else {
                i += 1;
            }
        }
        false
    }

    fn c_attribute_problem(&self, start: usize, end: usize) -> bool {
        if self.cpp {return false;}
        let mut i=start;
        while i<end {
            if self.text(i)=="[" && self.text(i+1)=="[" {return true;}
            if self.text(i)=="{" && i>start && self.text(i-1)=="("
                && let Some(close)=self.matching(i,end) {
                i=close+1;
            } else {i+=1;}
        }
        false
    }

    fn problem_statement_end(&self, start: usize) -> usize {
        if start >= self.limit {
            return self.limit;
        }
        match self.text(start) {
            "{" | "<%" => self.matching(start,self.limit).map_or(self.limit,|n| n+1),
            "if" | "while" | "for" | "switch" => {
                let keyword = self.text(start);
                let open = start+1+usize::from(self.text(start+1)=="constexpr");
                let Some(close) = self.matching(open,self.limit) else {return self.limit};
                let mut end = self.problem_statement_end(close+1);
                if keyword == "if" && self.text(end)=="else" {
                    end = self.problem_statement_end(end+1);
                }
                end
            }
            "do" => {
                let body_end = self.problem_statement_end(start+1);
                if self.text(body_end)=="while" {
                    self.matching(body_end+1,self.limit).map_or(self.limit,|n| n+1+usize::from(self.text(n+1)==";"))
                } else {body_end}
            }
            _ if self.tokens[start].kind == TokenKind::Identifier && self.text(start+1)==":" => {
                self.problem_statement_end(start+2)
            }
            _ => {
                let end=self.statement_end(start);
                end+usize::from(self.text(end)==";")
            }
        }
    }

    fn expression(&mut self, minimum: u8) -> Expr {
        let start = self.pos;
        let mut left = self.prefix();
        loop {
            if self.pos >= self.limit {
                break;
            }
            let op = if self.cpp {
                alternative_operator(self.peek())
            } else {
                self.peek()
            }
            .to_string();
            // All postfix forms bind more tightly than unary/binary operators.
            if minimum <= 15 && op == "(" {
                if !self.macro_expansion
                    && let ExprKind::Identifier(name) = &left.kind
                    && matches!(name.as_str(), "__builtin_va_arg" | "__builtin_offsetof" | "__builtin_types_compatible_p" | "__offsetof__")
                    && let Some(call) = self.builtin_macro_call(name.clone(),start)
                {
                    left=call;
                    continue;
                }
                self.pos += 1;
                let mut arguments = Vec::new();
                while self.pos < self.limit && !self.at(")") {
                    let before = self.pos;
                    arguments.push(self.expression(1));
                    if !self.eat(",") || self.pos <= before {
                        break;
                    }
                }
                self.expect(")");
                if matches!(left.kind,ExprKind::Identifier(_))
                    && self.span(start,start+1).start < left.span.start
                {
                    let span=left.span.clone();
                    left=Expr {kind:ExprKind::Bracketed(Box::new(left)),span};
                }
                left = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(left),
                        arguments,
                    },
                    span: self.span(start, self.pos),
                };
                continue;
            }
            if minimum <= 15 && op == "[" {
                self.pos += 1;
                let index = self.expression(0);
                self.expect("]");
                left = Expr {
                    kind: ExprKind::Index {
                        base: Box::new(left),
                        index: Box::new(index),
                    },
                    span: self.span(start, self.pos),
                };
                continue;
            }
            if minimum <= 15 && matches!(op.as_str(), "." | "->") {
                self.pos += 1;
                if self.cpp {self.eat("template");}
                let name = self.peek().to_string();
                if !name.is_empty() {
                    self.pos += 1;
                }
                left = Expr {
                    kind: ExprKind::Member {
                        base: Box::new(left),
                        name,
                        indirect: op == "->",
                    },
                    span: self.span(start, self.pos),
                };
                continue;
            }
            if minimum <= 15 && matches!(op.as_str(), "++" | "--") {
                self.pos += 1;
                left = Expr {
                    kind: ExprKind::Unary {
                        op,
                        argument: Box::new(left),
                        postfix: true,
                    },
                    span: self.span(start, self.pos),
                };
                continue;
            }
            if op == "?" && minimum <= 2 {
                self.pos += 1;
                let consequence = if self.at(":") {
                    Expr {
                        kind: ExprKind::Unknown(String::new()),
                        span: self.span(self.pos, self.pos),
                    }
                } else {
                    self.expression(0)
                };
                self.expect(":");
                let alternative = self.expression(1);
                left = Expr {
                    kind: ExprKind::Conditional {
                        condition: Box::new(left),
                        consequence: Box::new(consequence),
                        alternative: Box::new(alternative),
                    },
                    span: self.span(start, self.pos),
                };
                continue;
            }
            let Some((precedence, right_associative)) = binary_precedence(&op) else {
                break;
            };
            if precedence < minimum {
                break;
            }
            self.pos += 1;
            let right = self.expression(if right_associative {
                precedence
            } else {
                precedence + 1
            });
            if op == "," {
                let mut values = match left.kind {
                    ExprKind::List(v) => v,
                    _ => vec![left],
                };
                values.push(right);
                left = Expr {
                    kind: ExprKind::List(values),
                    span: self.span(start, self.pos),
                };
            } else {
                left = Expr {
                    kind: ExprKind::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    span: self.span(start, self.pos),
                };
            }
        }
        left
    }

    fn builtin_macro_call(&mut self, name: String, start: usize) -> Option<Expr> {
        let close = self.matching(self.pos, self.limit)?;
        let ranges = self.split_ranges(self.pos + 1, close, ",");
        let arity = if name == "__offsetof__" { 1 } else { 2 };
        if ranges.len() != arity {
            return None;
        }
        let arguments: Vec<_> = ranges.iter().map(|&(a,b)| self.raw(a,b)).collect();
        let expansion = match name.as_str() {
            // CDT's type-id signature renders typeof's operand as 'typeof'.
            // The unparenthesized first argument preserves replacement-list
            // precedence, including a conditional or binary argument.
            "__builtin_va_arg" => format!("*(typeof *){}", arguments[0]),
            "__builtin_offsetof" if self.cpp => format!(
                "reinterpret_cast<size_t>(&reinterpret_cast<const volatile char &>(static_cast<{}*>(0)->{}))",
                arguments[0], arguments[1]),
            "__builtin_offsetof" => format!("((size_t)&(({} *)0)->{})", arguments[0], arguments[1]),
            "__builtin_types_compatible_p" => format!(
                "__builtin_types_compatible_p(sizeof({}),sizeof({}))", arguments[0], arguments[1]),
            "__offsetof__" => format!("({})", arguments[0]),
            _ => return None,
        };
        self.pos = close + 1;
        let span = self.span(start, self.pos);
        Some(self.expanded_macro(name, arity, &ranges, &expansion, span))
    }

    fn expanded_macro(&mut self, name: String, arity: usize, arguments: &[(usize,usize)], replacement: &str, span: Span) -> Expr {
        let (tokens, diagnostics) = lex_preprocessed(replacement, true);
        let limit = tokens.len();
        let mut parser = Parser {
            source: replacement, tokens, pos: 0, limit, cpp: self.cpp,
            diagnostics, types: self.types.clone(), explicit_types: self.explicit_types.clone(), type_aliases: self.type_aliases.clone(), variables: self.variables.clone(),
            variable_types: self.variable_types.clone(), variable_closures: self.variable_closures.clone(),
            function_returns: self.function_returns.clone(), function_full_name: self.function_full_name.clone(), lambda_counter: self.lambda_counter,
            imports: self.imports.clone(), using_namespaces: self.using_namespaces.clone(), lexical_scope: self.lexical_scope.clone(), extern_c: self.extern_c, extern_c_names:self.extern_c_names.clone(), class_scopes:self.class_scopes.clone(), class_fields: self.class_fields.clone(), function_fields: self.function_fields.clone(), function_member_cv: self.function_member_cv, asm_problem_recovery: false,
            in_function_body: self.in_function_body, function_declarations: Vec::new(),
            global_expressions: Vec::new(),
            // The replacement for types_compatible_p contains its own name;
            // a disabled macro is not expanded a second time by CDT.
            macro_expansion: true,
        };
        let mut expansion = parser.expression(0);
        if parser.pos < parser.limit {
            parser.diagnose(parser.pos, parser.limit, "unparsed builtin macro expansion tokens");
        }
        for diagnostic in &mut parser.diagnostics {
            diagnostic.span = span.clone();
        }
        self.diagnostics.append(&mut parser.diagnostics);
        let mut cloned = Vec::new();
        for &(a,b) in arguments {
            let code: String = (a..b).map(|i| self.text(i)).collect();
            if let Some(mut expression) = expanded_argument(&expansion, replacement, &code) {
                generated_expression(&mut expression, replacement, &span);
                cloned.push(expression);
            }
        }
        generated_expression(&mut expansion, replacement, &span);
        Expr {
            kind: ExprKind::MacroCall { name, arguments: cloned, expansion: Box::new(expansion), arity },
            span,
        }
    }

    fn closure_expression<'a>(&'a self, expression: &'a Expr) -> Option<&'a Closure> {
        closure_expression(expression, &self.variable_closures)
    }

    fn lambda_expression(&mut self) -> Expr {
        let start = self.pos;
        let Some(capture_end) = self.matching(start, self.limit) else {
            self.diagnose(start, start + 1, "unterminated lambda capture list");
            self.pos += 1;
            return Expr {kind: ExprKind::Unknown("[".into()), span: self.span(start, self.pos)};
        };
        // Captures are not expression AST children in c2cpg 4.0.150, including
        // init captures. Their body identifiers use the surrounding scope.
        self.pos = capture_end + 1;
        let inherited_bindings = self.variable_types.iter().map(|(name, ty)| (name.clone(), ty.clone())).collect();
        let inherited_closures = self.variable_closures.iter().map(|(name, closure)| (name.clone(), closure.clone())).collect();
        let name = format!("<lambda>{}", self.lambda_counter);
        self.lambda_counter += 1;
        let full_name = if self.function_full_name.is_empty() {
            name.clone()
        } else {
            format!("{}.{}", self.function_full_name, name)
        };
        let mut parameter_types = Vec::new();
        let parameters = if self.at("(") {
            let open = self.pos;
            let close = self.matching(open, self.limit).unwrap_or(self.limit);
            parameter_types = self.split_ranges(open + 1, close, ",").into_iter()
                .filter(|(a, b)| a < b)
                .map(|(a, b)| self.binding_parameter_type(a, b))
                .filter(|ty| ty != "void").collect();
            let parameters = self.parameters(open + 1, close);
            self.pos = close.saturating_add(1).min(self.limit);
            parameters
        } else {
            Vec::new()
        };
        let mut declared_return = None;
        let mut callable_return = None;
        while self.pos < self.limit && !self.at("{") {
            if let Some(after) = self.skip_attribute(self.pos, self.limit) {
                self.pos = after;
            } else if self.eat("->") {
                let return_start = self.pos;
                let specifier_end = self.specifier_end(return_start, self.limit);
                self.pos = specifier_end;
                while matches!(self.peek(), "*" | "&" | "&&") || qualifier(self.peek()) {
                    self.pos += 1;
                }
                declared_return = Some(self.clean_type(return_start, specifier_end));
                callable_return = Some(self.binding_type(&self.clean_type(return_start, self.pos)));
            } else if self.at("(") {
                self.pos = self.matching(self.pos, self.limit).map_or(self.limit, |close| close + 1);
            } else {
                self.pos += 1;
            }
        }
        if !self.at("{") {
            self.diagnose(start, self.pos, "lambda requires a compound body");
            return Expr {kind: ExprKind::Unknown(self.raw(start, self.pos)), span: self.span(start, self.pos)};
        }
        let old_variables = self.variables.clone();
        for parameter in &parameters {
            self.variables.insert(parameter.name.clone());
        }
        let body = self.statement();
        self.variables = old_variables;
        let return_type = declared_return.unwrap_or_else(|| "ANY".into());
        let signature = format!("{}({})", return_type,
            parameters.iter().map(|parameter| parameter.type_name.as_str()).collect::<Vec<_>>().join(","));
        let mut types = self.variable_types.clone();
        for (name, ty) in &self.function_fields {
            types.entry(name.clone()).or_insert_with(|| ty.clone());
        }
        for parameter in &parameters {
            types.insert(parameter.name.clone(), parameter.type_name.clone());
        }
        let mut returns = Vec::new();
        infer_return_types(&body, &mut types, &mut self.variable_closures.clone(), &self.function_returns, &mut returns);
        let call_return = callable_return.unwrap_or_else(|| {
            returns.into_iter().reduce(common_type).unwrap_or_else(|| "void".into())
        });
        let closure = Closure {full_name: full_name.clone(), return_type: self.binding_type(&call_return), parameter_types};
        self.function_declarations.push(Function {
            name, full_name, binding_return_type: self.binding_type(&return_type), return_type, signature,
            implicit_this: None, implicit_fields: self.function_fields.clone(), member_cv_qualified: self.function_member_cv,
            lambda: true, is_static: false, inherited_bindings, inherited_closures,
            parameters, body, span: self.span(start, self.pos),
        });
        Expr {kind: ExprKind::Lambda(closure), span: self.span(start, self.pos)}
    }

    fn prefix(&mut self) -> Expr {
        let start = self.pos;
        let token = if self.cpp {
            alternative_operator(self.peek())
        } else {
            self.peek()
        }
        .to_string();
        let kind = match token.as_str() {
            "true" | "false" | "nullptr" if self.cpp => {
                self.pos += 1;
                ExprKind::Literal(token)
            }
            "__null" if !self.macro_expansion => {
                self.pos+=1;
                let replacement=if self.cpp {"0"} else {"(void *)0"};
                return self.expanded_macro(token,0,&[],replacement,self.span(start,self.pos));
            }
            "__imag__" | "__real__" if !self.macro_expansion => {
                self.pos+=1;
                let operand_start=self.pos;
                self.expression(14);
                let replacement=format!("(int){}",self.raw(operand_start,self.pos));
                return self.expanded_macro(token,0,&[],&replacement,self.span(start,self.pos));
            }
            "__extension__" => {
                self.pos += 1;
                return self.expression(14);
            }
            "[" if self.cpp => return self.lambda_expression(),
            "(" => {
                self.pos += 1;
                if self.at("{") {
                    let statement = self.statement();
                    self.expect(")");
                    return Expr {
                        span: self.span(start, self.pos),
                        kind: ExprKind::Statement(Box::new(statement)),
                    };
                }
                let close = self.matching(start, self.limit);
                if close.is_some_and(|end| self.is_cast(start + 1, end)) {
                    let close = close.unwrap();
                    let type_name = if self.macro_expansion {
                        macro_type_code(&self.raw(start + 1, close))
                    } else { self.clean_type(start + 1, close) };
                    self.pos = close + 1;
                    let argument = self.expression(14);
                    ExprKind::Cast {
                        type_name,
                        argument: Box::new(argument),
                    }
                } else {
                    let inner = self.expression(0);
                    self.expect(")");
                    // CDT's bracketed-primary conversion retains only the operand AST.
                    return if self.macro_expansion || matches!(inner.kind,ExprKind::List(_)) {
                        // Expanded parent CODE still contains these brackets,
                        // although their operand has no extra CPG AST vertex.
                        let span = if matches!(inner.kind, ExprKind::List(_)) {
                            self.span(start, self.pos)
                        } else { inner.span.clone() };
                        Expr {span,kind:ExprKind::Bracketed(Box::new(inner))}
                    } else {inner};
                }
            }
            "{" => {
                self.pos += 1;
                let mut values = Vec::new();
                while self.pos < self.limit && !self.at("}") {
                    let before = self.pos;
                    let old_field_designator = self.tokens[self.pos].kind == TokenKind::Identifier
                        && self.text(self.pos + 1) == ":";
                    if self.at(".") || self.at("[") || old_field_designator {
                        let mut designators = Vec::new();
                        if old_field_designator {
                            let field = self.pos;
                            self.pos += 2;
                            designators.push(Expr {
                                kind: ExprKind::Identifier(self.text(field).into()),
                                span: self.span(field, field + 1),
                            });
                        }
                        while self.at(".") || self.at("[") {
                            if self.eat(".") {
                                let field = self.pos;
                                if self.pos < self.limit
                                    && self.tokens[self.pos].kind == TokenKind::Identifier {
                                    self.pos += 1;
                                } else {
                                    self.diagnose(field, field + 1, "field designator requires a name");
                                }
                                designators.push(Expr {
                                    kind: ExprKind::Identifier(self.raw(field, self.pos)),
                                    span: self.span(field, self.pos),
                                });
                            } else {
                                let open = self.pos;
                                self.pos += 1;
                                let index = self.expression(0);
                                self.expect("]");
                                let index = match index.kind {
                                    ExprKind::Binary { op, left, right } if op == "..." => Expr {
                                        kind: ExprKind::ArrayInitializer(vec![*left, *right]),
                                        span: self.span(open, self.pos),
                                    },
                                    _ => index,
                                };
                                designators.push(index);
                            }
                        }
                        // CDT's GNU array designator permits omission of '='.
                        self.eat("=");
                        let value = self.expression(1);
                        let assignments = designators.into_iter().map(|target| Expr {
                            kind: ExprKind::Binary {
                                op: "=".into(),
                                left: Box::new(target),
                                right: Box::new(value.clone()),
                            },
                            span: self.span(before, self.pos),
                        }).collect();
                        values.push(Expr {
                            kind: ExprKind::Block(assignments),
                            span: self.span(before, self.pos),
                        });
                    } else {
                        values.push(self.expression(1));
                    }
                    if self.pos <= before {
                        self.pos += 1;
                    }
                    if !self.eat(",") {
                        break;
                    }
                }
                self.expect("}");
                ExprKind::ArrayInitializer(values)
            }
            "++" | "--" | "+" | "-" | "*" | "&" | "!" | "~" | "&&" | "sizeof" | "alignof"
            | "_Alignof" | "__alignof__" | "__alignof" | "typeof" | "__typeof__" | "noexcept"
            | "delete" | "throw" if self.cpp || !matches!(token.as_str(),"delete"|"throw"|"noexcept") => {
                self.pos += 1;
                if token == "delete" && self.eat("[") {
                    self.expect("]");
                }
                let argument = if self.at("(")
                    && self
                        .matching(self.pos, self.limit)
                        .is_some_and(|end| self.is_type_range(self.pos + 1, end))
                    && matches!(
                        token.as_str(),
                        "sizeof"
                            | "alignof"
                            | "_Alignof"
                            | "__alignof__"
                            | "__alignof"
                            | "typeof"
                            | "__typeof__"
                    ) {
                    let open = self.pos;
                    let end = self.matching(open, self.limit).unwrap();
                    // astForTypeIdExpression converts only the declaration
                    // specifier: an abstract pointer is not an operand, and a
                    // typeof's internal expression is not evaluated here.
                    let specifier = self.specifier_end(open + 1, end);
                    let typeof_specifier = matches!(self.text(open+1),"typeof"|"__typeof__"|"__typeof"|"decltype");
                    let argument_end = if typeof_specifier { open + 2 } else { specifier };
                    let type_name = self.clean_type(open + 1, argument_end);
                    let type_name = type_name.strip_prefix("struct ")
                        .or_else(|| type_name.strip_prefix("union "))
                        .or_else(|| type_name.strip_prefix("enum "))
                        .unwrap_or(&type_name).to_string();
                    let argument = Expr {
                        kind: ExprKind::TypeSpecifier {
                            code: self.raw(open + 1, argument_end),
                            type_name,
                        },
                        span: self.span(open + 1, argument_end),
                    };
                    self.pos = end + 1;
                    argument
                } else {
                    self.expression(14)
                };
                ExprKind::Unary {
                    op: token,
                    argument: Box::new(argument),
                    postfix: false,
                }
            }
            "static_cast" | "reinterpret_cast" | "const_cast" | "dynamic_cast"
                if self.cpp && self.text(start + 1) == "<" =>
            {
                let close = self.angle_close(start + 1, self.limit).unwrap_or(start + 1);
                let type_name = if self.macro_expansion {
                    macro_type_code(&self.raw(start + 2, close))
                } else {self.clean_type(start + 2, close)};
                self.pos = close + 1;
                self.expect("(");
                let argument = self.expression(0);
                self.expect(")");
                ExprKind::Cast {
                    type_name,
                    argument: Box::new(argument),
                }
            }
            "new" if self.cpp => {
                self.pos += 1;
                let type_start = self.pos;
                self.pos = self.specifier_end(self.pos, self.limit);
                let type_end = self.pos;
                while self.eat("*") {}
                let type_name = self.clean_type(type_start, type_end).replace("::", ".");
                let mut arguments = vec![Expr {
                    kind: ExprKind::TypeSpecifier {
                        code: self.raw(type_start, type_end),
                        type_name,
                    },
                    span: self.span(type_start, type_end),
                }];
                while self.eat("[") {
                    arguments.push(self.expression(0));
                    self.expect("]");
                }
                if self.eat("(") {
                    while self.pos < self.limit && !self.at(")") {
                        arguments.push(self.expression(1));
                        if !self.eat(",") {
                            break;
                        }
                    }
                    self.expect(")");
                } else if self.at("{") {
                    arguments.push(self.prefix());
                }
                ExprKind::Call {
                    callee: Box::new(Expr {
                        kind: ExprKind::Identifier("<operator>.new".into()),
                        span: self.span(start, start + 1),
                    }),
                    arguments,
                }
            }
            "::" if self.tokens.get(start + 1).is_some_and(|t| t.kind == TokenKind::Identifier) => {
                self.pos += 2;
                while self.eat("::") {
                    self.eat("template");
                    if self.pos < self.limit {
                        self.pos += 1;
                    }
                }
                ExprKind::Identifier(self.raw(start, self.pos))
            }
            "" | ")" | "]" | "}" | ";" | ":" => {
                self.diagnose(start, start, "missing expression");
                ExprKind::Unknown(String::new())
            }
            _ => {
                self.pos += 1;
                match self.tokens[start].kind {
                    TokenKind::Literal => {
                        // Adjacent string literals are one CDT literal expression.
                        while self.pos < self.limit
                            && self.tokens[self.pos].kind == TokenKind::Literal
                            && self.text(self.pos).contains('"')
                            && token.contains('"')
                        {
                            self.pos += 1;
                        }
                        ExprKind::Literal(self.raw(start, self.pos))
                    }
                    TokenKind::Identifier => {
                        while self.eat("::") {
                            self.eat("template");
                            if self.pos < self.limit {
                                self.pos += 1;
                            }
                        }
                        // Template calls: include type arguments in the callee name.
                        if self.cpp
                            && self.at("<")
                            && let Some(close) = self.angle_close(self.pos, self.limit)
                            && (self.text(close + 1) == "(" || self.text(close + 1) == "::")
                        {
                            self.pos = close + 1;
                        }
                        ExprKind::Identifier(self.raw(start, self.pos))
                    }
                    TokenKind::Punctuation => {
                        self.diagnose(
                            start,
                            self.pos,
                            format!("unsupported expression token '{token}'"),
                        );
                        ExprKind::Unknown(token)
                    }
                }
            }
        };
        Expr {
            kind,
            span: self.span(start, self.pos),
        }
    }

    fn can_start_expression(&self, i: usize) -> bool {
        if i >= self.limit {
            return false;
        }
        self.tokens[i].kind != TokenKind::Punctuation
            || matches!(
                self.text(i),
                "(" | "{" | "::" | "++" | "--" | "+" | "-" | "*" | "&" | "!" | "~"
            )
            || self.cpp && self.text(i) == "["
    }

    fn is_cast(&self, start: usize, end: usize) -> bool {
        if !self.cpp && self.variables.contains(self.text(start)) {
            // An ordinary C object binding shadows a typedef in this scope.
            return false;
        }
        if !self.is_type_range(start, end) || !self.can_start_expression(end + 1) {
            return false;
        }
        let first = self.text(start);
        if type_word(first)
            || qualifier(first)
            || self.explicit_types.contains(first)
            || matches!(first, "struct" | "union" | "enum" | "class")
            || (start..end).any(|i| self.text(i) == "*")
        {
            return true;
        }
        // An absent typedef is common after DecBench removes system headers.
        // '+'/'-'/'*'/'&' after an unknown bracketed identifier are ambiguous
        // binary expressions; retain that interpretation unless it is a type.
        let next = self.text(end + 1);
        if next == "("
            && let Some(close) = self.matching(end + 1, self.limit)
            && self.is_cast(end + 2, close)
        {
            // A following cast resolves the otherwise ambiguous '(T)(...)'
            // form: its type-id is not a valid call argument expression.
            return true;
        }
        self.tokens[end + 1].kind != TokenKind::Punctuation
            || matches!(next, "{" | "!" | "~" | "++" | "--")
    }

    fn is_type_range(&self, start: usize, end: usize) -> bool {
        if start == end {
            return false;
        }
        let first = self.text(start);
        // A type-id begins with a declaration specifier, never with an
        // expression's dereference or parenthesized operand. In particular,
        // '(*(code *)(p))()' must retain its pointer-call receiver.
        if self.tokens[start].kind != TokenKind::Identifier && first != "::" {
            return false;
        }
        let known = type_word(first)
            || qualifier(first)
            || self.types.contains(first)
            || matches!(
                first,
                "struct" | "union" | "enum" | "class" | "typeof" | "__typeof__" | "decltype"
            );
        if !known && self.variables.contains(first) {
            return false;
        }
        let pointer = (start..end)
            .any(|i| self.text(i) == "*" || (self.cpp && matches!(self.text(i), "&" | "&&")));
        if !known && !pointer {
            return end == start + 1
                && self.tokens[start].kind == TokenKind::Identifier
                && !self.variables.contains(first);
        }
        // An abstract declarator follows the type specifiers. Logical && is
        // not a pointer; treating it as one would erase sizeof's expression
        // operands (CDT/Joern includes those operands in its CFG).
        let base_end = self.specifier_end(start, end);
        let mut i = base_end;
        while i < end {
            if let Some(after) = self.skip_attribute(i, end) {
                i = after;
                continue;
            }
            match self.text(i) {
                "*" => i += 1,
                "&" | "&&" if self.cpp => i += 1,
                "(" | "[" => {
                    let Some(close) = self.matching(i, end) else {
                        return false;
                    };
                    if self.text(i) == "("
                        && i == base_end
                        && !known
                        && !(i + 1..close).any(|n| self.text(n) == "*")
                    {
                        return false;
                    }
                    i = close + 1;
                }
                t if qualifier(t) => i += 1,
                _ => return false,
            }
        }
        true
    }

    fn remember_inferred_type(&mut self, start: usize, end: usize) {
        let mut i = start;
        while i < end {
            if let Some(after) = self.skip_attribute(i, end) {
                i = after;
                continue;
            }
            if !self.cpp && matches!(self.text(i), "struct" | "union" | "enum") {
                i += 2;
                continue;
            }
            if self.tokens[i].kind == TokenKind::Identifier
                && !type_word(self.text(i))
                && !qualifier(self.text(i))
                && !matches!(
                    self.text(i),
                    "struct" | "class" | "union" | "enum" | "typeof" | "__typeof__" | "decltype"
                )
            {
                self.types.insert(self.text(i).to_string());
                break;
            }
            i += 1;
        }
    }
}

fn closure_expression<'a>(expression: &'a Expr, closures: &'a HashMap<String, Closure>) -> Option<&'a Closure> {
    match &expression.kind {
        ExprKind::Lambda(closure) => Some(closure),
        ExprKind::Identifier(name) => closures.get(name),
        ExprKind::Bracketed(inner) | ExprKind::Generated {expression: inner, ..} => closure_expression(inner, closures),
        ExprKind::Unary {op, argument, ..} if matches!(op.as_str(), "*" | "&") => closure_expression(argument, closures),
        _ => None,
    }
}

fn common_type(left: String, right: String) -> String {
    if left == right { return left; }
    if left == "ANY" || right == "ANY" { return "ANY".into(); }
    for ty in ["long double", "double", "float", "unsigned long long", "long long", "unsigned long", "long", "unsigned int", "int"] {
        if left == ty || right == ty { return ty.into(); }
    }
    left
}

fn infer_value_type(expression: &Expr, types: &HashMap<String, String>, closures: &HashMap<String, Closure>, functions: &HashMap<String, String>) -> String {
    match &expression.kind {
        ExprKind::Identifier(name) => types.get(name).map(|ty| ty.trim_end_matches('&').to_string()).unwrap_or_else(|| "ANY".into()),
        ExprKind::Literal(value) => {
            if matches!(value.as_str(), "true" | "false") { "bool".into() }
            else if value.starts_with('"') { "char*".into() }
            else if value.starts_with('\'') { "char".into() }
            else if value.contains('.') || (!value.starts_with("0x") && value.to_lowercase().contains('e')) {
                if value.ends_with(['f', 'F']) { "float".into() } else { "double".into() }
            } else if value.ends_with(['l', 'L']) { "long".into() }
            else if value.ends_with(['u', 'U']) { "unsigned int".into() }
            else { "int".into() }
        }
        ExprKind::Cast {type_name, ..} => type_name.clone(),
        ExprKind::Bracketed(inner) | ExprKind::Generated {expression: inner, ..} => infer_value_type(inner, types, closures, functions),
        ExprKind::Unary {op, argument, ..} => {
            let ty = infer_value_type(argument, types, closures, functions);
            match op.as_str() {
                "!" => "bool".into(),
                "*" => ty.strip_suffix('*').unwrap_or("ANY").into(),
                "&" => format!("{ty}*"),
                "sizeof" | "alignof" => "unsigned long".into(),
                _ => ty,
            }
        }
        ExprKind::Binary {op, left, right} => {
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
        ExprKind::Conditional {consequence, alternative, ..} => common_type(infer_value_type(consequence, types, closures, functions), infer_value_type(alternative, types, closures, functions)),
        ExprKind::Call {callee, ..} => {
            if let Some(closure) = closure_expression(callee, closures) { closure.return_type.clone() }
            else if let ExprKind::Identifier(name) = &callee.kind { functions.get(name.rsplit("::").next().unwrap()).cloned().unwrap_or_else(|| "ANY".into()) }
            else { "ANY".into() }
        }
        ExprKind::Index {base, ..} => {
            let ty = infer_value_type(base, types, closures, functions);
            ty.strip_suffix('*').or_else(|| ty.split_once('[').map(|(ty, _)| ty)).unwrap_or("ANY").into()
        }
        ExprKind::List(expressions) | ExprKind::Block(expressions) => expressions.last().map(|value| infer_value_type(value, types, closures, functions)).unwrap_or_else(|| "void".into()),
        _ => "ANY".into(),
    }
}

fn infer_return_types(statement: &Stmt, types: &mut HashMap<String, String>, closures: &mut HashMap<String, Closure>, functions: &HashMap<String, String>, returns: &mut Vec<String>) {
    match &statement.kind {
        StmtKind::Block(statements) => {
            let mut types = types.clone();
            let mut closures = closures.clone();
            for statement in statements { infer_return_types(statement, &mut types, &mut closures, functions, returns); }
        }
        StmtKind::Sequence(statements) => for statement in statements { infer_return_types(statement, types, closures, functions, returns); },
        StmtKind::Declaration(declarations) => for declaration in declarations {
            let ty = if declaration.type_name.starts_with("auto") {
                declaration.initializer.as_ref().map(|value| infer_value_type(value, types, closures, functions)).unwrap_or_else(|| "ANY".into())
            } else { declaration.type_name.clone() };
            if let Some(closure) = declaration.initializer.as_ref().and_then(|value| closure_expression(value, closures)).cloned() {
                closures.insert(declaration.name.clone(), closure);
            }
            types.insert(declaration.name.clone(), ty);
        },
        StmtKind::Return(value) => returns.push(value.as_ref().map(|value| infer_value_type(value, types, closures, functions)).unwrap_or_else(|| "void".into())),
        StmtKind::If {consequence, alternative, ..} => {
            infer_return_types(consequence, &mut types.clone(), &mut closures.clone(), functions, returns);
            if let Some(alternative) = alternative { infer_return_types(alternative, &mut types.clone(), &mut closures.clone(), functions, returns); }
        }
        StmtKind::While {body, ..} | StmtKind::DoWhile {body, ..} | StmtKind::Switch {body, ..} | StmtKind::For {body, ..} | StmtKind::RangeFor {body, ..} | StmtKind::Label {statement: body, ..} => infer_return_types(body, &mut types.clone(), &mut closures.clone(), functions, returns),
        StmtKind::Try {body, catches} => {
            infer_return_types(body, &mut types.clone(), &mut closures.clone(), functions, returns);
            for catch in catches { infer_return_types(catch, &mut types.clone(), &mut closures.clone(), functions, returns); }
        }
        _ => (),
    }
}

// CDT uses a formatted AST signature for expanded syntax, rather than the
// original macro call's source. This also governs which original arguments
// MacroHandler can find and clone in the expansion AST.
fn expanded_code(expression: &Expr, source: &str) -> String {
    let code = |e: &Expr| if let ExprKind::Bracketed(inner)=&e.kind {
        format!("({})",expanded_code(inner,source))
    } else {expanded_code(e, source)};
    match &expression.kind {
        ExprKind::Identifier(name) | ExprKind::Literal(name) => name.clone(),
        ExprKind::TypeSpecifier { code, .. } => code.clone(),
        ExprKind::Binary {op,left,right} => format!("{} {op} {}",code(left),code(right)),
        ExprKind::Conditional {condition,consequence,alternative} => format!("{} ? {} : {}",code(condition),code(consequence),code(alternative)),
        ExprKind::Cast {type_name,argument} => {
            let raw = &source[expression.span.start..expression.span.end];
            let cast = ["static_cast", "reinterpret_cast", "const_cast", "dynamic_cast"]
                .into_iter().find(|cast| raw.starts_with(cast));
            if let Some(cast) = cast {
                format!("{cast}<{}>({})", macro_type_code(type_name), code(argument))
            } else {
                format!("({}){}", macro_type_code(type_name), code(argument))
            }
        }
        ExprKind::Unary {op,argument,postfix} => {
            if *postfix {format!("{}{op}",code(argument))}
            else if matches!(op.as_str(),"sizeof"|"alignof"|"_Alignof"|"__alignof__"|"__alignof"|"typeof"|"__typeof__") {
                let raw=&source[expression.span.start..expression.span.end];
                // ASTSignatureUtil supplies the parentheses for a type-id;
                // an expression operand already retains its bracketed syntax.
                if !matches!(argument.kind, ExprKind::TypeSpecifier { .. }) {
                    return format!("{op} {}", code(argument));
                }
                let value = if matches!(&argument.kind,ExprKind::TypeSpecifier {code,..} if code=="typeof"||code=="__typeof__"||code=="decltype") {
                    code(argument)
                } else if let Some(inner)=raw.strip_prefix(op).and_then(|s| s.trim().strip_prefix('(')).and_then(|s| s.strip_suffix(')')) {
                    macro_type_code(inner)
                } else {code(argument)};
                format!("{op} ({value})")
            } else {format!("{op}{}",code(argument))}
        }
        ExprKind::Call {callee,arguments} => format!("{}({})",code(callee),arguments.iter().map(code).collect::<Vec<_>>().join(", ")),
        ExprKind::Bracketed(inner) => expanded_code(inner,source),
        ExprKind::Generated {code,..} => code.clone(),
        ExprKind::Member {base,name,indirect} => format!("{}{}{name}",code(base),if *indirect {"->"}else{"."}),
        ExprKind::Index {base,index} => format!("{}[{}]",code(base),code(index)),
        ExprKind::List(values)|ExprKind::Block(values) => values.iter().map(code).collect::<Vec<_>>().join(", "),
        ExprKind::ArrayInitializer(values) => format!("{{{}}}",values.iter().map(code).collect::<Vec<_>>().join(", ")),
        _ => source[expression.span.start..expression.span.end].into(),
    }
}

fn macro_type_code(type_name: &str) -> String {
    let type_name=type_name.trim();
    let type_name=type_name.strip_prefix("struct ").or_else(||type_name.strip_prefix("union ")).or_else(||type_name.strip_prefix("enum ")).unwrap_or(type_name);
    type_name.replace('*'," *").replace('&'," &").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn expanded_argument(expression: &Expr, source: &str, argument: &str) -> Option<Expr> {
    if expanded_code(expression,source)==argument {
        return Some(expression.clone());
    }
    let children: Vec<&Expr> = match &expression.kind {
        ExprKind::Unary {argument,..}|ExprKind::Cast {argument,..}|ExprKind::Bracketed(argument)|ExprKind::Generated {expression:argument,..} => vec![argument],
        ExprKind::Binary {left,right,..} => vec![left,right],
        ExprKind::Conditional {condition,consequence,alternative} => vec![condition,consequence,alternative],
        // Static call names and FIELD_IDENTIFIERs have no expression AST.
        ExprKind::Call {arguments,..} => arguments.iter().collect(),
        ExprKind::Member {base,..} => vec![base],
        ExprKind::Index {base,index} => vec![base,index],
        ExprKind::List(values)|ExprKind::Block(values)|ExprKind::ArrayInitializer(values) => values.iter().collect(),
        _ => Vec::new(),
    };
    children.into_iter().find_map(|child| expanded_argument(child,source,argument))
}

fn generated_expression(expression: &mut Expr, source: &str, span: &Span) {
    let code=expanded_code(expression,source);
    match &mut expression.kind {
        ExprKind::Unary {argument,..}|ExprKind::Bracketed(argument)|ExprKind::Generated {expression:argument,..} => generated_expression(argument,source,span),
        ExprKind::Cast {argument,type_name} => {
            *type_name=macro_type_code(type_name);
            generated_expression(argument,source,span);
        }
        ExprKind::Binary {left,right,..} => {generated_expression(left,source,span);generated_expression(right,source,span);}
        ExprKind::Conditional {condition,consequence,alternative} => {generated_expression(condition,source,span);generated_expression(consequence,source,span);generated_expression(alternative,source,span);}
        ExprKind::Call {callee,arguments} => {
            if matches!(callee.kind,ExprKind::Identifier(_)) {callee.span=span.clone();}
            else {generated_expression(callee,source,span);}
            for argument in arguments {generated_expression(argument,source,span);}
        }
        ExprKind::MacroCall {arguments,expansion,..} => {for argument in arguments {generated_expression(argument,source,span);}generated_expression(expansion,source,span);}
        ExprKind::Member {base,..} => generated_expression(base,source,span),
        ExprKind::Index {base,index} => {generated_expression(base,source,span);generated_expression(index,source,span);}
        ExprKind::List(values)|ExprKind::Block(values)|ExprKind::ArrayInitializer(values) => for value in values {generated_expression(value,source,span);},
        ExprKind::Statement(statement) => generated_statement(statement,source,span),
        _ => (),
    }
    let kind=std::mem::replace(&mut expression.kind,ExprKind::Unknown(String::new()));
    expression.kind=ExprKind::Generated {expression:Box::new(Expr {kind,span:span.clone()}),code};
    expression.span=span.clone();
}

fn generated_statement(statement: &mut Stmt, source: &str, span: &Span) {
    match &mut statement.kind {
        StmtKind::Block(statements)|StmtKind::Sequence(statements) => for statement in statements {generated_statement(statement,source,span);},
        StmtKind::Expression(expression) => generated_expression(expression,source,span),
        StmtKind::Declaration(declarations) => for declaration in declarations {
            declaration.span=span.clone();
            if let Some(expression)=&mut declaration.initializer {generated_expression(expression,source,span);}
            for expression in &mut declaration.dimensions {generated_expression(expression,source,span);}
        },
        StmtKind::If {condition,consequence,alternative} => {generated_expression(condition,source,span);generated_statement(consequence,source,span);if let Some(statement)=alternative {generated_statement(statement,source,span);}},
        StmtKind::While {condition,body}|StmtKind::Switch {condition,body}|StmtKind::DoWhile {condition,body} => {generated_expression(condition,source,span);generated_statement(body,source,span);},
        StmtKind::For {initializer,condition,update,body} => {
            if let Some(statement)=initializer {generated_statement(statement,source,span);}
            for expression in [condition,update].into_iter().flatten() {generated_expression(expression,source,span);}
            generated_statement(body,source,span);
        },
        StmtKind::RangeFor {declaration,iterable,body} => {declaration.span=span.clone();generated_expression(iterable,source,span);generated_statement(body,source,span);},
        StmtKind::Case(expression)|StmtKind::Return(expression)|StmtKind::Throw(expression) => if let Some(expression)=expression {generated_expression(expression,source,span);},
        StmtKind::Label {statement,..} => generated_statement(statement,source,span),
        StmtKind::Try {body,catches} => {generated_statement(body,source,span);for statement in catches {generated_statement(statement,source,span);}},
        StmtKind::Unknown(expressions) => for expression in expressions {generated_expression(expression,source,span);},
        _ => (),
    }
    statement.span=span.clone();
}

fn qualifier(t: &str) -> bool {
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
            | "__noreturn"
            | "_Noreturn"
    )
}

fn type_word(t: &str) -> bool {
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

fn binary_precedence(op: &str) -> Option<(u8, bool)> {
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
fn primitive_return_display(code: &str) -> String {
    let split = code.find(['*', '&', '[']).unwrap_or(code.len());
    let words: Vec<_> = code[..split].split_whitespace().collect();
    let modifiers = ["const", "volatile", "signed", "unsigned"];
    let mut base: Vec<_> = modifiers
        .iter()
        .copied()
        .filter(|modifier| words.contains(modifier))
        .collect();
    base.extend(words.into_iter().filter(|word| !modifiers.contains(word)));
    format!("{}{}", base.join(" "), &code[split..]).replace("_Bool", "bool")
}

fn simple_type_name(name: &str) -> &str {
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

fn alternative_operator(op: &str) -> &str {
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

#[cfg(test)]
mod declaration_tests {
    use super::*;

    #[test]
    fn const_first_knr_parameters_keep_all_function_bodies() {
        let unit = parse("struct global { int p; int x; }; int first(p) const char *p; { if (*p) return 1; return 0; } int second(x) int x; { return x; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["first", "second"]);
        assert_eq!(unit.functions[0].parameters[0].name, "p");
        assert_eq!(unit.functions[0].parameters[0].type_name, "char*");
        assert!(matches!(unit.functions[0].body.kind, StmtKind::Block(_)));
    }

    #[test]
    fn object_initializers_and_array_bounds_do_not_hide_following_functions() {
        let unit = parse("struct item { int value; }; static struct item configuration = { 0 }; static char options[2 * sizeof(configuration)]; int after(int x) { if (x) return 1; return 0; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.len(), 1);
        assert_eq!(unit.functions[0].name, "after");
    }

    #[test]
    fn anonymous_record_initializer_and_enum_alias_preserve_later_bodies() {
        let unit = parse("static const struct { int x; } pairs[] = { { 1 }, { 2 } }; typedef enum { first, second } Choice; int f(Choice x) { if (x) return 1; return 0; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.len(), 1);
        assert_eq!(unit.functions[0].name, "f");
        assert_eq!(unit.functions[0].parameters[0].type_name, "Choice");
    }

    #[test]
    fn typedef_function_parameters_are_types_without_executable_expressions() {
        let unit = parse("typedef long read_fn(void *cookie, char *buf, unsigned long count); typedef void (*callback_fn)(int); int f(read_fn *reader, callback_fn callback) { callback(1); return 0; }", true);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.len(), 1);
        assert_eq!(unit.functions[0].parameters[0].type_name, "read_fn*");
    }

    #[test]
    fn elaborated_return_declarations_follow_cdt_type_declaration_conversion() {
        let unit = parse("struct item; struct item *declaration(void); struct item *definition(void) { return 0; } int ordinary(void); int ordinary(void) { return 1; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["definition", "ordinary"]);
    }

    #[test]
    fn operator_definitions_replace_matching_declaration_stubs() {
        let unit = parse(include_str!("../tests/fixtures/decbench-regressions/cpp_operator_definitions.cpp"), true);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["==", "!=", "="]);
        let declarations = parse(include_str!("../tests/fixtures/decbench-regressions/cpp_operator_declarations.cpp"), true);
        assert_eq!(declarations.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["operator ==", "operator !=", "operator ="]);
    }

    #[test]
    fn inferred_typenames_do_not_turn_object_assignment_into_declarations() {
        let unit = parse("typedef int array; int f(void) { array = 1; array[0] = 2; array.member = 3; return 0; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else { panic!() };
        assert!(body[..3].iter().all(|s| matches!(s.kind, StmtKind::Expression(_))));
    }

    #[test]
    fn named_type_function_pointer_fields_are_objects() {
        let unit = parse("typedef long size; struct callbacks { size (*read)(int); size (*write)(int); }; size (*global_callback)(int); size f(int x) { return x; }", true);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.len(), 1);
        assert_eq!(unit.functions[0].name, "f");
    }

    #[test]
    fn using_declaration_resolves_method_owner_and_removes_stub() {
        let unit = parse("namespace n { struct A { int f(); }; } using n::A; int A::f() { return 0; }", true);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.len(), 1);
        assert_eq!(unit.functions[0].full_name, "n.A.f:int()");
    }

    #[test]
    fn cpp_method_names_and_signatures_match_original_joern() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/ddg-lambdas/method-names.json"
        )).unwrap();
        assert_eq!(reference["generator"], "Original Joern 4.0.150 / c2cpg");
        for (filename, source) in [
            ("using.cpp", include_str!("../tests/fixtures/ddg-lambdas/using.cpp")),
            ("namespaced.cpp", include_str!("../tests/fixtures/ddg-lambdas/namespaced.cpp")),
        ] {
            let unit = parse(source, true);
            assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
            let methods = reference["methods"].as_array().unwrap().iter()
                .filter(|method| method["filename"] == filename).collect::<Vec<_>>();
            assert_eq!(unit.functions.len(), methods.len());
            for function in &unit.functions {
                let original = methods.iter().find(|method| method["name"] == function.name).unwrap();
                assert_eq!(function.full_name, original["fullname"].as_str().unwrap());
                assert_eq!(function.signature, original["signature"].as_str().unwrap());
                assert_eq!(function.return_type, original["return_type"].as_str().unwrap());
            }
        }
    }

    #[test]
    fn cpp_declaration_iteration_matches_original_constructor_selection() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/ddg-classes/joern-4.0.150.json"
        )).unwrap();
        let unit = parse(include_str!("../tests/fixtures/ddg-classes/constructors.cpp"), true);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let expected: Vec<_> = reference["methods"].as_array().unwrap().iter()
            .map(|method| method["fullname"].as_str().unwrap()).collect();
        assert_eq!(unit.functions.iter().map(|function| function.full_name.as_str()).collect::<Vec<_>>(), expected);
        // Every reduced constructor has one CFG node, so PyJoern keeps the
        // last declaration on ties, including Plain's default overload.
        for name in ["type_info", "Plain"] {
            let selected = unit.functions.iter().rev().find(|function| function.name == name).unwrap();
            let original = reference["methods"].as_array().unwrap().iter()
                .find(|method| method["name"] == name && method["selected_by_from_many"] == true).unwrap();
            assert_eq!(selected.full_name, original["fullname"].as_str().unwrap());
        }
    }

    #[test]
    fn c_and_cpp_declaration_orders_match_original_joern() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/ddg-classes/method-order-oracle.json"
        )).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let unit = parse(case["source"].as_str().unwrap(), case["language"] == "cpp");
            assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
            let original = case["ordered_methods"].as_array().unwrap();
            assert_eq!(unit.functions.len(), original.len());
            for (function, method) in unit.functions.iter().zip(original) {
                assert_eq!(function.name, method["name"].as_str().unwrap());
                assert_eq!(function.full_name, method["fullname"].as_str().unwrap());
                assert_eq!(function.signature, method["signature"].as_str().unwrap());
                assert_eq!(function.return_type, method["return_type"].as_str().unwrap());
            }
        }
    }

    #[test]
    fn c_symbol_definition_replaces_array_parameter_declaration() {
        let unit = parse("int f(int values[]); int f(int *values) { return values[0]; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.len(), 1);
        assert!(matches!(unit.functions[0].body.kind, StmtKind::Block(_)));
    }

    #[test]
    fn file_scope_assembly_preserves_following_method_and_asm_symbol_alias() {
        let unit = parse("__asm__(\".section .text\"); __asm { nop } int declared(void) __asm__(\"symbol\"); int f(void) { return 0; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["f", "declared"]);
    }

    #[test]
    fn cdt_problem_attributes_recover_later_methods_with_language_specific_rules() {
        let source = "int empty(int x) __attribute__(()); int empty_body(int x) __attribute__(()) { return x; } [[__nodiscard__]] int cpp_only([[__maybe_unused__]] int x) { return x; } int valid(int x) __attribute__((unused)); int after(int x) { return x; }";
        for cpp in [false, true] {
            let unit = parse(source, cpp);
            assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
            let names = unit.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>();
            assert_eq!(names, if cpp { vec!["cpp_only", "after", "valid"] } else { vec!["after", "valid"] });
        }
    }

    #[test]
    fn cdt_noreturn_problem_recovery_depends_on_declaration_order() {
        let source = "_Noreturn void prefix(int x) { return; } static _Noreturn void after_static(int x) { return; } void _Noreturn after_type(int x) { return; } __attribute__((noreturn)) void gnu(int x) { return; } int after(void) { return 0; }";
        for cpp in [false, true] {
            let unit = parse(source, cpp);
            assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
            assert_eq!(unit.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["prefix", "gnu", "after"]);
        }
    }

    #[test]
    fn attributes_before_local_declaration_keep_initializers() {
        for source in ["int f(int x) { __attribute__((unused)) int value = x + 1; return value; }", "int f(int x) { [[__maybe_unused__]] int value = x + 1; return value; }"] {
            let unit = parse(source, true);
            assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
            let StmtKind::Block(body) = &unit.functions[0].body.kind else { panic!() };
            assert!(matches!(&body[0].kind, StmtKind::Declaration(values) if values[0].initializer.is_some()));
        }
    }

    #[test]
    fn c_record_tags_do_not_classify_function_calls_as_declarations() {
        let unit = parse("struct sigaction { int value; }; int f(int x) { struct sigaction state; sigaction(x, 0, &state); return x; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else { panic!() };
        assert!(matches!(body[1].kind, StmtKind::Expression(Expr { kind: ExprKind::Call {..}, .. })));
    }

    #[test]
    fn array_allocation_and_deallocation_operator_headers_keep_exception_specs() {
        let unit = parse("void *operator new[](unsigned long size) { return 0; } void operator delete[](void *ptr) throw() { release(ptr); }", true);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["new[]", "delete[]"]);
    }

    #[test]
    fn block_scope_prototypes_emit_method_stubs_without_local_initializers() {
        let source = "int f(int x) { extern void start(void) __attribute__((__noreturn__)); int first(int), second(int); start(); return first(x); }";
        for cpp in [false, true] {
            let unit = parse(source, cpp);
            assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
            let expected = if cpp { ["f", "first", "start"] } else { ["f", "start", "first"] };
            assert_eq!(unit.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), expected);
            assert!(unit.functions[1..].iter().all(|f| matches!(f.body.kind, StmtKind::Empty)));
            let StmtKind::Block(body) = &unit.functions[0].body.kind else { panic!() };
            assert!(matches!(&body[0].kind, StmtKind::Declaration(values) if values.is_empty()));
        }
    }

    #[test]
    fn grouped_declarators_keep_definitions_and_exclude_pointer_objects() {
        let source = "typedef int Number; Number (*callback)(int); static inline Number (*arrays(void))[4] { return 0; } int (digit)(int x) { return x; } static int (inside(int x)) { return x; } void (handler)(void) { return; }";
        let unit = parse(source, false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["arrays", "digit", "inside", "handler"]);
        assert_eq!(unit.functions[0].return_type, "Number*[4]");
        assert_eq!(unit.functions[1].parameters[0].name, "x");
    }

    #[test]
    fn unnamed_declared_parameters_keep_empty_binding_names() {
        let unit = parse("int sink(int, const char *);", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions[0].parameters[0].name, "");
        assert_eq!(unit.functions[0].parameters[1].name, "");
    }

    #[test]
    fn cpp_keyword_spelling_in_c_parameter_does_not_hide_later_definitions() {
        let unit = parse("struct block { int value; }; static struct block *using_blocks(struct block *using[2]) { return using[0]; } int after(int x) { if (x) return 1; return 0; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["using_blocks", "after"]);
    }

    #[test]
    fn object_shadowing_typedef_keeps_mutations_and_identifier_condition() {
        let unit = parse("typedef int count; int f(int count) { count--; count -= 1; if (count) return 1; return 0; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else { panic!() };
        assert!(matches!(body[0].kind, StmtKind::Expression(Expr {kind: ExprKind::Unary {postfix:true,..},..})));
        assert!(matches!(&body[1].kind, StmtKind::Expression(Expr {kind: ExprKind::Binary {op,..},..}) if op == "-="));
        assert!(matches!(body[2].kind, StmtKind::If { condition:Expr {kind:ExprKind::Identifier(_),..},..}));
    }

    #[test]
    fn first_function_pointer_uses_cdt_binding_conversion_and_ignores_siblings() {
        let source = "int target(int x); void f(int x) { int (*pointer)(int) = x ? target : second, ignored = call(); }";
        for cpp in [false, true] {
            let unit = parse(source, cpp);
            assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
            let StmtKind::Block(body) = &unit.functions[0].body.kind else { panic!() };
            let StmtKind::Declaration(values) = &body[0].kind else { panic!() };
            assert_eq!(values.len(), 1);
            assert_eq!(values[0].name, "pointer");
            assert!(values[0].initializer.is_none());
            assert_eq!(values[0].problem, cpp);
        }
    }

    #[test]
    fn plain_first_declarator_keeps_later_function_pointer_initializer() {
        let unit = parse("void f(int x) { int first = call(), (*pointer)(int) = x ? target : second; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else { panic!() };
        let StmtKind::Declaration(values) = &body[0].kind else { panic!() };
        assert_eq!(values.len(), 2);
        assert!(values.iter().all(|d| d.initializer.is_some() && !d.problem));
    }

    #[test]
    fn an_unsized_array_modifier_disables_all_allocation_bounds() {
        let unit = parse("void f(int n) { extern int values[][n + 1]; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else { panic!() };
        let StmtKind::Declaration(values) = &body[0].kind else { panic!() };
        assert!(values[0].dimensions.is_empty());
    }

    #[test]
    fn genuine_type_bindings_govern_ambiguous_address_casts() {
        let inferred = parse("int f(uint32_t n) { return (uint32_t)&target; }", false);
        let declared = parse("typedef unsigned int uint32_t; int f(uint32_t n) { return (uint32_t)&target; }", false);
        for unit in [&inferred, &declared] {
            assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        }
        let StmtKind::Block(inferred_body) = &inferred.functions[0].body.kind else { panic!() };
        let StmtKind::Block(declared_body) = &declared.functions[0].body.kind else { panic!() };
        assert!(matches!(&inferred_body[0].kind, StmtKind::Return(Some(e)) if matches!(&e.kind, ExprKind::Binary {op,..} if op=="&")));
        assert!(matches!(&declared_body[0].kind, StmtKind::Return(Some(e)) if matches!(e.kind, ExprKind::Cast {..})));
    }

    #[test]
    fn top_level_declarations_convert_each_function_declarator() {
        let unit = parse("void first(void), second(int); char *third(void), *fourth(int);", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["third", "fourth", "first", "second"]);
        assert_eq!(unit.functions[0].return_type, "char*");
        assert_eq!(unit.functions[1].return_type, "char*");
    }

    #[test]
    fn record_attributes_and_grouped_arrays_do_not_create_methods() {
        let unit = parse("typedef unsigned char byte; struct __attribute__((aligned(8))) R { byte (mask)[8]; }; int after(void) { return 0; }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.len(), 1);
        assert_eq!(unit.functions[0].name, "after");
    }

    #[test]
    fn attributed_unknown_function_pointer_is_a_declaration() {
        let unit = parse("int f(void) { LONG (__attribute__((__stdcall__)) *query)(HANDLE handle); return query(0); }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else { panic!() };
        let StmtKind::Declaration(values) = &body[0].kind else { panic!() };
        assert_eq!(values[0].name, "query");
        assert_eq!(values.len(), 1);
    }

    #[test]
    fn void_and_function_pointer_parameters_preserve_cdt_properties() {
        let unit = parse("int empty(void) { return 0; } int f(int (*call)(char *), char *p) { return call(p); }", false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions[0].signature, "int(void)");
        assert_eq!(unit.functions[0].parameters[0].type_name, "void");
        assert!(unit.functions[0].parameters[0].name.is_empty());
        assert_eq!(unit.functions[1].parameters[0].type_name, "int");
        assert!(unit.functions[1].parameters[0].function_pointer);
        assert_eq!(unit.functions[1].parameters[1].type_name, "char*");
        assert!(!unit.functions[1].parameters[1].function_pointer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preprocessed_types_pointers_and_control_flow() {
        let source = "# 1 \"x.c\"\ntypedef unsigned long size_t;\nint helper(int);\nint f(int x, char *p) { size_t n = x, j; int (*fn)(int) = helper;\nL: for (j=0; j<n; ++j) { if (fn(j) && p[j]) continue; else break; }\nswitch (x) { case 1: x++; case 2 ... 4: goto L; default: return (int)n; } }";
        let unit = parse(source, false);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(
            unit.functions
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            ["f", "helper"]
        );
        assert!(matches!(unit.functions[1].body.kind, StmtKind::Empty));
        assert_eq!(unit.functions[0].parameters[1].type_name, "char*");
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        let StmtKind::Declaration(declarations) = &body[1].kind else {
            panic!()
        };
        assert_eq!(declarations[0].name, "fn");
        assert!(
            matches!(&body[2].kind, StmtKind::Label { statement, .. } if matches!(statement.kind, StmtKind::For { .. }))
        );
    }

    #[test]
    fn precedence_and_statement_expressions() {
        let unit = parse(
            "int f(int a, int b) { int x = ({ int t = b; t + 1; }); return a ? x : b || a && x; }",
            false,
        );
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        let StmtKind::Return(Some(Expr {
            kind: ExprKind::Conditional { alternative, .. },
            ..
        })) = &body[1].kind
        else {
            panic!()
        };
        assert!(
            matches!(&alternative.kind, ExprKind::Binary { op, right, .. } if op == "||" && matches!(&right.kind, ExprKind::Binary { op, .. } if op == "&&"))
        );
    }

    #[test]
    fn namespace_methods_and_cpp_conditions() {
        let unit = parse(
            "namespace n { class A { public: int f(int x) { if (int y=x) return y; for (auto z : xs) x += z; try { throw x; } catch (...) { return 0; } return x; } }; template<class T> T id(T x) { return x; } }",
            true,
        );
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.len(), 2);
        assert_eq!(unit.functions[0].full_name, "n.A.f:int(int)");
        assert_eq!(unit.functions[1].return_type, "T");
    }

    #[test]
    fn parentheses_keep_operand_source_spans() {
        let source = "int f(int x) { return (x) + 1; }";
        let unit = parse(source, false);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        let StmtKind::Return(Some(Expr {
            kind: ExprKind::Binary { left, .. },
            span,
        })) = &body[0].kind
        else {
            panic!()
        };
        assert_eq!(&source[left.span.start..left.span.end], "x");
        assert_eq!(&source[span.start..span.end], "(x) + 1");
    }

    #[test]
    fn unknown_types_gnu_ternary_and_computed_goto() {
        let unit = parse(
            "__int64 f(_DWORD *a) { unknown_t v = (_DWORD)*a; int x = v ?: 1; goto *a; }",
            false,
        );
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        assert!(matches!(&body[2].kind, StmtKind::Goto(name) if name == "*"));
    }

    #[test]
    fn knr_definitions_use_parameter_declarations() {
        let unit = parse(
            "void f(x,p) unsigned x; char *p; { while (x--) *p++=0; } unsigned g() { return 1; }",
            false,
        );
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.len(), 2);
        assert_eq!(unit.functions[0].parameters[0].name, "x");
        assert_eq!(unit.functions[0].parameters[0].type_name, "unsigned");
        assert_eq!(unit.functions[0].parameters[1].type_name, "char*");
        assert!(matches!(unit.functions[0].body.kind, StmtKind::Block(_)));
    }

    #[test]
    fn arrays_default_parameters_and_operator_methods() {
        let unit = parse(
            "int sum(int a[4]); struct A { bool operator==(A x) { return 1; } int operator()(int x=3) { return x; } }; int f(int x=1) { return x; }",
            true,
        );
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(
            unit.functions
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            ["==", "()", "f", "sum"]
        );
        assert_eq!(unit.functions[3].parameters[0].type_name, "int[4]");
    }

    #[test]
    fn unknown_parenthesized_identifier_remains_binary_operand() {
        let unit = parse("int f(void) { return (MAX) + 1; }", false);
        assert!(unit.diagnostics.is_empty());
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        assert!(
            matches!(&body[0].kind, StmtKind::Return(Some(Expr {kind: ExprKind::Binary {op,..},..})) if op == "+")
        );
    }

    #[test]
    fn sizeof_keeps_call_operands_in_logical_expression() {
        let unit = parse(
            "int foo(int); int bar(int); int f(int n) { return sizeof(foo(n) && bar(n)); }",
            false,
        );
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        let StmtKind::Return(Some(Expr {
            kind: ExprKind::Unary { op, argument, .. },
            ..
        })) = &body[0].kind
        else {
            panic!()
        };
        assert_eq!(op, "sizeof");
        assert!(
            matches!(&argument.kind, ExprKind::Binary { op, left, right } if op == "&&" && matches!(left.kind, ExprKind::Call {..}) && matches!(right.kind, ExprKind::Call {..}))
        );
    }

    #[test]
    fn unknown_calls_with_address_arguments_are_expressions() {
        let source = "int f(char *src, int *p) { char filename[20]; char *sa=filename; strcat(&filename, &src[(int)(sa - &filename)]); if (cond) strcat(&filename, \".bz2\"); consume(*p); Unknown (*callback)(int); return (uch)(*p); }";
        for cpp in [false, true] {
            let unit = parse(source, cpp);
            assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
            let StmtKind::Block(body) = &unit.functions[0].body.kind else {
                panic!()
            };
            assert!(matches!(
                &body[2].kind,
                StmtKind::Expression(Expr {
                    kind: ExprKind::Call { .. },
                    ..
                })
            ));
            assert!(
                matches!(&body[3].kind, StmtKind::If {consequence,..} if matches!(consequence.kind, StmtKind::Expression(Expr {kind: ExprKind::Call {..},..})))
            );
            assert!(matches!(
                &body[4].kind,
                StmtKind::Expression(Expr {
                    kind: ExprKind::Call { .. },
                    ..
                })
            ));
            assert!(
                matches!(&body[5].kind, StmtKind::Declaration(declarations) if declarations[0].name == "callback")
            );
            assert!(matches!(
                &body[6].kind,
                StmtKind::Return(Some(Expr {
                    kind: ExprKind::Call { .. },
                    ..
                }))
            ));
        }
    }

    #[test]
    fn cannot_silently_drop_control_flow() {
        let unit = parse(
            "int f(int x) { co_await x; asm goto (\"\" : : : : done); done: return x; }",
            true,
        );
        assert!(!unit.diagnostics.is_empty());
        assert_eq!(unit.functions.len(), 1);
    }
}
