mod builder;
mod dataflow;
pub mod graph;
mod lexer;
mod normalize;
mod parser;
mod syntax;

use graph::{
    Analysis, Diagnostic, Edge, ExternalReference, FunctionGraph, Node, Options, PropertyGraph,
};
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap};
use std::ffi::{CStr, CString, c_char};

pub fn analyze(source: &str, filename: &str, options: &Options) -> Result<Analysis, String> {
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
    bind_lambda_captures(&unit.functions, &mut functions);
    let global = (!unit.global_expressions.is_empty()).then(|| {
        let function = syntax::Function {
            name: "<global>".into(),
            full_name: "<global>".into(),
            return_type: "void".into(),
            signature: "void()".into(),
            extern_c: false,
            implicit_this: None,
            lambda_parent: None,
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
    append_macro_methods(
        &mut functions,
        global.as_ref(),
        &unit.retained_macro_calls,
        filename,
    );
    functions
        .par_iter_mut()
        .for_each(dataflow::decorate_parameters);
    if options.data_flow || options.reaching_definitions {
        let all_internal_methods = unit
            .functions
            .iter()
            .map(|function| function.full_name.clone())
            .collect();
        let internal_methods = functions
            .iter()
            .filter(|function| {
                function.cpg.edges.iter().any(|edge| {
                    edge.kind == "CFG"
                        && function.cpg.nodes[edge.source as usize].kind == "METHOD"
                        && function.cpg.nodes[edge.target as usize].kind != "METHOD_RETURN"
                })
            })
            .map(|function| function.fullname.clone())
            .collect();
        functions.par_iter_mut().for_each(|f| {
            dataflow::apply_with_context(f, options.reaching_definitions, &internal_methods, cpp);
            f.ddg_projection = Some(dataflow::project_ddg(
                f,
                &internal_methods,
                &all_internal_methods,
                cpp,
            ));
        });
        diagnostics.push(Diagnostic {
            filename: filename.into(),
            line: 0,
            column: 0,
            severity: "warning".into(),
            message: dataflow::LIMITATIONS.into(),
        });
    }
    Ok(Analysis {
        schema_version: 1,
        functions,
        diagnostics,
    })
}

/// Joern's base pass creates external methods for CDT's predefined macros.
/// Their missing METHOD_RETURN line also changes PyJoern's boundary lifting.
fn append_macro_methods(
    functions: &mut Vec<FunctionGraph>,
    global: Option<&PropertyGraph>,
    retained: &[syntax::RetainedMacroCall],
    filename: &str,
) {
    let mut macros = BTreeMap::new();
    let mut calls: Vec<_> = functions
        .iter()
        .map(|function| &function.cpg)
        .chain(global)
        .flat_map(|graph| {
            graph
                .nodes
                .iter()
                .filter(|node| {
                    node.kind == "CALL"
                        && node
                            .method_full_name
                            .as_deref()
                            .is_some_and(|name| name.starts_with(":-1:-1:"))
                })
                .map(move |node| (node, graph))
        })
        .collect();
    calls.sort_by_key(|(call, _)| call.start_byte);
    for (call, graph) in calls {
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
        macros.insert(
            fullname.to_owned(),
            (name.to_owned(), parameters, String::new(), -1, -1),
        );
    }
    let macro_file = std::path::Path::new(filename)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(filename);
    for call in retained {
        let start = call.definition_span.line as i64;
        let end = call.definition_span.end_line as i64;
        macros.insert(
            format!(
                "{macro_file}:{start}:{end}:{}:{}",
                call.name, call.formal_arity
            ),
            (
                call.name.clone(),
                call.parameter_count,
                macro_file.into(),
                start,
                end,
            ),
        );
    }
    for (fullname, (name, parameters, filename, start_line, end_line)) in macros {
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
                external_ref: None,
                cfg_nop,
                type_name: (kind != "METHOD").then(|| "ANY".into()),
                line,
                column: 0,
                start_byte: 0,
                end_byte: 0,
            });
            id
        };
        let method = node(
            "METHOD",
            String::new(),
            Some(name.clone()),
            start_line,
            Some(true),
        );
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
            filename,
            return_type: String::new(),
            signature: String::new(),
            start_line,
            end_line,
            cpg,
            cfg,
            ddg: None,
            ddg_projection: None,
            reaching_definitions: None,
        });
    }
}

/// CDT converts lambda bodies while the surrounding lexical scopes are still
/// active. Its own parameter scope is already popped at that point. Retain
/// cross-method references explicitly because node IDs are local to each CPG.
fn bind_lambda_captures(syntax: &[syntax::Function], functions: &mut [FunctionGraph]) {
    for (index, lambda) in syntax.iter().enumerate() {
        if lambda.lambda_parent.is_none() {
            continue;
        }
        let mut ancestors: Vec<_> = syntax
            .iter()
            .enumerate()
            .filter(|(parent, function)| {
                *parent != index
                    && function.span.start <= lambda.span.start
                    && function.span.end >= lambda.span.end
                    && (function.span.start < lambda.span.start
                        || function.span.end > lambda.span.end)
            })
            .collect();
        ancestors.sort_by_key(|(_, function)| {
            std::cmp::Reverse(function.span.end - function.span.start)
        });
        let mut bindings = HashMap::new();
        for (parent_index, parent) in ancestors {
            let graph = &functions[parent_index].cpg;
            let Some(call) = graph
                .nodes
                .iter()
                .filter(|node| {
                    node.kind == "METHOD_REF"
                        && node.start_byte <= lambda.span.start
                        && node.end_byte >= lambda.span.end
                })
                .min_by_key(|node| node.end_byte - node.start_byte)
            else {
                continue;
            };
            let mut parents = vec![None; graph.nodes.len()];
            for edge in &graph.edges {
                if edge.kind == "AST" {
                    parents[edge.target as usize] = Some(edge.source as usize);
                }
            }
            let mut scopes = Vec::new();
            let mut current = Some(call.id as usize);
            while let Some(id) = current {
                scopes.push(id);
                current = parents[id];
            }
            for &scope in scopes.iter().rev() {
                for node in &graph.nodes {
                    let visible = node.kind == "METHOD_PARAMETER_IN"
                        && parent.lambda_parent.is_none()
                        && scope == 0
                        || node.kind == "LOCAL"
                            && node.start_byte <= call.start_byte
                            && parents[node.id as usize] == Some(scope);
                    if visible && let Some(name) = &node.name {
                        bindings.insert(
                            name.clone(),
                            (
                                ExternalReference {
                                    method_full_name: functions[parent_index].fullname.clone(),
                                    node: node.id,
                                },
                                node.type_name.clone(),
                            ),
                        );
                    }
                }
            }
        }
        let graph = &mut functions[index].cpg;
        let referenced: std::collections::HashSet<_> = graph
            .edges
            .iter()
            .filter(|edge| edge.kind == "REF")
            .map(|edge| edge.source)
            .collect();
        let mut changed_kind = false;
        for node in &mut graph.nodes {
            if !matches!(node.kind.as_str(), "IDENTIFIER" | "METHOD_REF")
                || referenced.contains(&node.id)
            {
                continue;
            }
            if let Some((target, type_name)) =
                node.name.as_ref().and_then(|name| bindings.get(name))
            {
                node.external_ref = Some(target.clone());
                node.type_name.clone_from(type_name);
                if node.kind == "METHOD_REF" {
                    node.kind = "IDENTIFIER".into();
                    node.method_full_name = None;
                    node.cfg_nop = None;
                    changed_kind = true;
                }
            }
        }
        if changed_kind {
            functions[index].cfg = normalize::normalize(&functions[index].cpg);
        }
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
        let options = if options_json.is_null() {
            Options::default()
        } else {
            serde_json::from_slice(unsafe { CStr::from_ptr(options_json) }.to_bytes())
                .map_err(|e| format!("invalid options: {e}"))?
        };
        serde_json::to_string(&analyze(source, filename, &options)?).map_err(|e| e.to_string())
    });
    let json = match result {
        Ok(Ok(json)) => json,
        Ok(Err(message)) => serde_json::json!({"error": message}).to_string(),
        Err(_) => serde_json::json!({"error": "native parser panicked"}).to_string(),
    };
    CString::new(json)
        .expect("JSON contains no raw NUL")
        .into_raw()
}

/// Release an analysis string returned by `rust_joern_analyze`.
///
/// # Safety
/// `pointer` must be null or an unreleased allocation from that function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_joern_free(pointer: *mut c_char) {
    if !pointer.is_null() {
        drop(unsafe { CString::from_raw(pointer) });
    }
}
