mod builder;
mod dataflow;
mod ddg;
pub mod graph;
mod lexer;
mod normalize;
mod parser;
mod syntax;

use graph::{Analysis, Diagnostic, Edge, FunctionGraph, Node, Options, PropertyGraph};
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::{CStr, CString, c_char};

pub fn analyze(source: &str, filename: &str, options: &Options) -> Result<Analysis, String> {
    analyze_sources(&[(source, filename)], options)
}

/// Analyze independent source files as one analysis. Each `(source, filename)`
/// pair retains its own language and source locations, while data-flow passes
/// resolve internal callees against the methods recovered from all files.
pub fn analyze_sources(sources: &[(&str, &str)], options: &Options) -> Result<Analysis, String> {
    let mut units = sources
        .iter()
        .map(|&(source, filename)| build_source(source, filename, options))
        .collect::<Result<Vec<_>, _>>()?;
    // Joern emits source declarations only when no matching definition was
    // recovered from any translation unit. Empty definitions are still real
    // definitions, even though their CFG qualifies as a data-flow stub.
    let defined_methods: HashSet<_> = units
        .iter()
        .flat_map(|unit| unit.defined_methods.iter().cloned())
        .collect();
    for unit in &mut units {
        unit.functions.retain(|function| {
            !unit.prototype_methods.contains(&function.fullname)
                || !defined_methods.contains(&function.fullname)
        });
    }
    let internal_methods: HashSet<_> = units
        .iter()
        .flat_map(|unit| unit.internal_methods.iter().cloned())
        .collect();
    let nonstub_methods: HashSet<_> = units
        .iter()
        .flat_map(|unit| &unit.functions)
        .filter(|function| {
            function.cpg.edges.iter().any(|edge| {
                edge.kind == "CFG"
                    && function.cpg.nodes[edge.source as usize].kind == "METHOD"
                    && function.cpg.nodes[edge.target as usize].kind != "METHOD_RETURN"
            })
        })
        .map(|function| function.fullname.clone())
        .collect();
    if options.data_flow || options.reaching_definitions {
        units.par_iter_mut().for_each(|unit| {
            unit.functions.par_iter_mut().for_each(|function| {
                dataflow::apply_with_context(
                    function,
                    options.reaching_definitions,
                    &nonstub_methods,
                    unit.cpp,
                );
                function.ddg_view = Some(ddg::project(
                    function,
                    &nonstub_methods,
                    &internal_methods,
                    unit.cpp,
                ));
            });
            unit.diagnostics.push(Diagnostic {
                filename: unit.filename.clone(),
                line: 0,
                column: 0,
                severity: "warning".into(),
                message: dataflow::LIMITATIONS.into(),
            });
        });
    }
    Ok(Analysis {
        schema_version: 1,
        functions: units
            .iter_mut()
            .flat_map(|unit| std::mem::take(&mut unit.functions))
            .collect(),
        diagnostics: units
            .into_iter()
            .flat_map(|unit| unit.diagnostics)
            .collect(),
    })
}

struct BuiltSource {
    filename: String,
    cpp: bool,
    functions: Vec<FunctionGraph>,
    diagnostics: Vec<Diagnostic>,
    internal_methods: HashSet<String>,
    prototype_methods: HashSet<String>,
    defined_methods: HashSet<String>,
}

fn build_source(source: &str, filename: &str, options: &Options) -> Result<BuiltSource, String> {
    let cpp = match options.language.as_deref() {
        Some("cpp" | "c++") => true,
        Some("c") => false,
        Some(other) => return Err(format!("unsupported language: {other}")),
        None => matches!(
            std::path::Path::new(filename)
                .extension()
                .and_then(|s| s.to_str()),
            Some("cpp" | "cc" | "cxx" | "c++" | "C" | "hpp" | "hh" | "hxx" | "ii")
        ),
    };
    let preprocessed = options.preprocessed
        || matches!(
            std::path::Path::new(filename)
                .extension()
                .and_then(|s| s.to_str()),
            Some("i" | "ii")
        );
    let unit = parser::parse_preprocessed(source, cpp, preprocessed);
    let mut diagnostics: Vec<_> = unit
        .diagnostics
        .iter()
        .map(|d| Diagnostic {
            filename: filename.into(),
            line: d.span.line,
            column: d.span.column,
            severity: "error".into(),
            message: d.message.clone(),
        })
        .collect();
    let known_functions = unit
        .functions
        .iter()
        .flat_map(|f| {
            [
                f.name.clone(),
                f.full_name.clone(),
                f.full_name.split(':').next().unwrap().replace('.', "::"),
            ]
        })
        .collect();
    let mut methods: HashMap<String, Vec<&syntax::Function>> = HashMap::new();
    for function in &unit.functions {
        methods
            .entry(function.full_name.split(':').next().unwrap().to_owned())
            .or_default()
            .push(function);
    }
    let mut built: Vec<_> = unit
        .functions
        .par_iter()
        .map(|f| builder::build(f, source, filename, &known_functions, cpp, &methods))
        .collect();
    for (_, messages) in &mut built {
        diagnostics.append(messages);
    }
    if options.strict && diagnostics.iter().any(|d| d.severity == "error") {
        return Err(diagnostics
            .iter()
            .filter(|d| d.severity == "error")
            .map(|d| format!("{}:{}:{}: {}", d.filename, d.line, d.column, d.message))
            .collect::<Vec<_>>()
            .join("\n"));
    }
    let mut functions: Vec<_> = built.into_iter().map(|(f, _)| f).collect();
    let global = (!unit.global_expressions.is_empty()).then(|| {
        // Global initializers/bounds can register external builtin macro
        // methods even though PyJoern does not expose the <global> method.
        let function = syntax::Function {
            name: "<global>".into(),
            full_name: "<global>".into(),
            return_type: "void".into(),
            binding_return_type: "void".into(),
            signature: "void()".into(),
            implicit_this: None,
            implicit_fields: Vec::new(),
            member_cv_qualified: false,
            lambda: false,
            is_static: false,
            inherited_bindings: Vec::new(),
            inherited_closures: Vec::new(),
            parameters: Vec::new(),
            body: syntax::Stmt {
                kind: syntax::StmtKind::Sequence(
                    unit.global_expressions
                        .iter()
                        .map(|expression| syntax::Stmt {
                            span: expression.span.clone(),
                            kind: syntax::StmtKind::Expression(expression.clone()),
                        })
                        .collect(),
                ),
                span: syntax::Span::default(),
            },
            span: syntax::Span::default(),
        };
        builder::build(&function, source, filename, &known_functions, cpp, &methods)
            .0
            .cpg
    });
    append_macro_methods(&mut functions, global.as_ref());
    functions
        .par_iter_mut()
        .for_each(dataflow::decorate_parameters);
    Ok(BuiltSource {
        filename: filename.into(),
        cpp,
        functions,
        diagnostics,
        // Source declarations are internal methods too, even when their CFG
        // is a stub. The DDG exporter uses these when hiding call arguments.
        internal_methods: unit.functions.iter().map(|f| f.full_name.clone()).collect(),
        prototype_methods: unit
            .functions
            .iter()
            .filter(|f| matches!(f.body.kind, syntax::StmtKind::Empty))
            .map(|f| f.full_name.clone())
            .collect(),
        defined_methods: unit
            .functions
            .iter()
            .filter(|f| !matches!(f.body.kind, syntax::StmtKind::Empty))
            .map(|f| f.full_name.clone())
            .collect(),
    })
}

/// Joern's base pass creates external methods for CDT's predefined macros.
/// Their missing METHOD_RETURN line also changes PyJoern's boundary lifting.
fn append_macro_methods(functions: &mut Vec<FunctionGraph>, global: Option<&PropertyGraph>) {
    let mut macros = BTreeMap::new();
    for graph in functions
        .iter()
        .map(|function| &function.cpg)
        .chain(global)
    {
        for call in graph.nodes.iter().filter(|node| node.kind == "CALL") {
            let Some(fullname) = call.method_full_name.as_deref() else {
                continue;
            };
            let Some((name, arity)) = fullname
                .strip_prefix(":-1:-1:")
                .and_then(|identity| identity.rsplit_once(':'))
            else {
                continue;
            };
            if arity.parse::<usize>().is_err() {
                continue;
            }
            let parameters = graph
                .edges
                .iter()
                .filter(|edge| edge.kind == "ARGUMENT" && edge.source == call.id)
                .count();
            macros.insert(fullname.to_owned(), (name.to_owned(), parameters));
        }
    }
    for (fullname, (name, parameters)) in macros {
        if functions
            .iter()
            .any(|function| function.fullname == fullname)
        {
            continue;
        }
        let mut cpg = PropertyGraph::default();
        let mut node = |kind: &str, code: String, name: Option<String>, line: i64, cfg_nop| {
            let id = cpg.nodes.len() as u32;
            cpg.nodes.push(Node {
                id,
                kind: kind.into(),
                code,
                name,
                method_full_name: None,
                cfg_nop,
                type_name: (kind != "METHOD").then(|| "ANY".into()),
                line,
                column: 0,
                start_byte: 0,
                end_byte: 0,
            });
            id
        };
        let method = node("METHOD", String::new(), Some(name.clone()), -1, Some(true));
        for index in 1..=parameters {
            let parameter = format!("p{index}");
            node(
                "METHOD_PARAMETER_IN",
                parameter.clone(),
                Some(parameter),
                0,
                None,
            );
        }
        node("BLOCK", String::new(), None, 0, None);
        let ret = node("METHOD_RETURN", "RET".into(), None, 0, Some(false));
        for target in 1..=ret {
            cpg.edges.push(Edge {
                source: method,
                target,
                kind: "AST".into(),
                label: None,
            });
        }
        cpg.edges.push(Edge {
            source: method,
            target: ret,
            kind: "CFG".into(),
            label: None,
        });
        let cfg = normalize::normalize(&cpg);
        functions.push(FunctionGraph {
            name,
            fullname,
            filename: String::new(),
            return_type: String::new(),
            signature: String::new(),
            start_line: -1,
            end_line: -1,
            cpg,
            cfg,
            ddg: None,
            ddg_view: None,
            reaching_definitions: None,
        });
    }
}

/// Analyze UTF-8 source directly in memory. The returned string must be released
/// with `rust_joern_free`; all input pointers are borrowed for this call only.
///
/// # Safety
/// `source` must address `source_len` readable bytes (or be null when length is
/// zero), and non-null string pointers must be valid NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_joern_analyze(
    source: *const u8,
    source_len: usize,
    filename: *const c_char,
    options_json: *const c_char,
) -> *mut c_char {
    let result = std::panic::catch_unwind(|| -> Result<String, String> {
        let (source, filename) = unsafe { borrow_source(source, source_len, filename) }?;
        let options = unsafe { borrow_options(options_json) }?;
        serde_json::to_string(&analyze(source, filename, &options)?).map_err(|e| e.to_string())
    });
    export_ffi_result(result)
}

/// One source file borrowed by `rust_joern_analyze_many`.
#[repr(C)]
pub struct RustJoernSource {
    pub source: *const u8,
    pub source_len: usize,
    pub filename: *const c_char,
}

/// Analyze source files together directly in memory. Inputs remain borrowed
/// for this call, and the result must be released with `rust_joern_free`.
///
/// # Safety
/// `sources` must address `source_count` readable `RustJoernSource` entries, or
/// be null when the count is zero. Each entry follows `rust_joern_analyze`'s
/// pointer requirements, as does the optional NUL-terminated options string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_joern_analyze_many(
    sources: *const RustJoernSource,
    source_count: usize,
    options_json: *const c_char,
) -> *mut c_char {
    let result = std::panic::catch_unwind(|| -> Result<String, String> {
        if sources.is_null() && source_count != 0 {
            return Err("null sources pointer".into());
        }
        let sources = if source_count == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(sources, source_count) }
        };
        let sources = sources
            .iter()
            .map(|source| unsafe {
                borrow_source(source.source, source.source_len, source.filename)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let options = unsafe { borrow_options(options_json) }?;
        serde_json::to_string(&analyze_sources(&sources, &options)?).map_err(|e| e.to_string())
    });
    export_ffi_result(result)
}

unsafe fn borrow_source<'a>(
    source: *const u8,
    source_len: usize,
    filename: *const c_char,
) -> Result<(&'a str, &'a str), String> {
    if source.is_null() && source_len != 0 {
        return Err("null source pointer".into());
    }
    let bytes = if source_len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(source, source_len) }
    };
    let source = std::str::from_utf8(bytes).map_err(|e| format!("source is not UTF-8: {e}"))?;
    let filename = if filename.is_null() {
        "<memory>.c"
    } else {
        unsafe { CStr::from_ptr(filename) }
            .to_str()
            .map_err(|e| e.to_string())?
    };
    Ok((source, filename))
}

unsafe fn borrow_options(options_json: *const c_char) -> Result<Options, String> {
    if options_json.is_null() {
        Ok(Options::default())
    } else {
        serde_json::from_slice(unsafe { CStr::from_ptr(options_json) }.to_bytes())
            .map_err(|e| format!("invalid options: {e}"))
    }
}

fn export_ffi_result(result: std::thread::Result<Result<String, String>>) -> *mut c_char {
    let json = match result {
        Ok(Ok(json)) => json,
        Ok(Err(message)) => serde_json::json!({"error": message}).to_string(),
        Err(_) => serde_json::json!({"error": "native parser panicked"}).to_string(),
    };
    CString::new(json)
        .expect("JSON contains no raw NUL")
        .into_raw()
}

/// Release a string returned by either native analysis entry point.
///
/// # Safety
/// `pointer` must be null or an unreleased allocation from an analysis function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_joern_free(pointer: *mut c_char) {
    if !pointer.is_null() {
        drop(unsafe { CString::from_raw(pointer) });
    }
}
