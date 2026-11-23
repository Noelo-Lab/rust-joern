//! The C/C++ syntax used by the CDT-to-CPG conversion, without a parser runtime.
//! Unsupported executable syntax is reported, rather than silently dropping its CFG.
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::lexer::{
    RetainedMacroInvocation, Token, TokenKind, lex_preprocessed,
    lex_preprocessed_with_macros_for_language,
};
use crate::syntax::*;

mod cdt_recovery;
mod cpp;
mod declarations;
mod expressions;
mod helpers;
mod macros;
mod statements;
mod tokens;
mod types;
use helpers::*;

#[cfg(test)]
pub fn parse(source: &str, cpp: bool) -> TranslationUnit {
    parse_preprocessed(source, cpp, false)
}

#[cfg(test)]
pub fn parse_preprocessed(source: &str, cpp: bool, preprocessed: bool) -> TranslationUnit {
    parse_with(source, cpp, preprocessed, false, &[], false)
}

/// `cdt_failures` reproduces CDT's failure on a parameter list or expression
/// it cannot parse; otherwise that code is still analyzed. `type_names` are
/// caller-proved visible C++ type names for an isolated source slice.
/// `constructor_initializers` enables the opt-in scalar initializer profile.
pub fn parse_with(
    source: &str,
    cpp: bool,
    preprocessed: bool,
    cdt_failures: bool,
    type_names: &[String],
    constructor_initializers: bool,
) -> TranslationUnit {
    let (unit, later) = parse_unit(
        source,
        cpp,
        preprocessed,
        cdt_failures,
        type_names,
        constructor_initializers,
        HashSet::new(),
    );
    if later.is_empty() {
        return unit;
    }
    // C binds a function before its declaration as well (`CScope`). A
    // `T *x;` that names a later one is parsed again with those bindings.
    parse_unit(
        source,
        cpp,
        preprocessed,
        cdt_failures,
        type_names,
        constructor_initializers,
        later,
    )
    .0
}

fn parse_unit(
    source: &str,
    cpp: bool,
    preprocessed: bool,
    cdt_failures: bool,
    type_names: &[String],
    constructor_initializers: bool,
    later_functions: HashSet<String>,
) -> (TranslationUnit, HashSet<String>) {
    let (tokens, diagnostics, retained_macro_invocations) =
        lex_preprocessed_with_macros_for_language(source, preprocessed, cpp);
    let limit = tokens.len();
    let mut parser = Parser {
        source,
        tokens,
        pos: 0,
        limit,
        cpp,
        cdt_failures,
        diagnostics,
        types: type_names.iter().cloned().collect(),
        explicit_types: type_names.iter().cloned().collect(),
        supplied_type_names: type_names.iter().cloned().collect(),
        type_aliases: HashMap::new(),
        class_alias_undo: Vec::new(),
        extern_c: false,
        extern_c_names: HashSet::new(),
        class_scopes: HashSet::new(),
        class_fields: HashMap::new(),
        scalar_constructor_classes: HashSet::new(),
        constructor_initializers,
        function_fields: Vec::new(),
        function_member_cv: false,
        variables: Rc::new(HashSet::new()),
        variable_types: Rc::new(HashMap::new()),
        variable_closures: Rc::new(HashMap::new()),
        function_returns: HashMap::new(),
        object_bindings: HashSet::new(),
        global_pointers: HashSet::new(),
        external_functions: HashSet::new(),
        later_functions,
        record_depth: 0,
        function_full_name: String::new(),
        imports: HashMap::new(),
        using_namespaces: Vec::new(),
        lexical_scope: String::new(),
        asm_problem_recovery: false,
        in_function_body: false,
        function_declarations: Vec::new(),
        global_expressions: Vec::new(),
        macro_expansion: false,
        current_method: String::new(),
        lambda_counter: 0,
        defined_cpp_methods: HashSet::new(),
        lambda_ast_parent: String::new(),
        retained_macro_invocations,
        retained_macro_call_ends: HashSet::new(),
    };
    let mut functions = Vec::new();
    parser.translation_scope("", &mut functions, false);
    functions.append(&mut parser.function_declarations);
    let later = parser.later_function_names(&functions);
    let declaration_count = functions
        .iter()
        .filter(|function| matches!(function.body.kind, StmtKind::Empty))
        .map(|function| function.full_name.as_str())
        .collect::<HashSet<_>>()
        .len();
    // Joern collects declarations separately, and emits only those without a
    // matching definition after visiting every translation-unit declaration.
    let definitions: HashSet<_> = functions
        .iter()
        .filter(|f| !matches!(f.body.kind, StmtKind::Empty))
        .map(|f| f.full_name.clone())
        .collect();
    // registerMethodDeclaration keeps the first declaration it visits, which
    // may be a block-scope one inside an earlier function body.
    let mut first_declaration: HashMap<String, usize> = HashMap::new();
    for function in &functions {
        if matches!(function.body.kind, StmtKind::Empty) {
            first_declaration
                .entry(function.full_name.clone())
                .and_modify(|start| *start = (*start).min(function.span.start))
                .or_insert(function.span.start);
        }
    }
    let mut declarations = HashSet::new();
    functions.retain(|f| {
        !matches!(f.body.kind, StmtKind::Empty)
            || !definitions.contains(&f.full_name)
                && first_declaration[&f.full_name] == f.span.start
                && declarations.insert(f.full_name.clone())
    });
    let declaration_names: Vec<_> = functions
        .iter()
        .filter(|function| matches!(function.body.kind, StmtKind::Empty))
        .map(|function| function.full_name.clone())
        .collect();
    let ranks = joern_declaration_order(&declaration_names, declaration_count);
    functions.sort_by_key(|f| {
        if matches!(f.body.kind, StmtKind::Empty) {
            (true, ranks[&f.full_name])
        } else {
            (false, 0)
        }
    });
    let retained_macro_calls = parser
        .retained_macro_invocations
        .into_iter()
        .filter(|event| parser.retained_macro_call_ends.contains(&event.callee_end))
        .map(|event| event.descriptor)
        .collect();
    let unit = TranslationUnit {
        known_type_names: parser
            .explicit_types
            .union(&parser.class_scopes)
            .cloned()
            .collect(),
        functions,
        global_expressions: parser.global_expressions,
        retained_macro_calls,
        diagnostics: parser.diagnostics,
    };
    (unit, later)
}

fn joern_declaration_order(names: &[String], registered_count: usize) -> HashMap<String, usize> {
    // AstCreationPass copies its ConcurrentHashMap into Scala's immutable Map,
    // then removes definitions. Small Maps retain Java's bucket order; larger
    // Maps iterate the hash trie, visiting inline data before child nodes.
    fn java_hash(name: &str) -> u32 {
        name.encode_utf16().fold(0u32, |hash, unit| {
            hash.wrapping_mul(31).wrapping_add(unit as u32)
        })
    }
    fn improved_hash(name: &str) -> u32 {
        let hash = java_hash(name);
        let hash = hash.wrapping_add(!(hash << 9));
        let hash = hash ^ (hash >> 14);
        let hash = hash.wrapping_add(hash << 4);
        hash ^ (hash >> 10)
    }
    fn trie_order<'a>(names: Vec<&'a String>, shift: u32, output: &mut Vec<&'a String>) {
        if shift >= 32 {
            output.extend(names);
            return;
        }
        let mut slots: [Vec<&String>; 32] = std::array::from_fn(|_| Vec::new());
        for name in names {
            slots[((improved_hash(name) >> shift) & 31) as usize].push(name);
        }
        output.extend(
            slots
                .iter()
                .filter(|slot| slot.len() == 1)
                .map(|slot| slot[0]),
        );
        for slot in slots.into_iter().filter(|slot| slot.len() > 1) {
            trie_order(slot, shift + 5, output);
        }
    }
    let mut order: Vec<_> = names.iter().collect();
    if registered_count <= 4 {
        order.sort_by_key(|name| {
            let hash = java_hash(name);
            (hash ^ (hash >> 16)) & 15
        });
    } else {
        let mut sorted = Vec::with_capacity(order.len());
        trie_order(order, 0, &mut sorted);
        order = sorted;
    }
    order
        .into_iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index))
        .collect()
}

#[cfg(test)]
mod expression_port_tests;

struct AliasUndo {
    alias: Option<String>,
    was_type: bool,
    was_explicit: bool,
}

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    limit: usize,
    cpp: bool,
    /// Whether code that CDT fails to parse is dropped as Joern drops it.
    cdt_failures: bool,
    diagnostics: Vec<ParseDiagnostic>,
    types: HashSet<String>,
    explicit_types: HashSet<String>,
    supplied_type_names: HashSet<String>,
    type_aliases: HashMap<String, String>,
    class_alias_undo: Vec<HashMap<String, AliasUndo>>,
    extern_c: bool,
    extern_c_names: HashSet<String>,
    class_scopes: HashSet<String>,
    class_fields: HashMap<String, Vec<(String, String)>>,
    scalar_constructor_classes: HashSet<String>,
    constructor_initializers: bool,
    function_fields: Vec<(String, String)>,
    function_member_cv: bool,
    variables: Rc<HashSet<String>>,
    variable_types: Rc<HashMap<String, String>>,
    variable_closures: Rc<HashMap<String, Closure>>,
    function_returns: HashMap<String, String>,
    /// File-scope C names whose first declaration is an object.
    object_bindings: HashSet<String>,
    /// File-scope C objects whose first declarator is a pointer.
    global_pointers: HashSet<String>,
    /// Undeclared C names that CDT bound as external functions while it
    /// resolved a statement ambiguity; the binding lasts for the unit.
    external_functions: HashSet<String>,
    /// C functions that a `T *x;` statement names before their declaration.
    later_functions: HashSet<String>,
    /// Nesting of record bodies, whose members are not file-scope names.
    record_depth: usize,
    function_full_name: String,
    imports: HashMap<String, String>,
    using_namespaces: Vec<String>,
    lexical_scope: String,
    asm_problem_recovery: bool,
    in_function_body: bool,
    function_declarations: Vec<Function>,
    global_expressions: Vec<Expr>,
    macro_expansion: bool,
    current_method: String,
    lambda_counter: usize,
    defined_cpp_methods: HashSet<String>,
    lambda_ast_parent: String,
    retained_macro_invocations: Vec<RetainedMacroInvocation>,
    retained_macro_call_ends: HashSet<usize>,
}

struct Header {
    name: String,
    name_start: usize,
    parameters_start: usize,
    parameters_end: usize,
    return_type: Option<String>,
    /// For one declarator of a declaration with several: the end of their
    /// shared specifiers, then the starts of the first declarator and this one.
    shared: Option<(usize, usize, usize)>,
    /// The parameter list after a group that nests this function declarator,
    /// as in `void (*hook(void))(int)`.
    outer_parameters: Option<(usize, usize)>,
}

struct CppExpressionName {
    end: usize,
    destructor: Option<usize>,
}

#[cfg(test)]
mod declaration_tests;

#[cfg(test)]
mod tests;
