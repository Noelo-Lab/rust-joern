use rust_joern::{
    analyze,
    graph::{Analysis, Options, PropertyGraph},
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("rust-joern: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut arguments = env::args().skip(1).peekable();
    if arguments.peek().is_none()
        || matches!(arguments.peek().map(String::as_str), Some("--help" | "-h"))
    {
        println!(
            "Usage: rust-joern analyze INPUT [--language c|cpp] [--preprocessed] [--data-flow] [--reaching-definitions] [--strict] [--output PATH] [--format json|dot] [--graph cfg|ddg|cpg]"
        );
        return Ok(());
    }
    if arguments.peek().map(String::as_str) == Some("--version") {
        println!("rust-joern {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if arguments.peek().map(String::as_str) == Some("analyze") {
        arguments.next();
    }
    let input = arguments.next().ok_or("missing input")?;
    let mut options = Options::default();
    let mut output = None;
    let mut format = "json".to_string();
    let mut graph = "cfg".to_string();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--language" => options.language = Some(arguments.next().ok_or("missing language")?),
            "--preprocessed" => options.preprocessed = true,
            "--data-flow" => options.data_flow = true,
            "--reaching-definitions" => options.reaching_definitions = true,
            "--strict" => options.strict = true,
            "--output" | "-o" => output = Some(arguments.next().ok_or("missing output path")?),
            "--format" => format = arguments.next().ok_or("missing output format")?,
            "--graph" => graph = arguments.next().ok_or("missing graph kind")?,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    if !matches!(format.as_str(), "json" | "dot") {
        return Err(format!("unknown format: {format}"));
    }
    if !matches!(graph.as_str(), "cfg" | "ddg" | "cpg") {
        return Err(format!("unknown graph: {graph}"));
    }
    if graph == "ddg" {
        options.data_flow = true;
    }
    let mut all = Analysis {
        schema_version: 1,
        functions: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut files = Vec::new();
    if input == "-" {
        use std::io::Read;
        let mut source = String::new();
        std::io::stdin()
            .read_to_string(&mut source)
            .map_err(|e| e.to_string())?;
        all = analyze(&source, "<stdin>.c", &options)?;
    } else {
        collect(Path::new(&input), &mut files).map_err(|e| e.to_string())?;
        files.sort();
        for path in files {
            let source =
                fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let mut result = analyze(&source, &path.to_string_lossy(), &options)?;
            all.functions.append(&mut result.functions);
            all.diagnostics.append(&mut result.diagnostics);
        }
    }
    let content = if format == "json" {
        serde_json::to_string(&all).map_err(|e| e.to_string())?
    } else {
        dot(&all, &graph)
    };
    if let Some(output) = output {
        fs::write(output, content).map_err(|e| e.to_string())?;
    } else {
        println!("{content}");
    }
    Ok(())
}

fn collect(path: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_file() {
        files.push(path.to_owned());
    } else if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_symlink() {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                collect(&path, files)?;
            } else if matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("c" | "h" | "cpp" | "cc" | "cxx" | "C" | "hpp" | "hh" | "hxx" | "i" | "ii")
            ) {
                files.push(path);
            }
        }
    } else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} does not exist", path.display()),
        ));
    }
    Ok(())
}

fn quote(text: &str) -> String {
    serde_json::to_string(text).unwrap()
}

fn graph_dot(output: &mut String, prefix: &str, graph: &PropertyGraph) {
    use std::fmt::Write;
    for node in &graph.nodes {
        let _ = writeln!(
            output,
            "{prefix}n{} [label={}];",
            node.id,
            quote(&format!("{}: {}", node.kind, node.code))
        );
    }
    for edge in &graph.edges {
        let _ = writeln!(
            output,
            "{prefix}n{} -> {prefix}n{} [label={}];",
            edge.source,
            edge.target,
            quote(
                &edge
                    .label
                    .as_ref()
                    .map(|v| format!("{}: {v}", edge.kind))
                    .unwrap_or_else(|| edge.kind.clone())
            )
        );
    }
}

fn dot(analysis: &Analysis, kind: &str) -> String {
    use std::fmt::Write;
    let mut output = String::from("digraph analysis {\n");
    for (i, function) in analysis.functions.iter().enumerate() {
        let prefix = format!("f{i}_");
        let _ = writeln!(
            output,
            "subgraph cluster_{i} {{ label={};",
            quote(&function.fullname)
        );
        match kind {
            "cfg" => {
                for block in &function.cfg.nodes {
                    let code = block
                        .statements
                        .iter()
                        .map(|&id| function.cpg.nodes[id as usize].code.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    let _ = writeln!(
                        output,
                        "{prefix}n{} [label={}, entry={}, exit={}];",
                        block.id,
                        quote(&code),
                        block.is_entrypoint,
                        block.is_exitpoint
                    );
                }
                for [source, target] in &function.cfg.edges {
                    let _ = writeln!(output, "{prefix}n{source} -> {prefix}n{target};");
                }
            }
            "ddg" => {
                if let Some(ddg) = &function.ddg {
                    graph_dot(&mut output, &prefix, ddg);
                }
            }
            _ => graph_dot(&mut output, &prefix, &function.cpg),
        }
        output.push_str("}\n");
    }
    output.push_str("}\n");
    output
}
