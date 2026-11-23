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
    let (tokens, diagnostics, _) = lex_preprocessed_with_macros_for_language(source, true, true);
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
    let source = "\\u00e9 a\\u00e9 \\U000000e9 a\\\nb a\\\r\nb +\\\n1 +\\\r\n2 \"\\\\n\" '\\\\'";
    let (tokens, diagnostics, _) = lex_preprocessed_with_macros_for_language(source, true, true);
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
        let (tokens, diagnostics, _) = lex_preprocessed_with_macros_for_language(source, true, cpp);
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
            "int", "f", "(", ")", ";", "int", "first", ";", "int", "API", ";", "int", "last", ";"
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
    let source = include_str!("../../tests/fixtures/unresolved-includes/includes_unresolved.c");
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
    let source = include_str!("../../tests/fixtures/unresolved-includes/includes_unresolved.c");
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
