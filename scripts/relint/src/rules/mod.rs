//! All lint rules.
//!
//! Each rule is a function that walks the syntax tree of one file and reports [`Diagnostic`]s.
//! Rules only see syntax: no name resolution, no types, and no macro expansion.

mod debug_formatted_error;
mod dyn_any_param;
mod error_name;
mod field_count;
mod inline_conversion;
mod log_sensitive_inline;
mod map_err_dropped;
mod map_err_to_string;
mod metric_total_suffix;
mod pyclass;
mod pymethods_str;
mod result_alias;
mod snapshot_options_default;
mod string_error;
mod todo_message;
mod tokio_runtime;
mod vertical_spacing;
mod zip_chain_method;

use ra_ap_syntax::{
    AstNode as _, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, T, TextRange, TextSize,
    ast::{self, HasArgList as _, HasName as _},
};

use crate::{diagnostic::Diagnostic, format_args::macro_args, source::SourceFile};

/// A lint rule.
pub struct Rule {
    /// The `snake_case` name used in output and in `relint: allow(…)` markers.
    pub name: &'static str,

    /// Report all violations in a file.
    pub check: fn(&SourceFile, &mut Vec<Diagnostic>),

    /// Also check Rust code blocks in markdown and doc comments?
    ///
    /// Docs snippets are often fragments or deliberately simplified,
    /// so rules about project structure (e.g. Tokio runtimes, pyclass parameters) do not apply.
    pub check_markdown: bool,

    /// Also check test code?
    ///
    /// Test code is items with `#[cfg(test)]`, `#[test]`, or similar, and test files such as
    /// `tests/*` and `tests.rs`.
    pub check_tests: bool,
}

/// Every rule, sorted by name.
pub const RULES: &[Rule] = &[
    result_alias::ANYHOW_RULE,
    debug_formatted_error::RULE,
    dyn_any_param::RULE,
    error_name::RULE,
    inline_conversion::RULE,
    log_sensitive_inline::RULE,
    map_err_dropped::RULE,
    map_err_to_string::RULE,
    metric_total_suffix::RULE,
    field_count::POSITIONAL_RULE,
    pyclass::EQ_RULE,
    pyclass::MODULE_RULE,
    pymethods_str::RULE,
    snapshot_options_default::RULE,
    string_error::RULE,
    todo_message::RULE,
    tokio_runtime::RULE,
    result_alias::TONIC_RULE,
    field_count::VARIANT_RULE,
    vertical_spacing::RULE,
    zip_chain_method::RULE,
];

/// Run the given rules on a file, including the code in the arguments of macro calls.
///
/// The parser does not expand macros, so the body of `cfg_select! { … }`, `tokio::select! { … }`,
/// or `macro_rules! m { … }`, and the arguments of `assert!(…)` or `vec![…]`, are only token trees.
/// We parse each `{…}` group in them as a function body, and each `(…)` or `[…]` group as the
/// arguments of a function call, run the rules on that too, and map the diagnostics back to the file.
pub fn check_file(file: &SourceFile, rules: &[&Rule]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in rules {
        (rule.check)(file, &mut diagnostics);
    }

    let macro_bodies = file.syntax().descendants().filter_map(|node| {
        if let Some(call) = ast::MacroCall::cast(node.clone()) {
            // `quote!` templates are full of `#var` interpolations, which do not parse as Rust.
            let is_quote = macro_path(&call)
                .is_some_and(|path| path.ends_with("quote") || path.ends_with("quote_spanned"));
            (!is_quote).then(|| call.token_tree()).flatten()
        } else if let Some(rules) = ast::MacroRules::cast(node.clone()) {
            rules.token_tree()
        } else {
            ast::MacroDef::cast(node).and_then(|def| def.body())
        }
    });
    for group in macro_bodies.flat_map(|body| {
        body.syntax()
            .descendants()
            .filter_map(ast::TokenTree::cast)
            .collect::<Vec<_>>()
    }) {
        let range = group.syntax().text_range();
        let text = &file.text[range];
        let (prefix, body, suffix) = match group.syntax().first_token().map(|token| token.kind()) {
            Some(T!['{']) => ("fn _()", text.to_owned(), ""),
            Some(T!['(']) => ("fn _(){f", text.to_owned(), ";}"),
            Some(T!['[']) => ("fn _(){f", format!("({})", &text[1..text.len() - 1]), ";}"),
            _ => continue,
        };
        let parsed = SourceFile::parse(file.path.clone(), format!("{prefix}{body}{suffix}"));
        let mut group_diagnostics = Vec::new();
        for rule in rules {
            (rule.check)(&parsed, &mut group_diagnostics);
        }
        diagnostics.extend(
            group_diagnostics
                .into_iter()
                .filter_map(|diagnostic| diagnostic.shifted(TextSize::of(prefix), range.start())),
        );
    }

    let test_ranges = test_ranges(file);
    diagnostics.retain(|diagnostic| {
        let rule = rules.iter().find(|rule| rule.name == diagnostic.rule);
        rule.is_none_or(|rule| rule.check_tests)
            || !test_ranges
                .iter()
                .any(|range| range.contains_inclusive(diagnostic.offset))
    });

    diagnostics.sort_by(|a, b| (a.offset, a.rule, &a.message).cmp(&(b.offset, b.rule, &b.message)));
    diagnostics.dedup();
    diagnostics
}

/// The parts of a file that are test code.
///
/// All of it for files under `tests/` or `benches/`, for `tests.rs`, `*_test.rs`, or `*_tests.rs`
/// (as in `#[cfg(test)] mod tests;`), and for files with `#![cfg(test)]`.
/// Otherwise the items with a test attribute (see [`is_test_attr`]).
fn test_ranges(file: &SourceFile) -> Vec<TextRange> {
    let file_name = file.path.rsplit('/').next().unwrap_or_default();
    let is_test_file = ["tests/", "benches/"]
        .iter()
        .any(|dir| file.path.starts_with(dir) || file.path.contains(&format!("/{dir}")))
        || file_name == "tests.rs"
        || file_name.ends_with("_test.rs")
        || file_name.ends_with("_tests.rs")
        || ast::HasAttrs::attrs(&file.tree).any(|attr| is_test_attr(&attr));
    if is_test_file {
        return vec![file.syntax().text_range()];
    }
    file.syntax()
        .descendants()
        .filter_map(ast::Item::cast)
        .filter(|item| ast::HasAttrs::attrs(item).any(|attr| is_test_attr(&attr)))
        .map(|item| item.syntax().text_range())
        .collect()
}

/// `#[test]`, `#[tokio::test(…)]`, `#[rstest]`, `#[cfg(test)]`, or `#[cfg(all(test, …))]`.
fn is_test_attr(attr: &ast::Attr) -> bool {
    let is_test_atom =
        |node: &SyntaxNode| node.kind() == SyntaxKind::CFG_ATOM && node.text() == "test";
    // The parser gives `cfg` its own syntax: `CFG_META`, with `CFG_ATOM` and `CFG_COMPOSITE`.
    if let Some(cfg) = attr
        .syntax()
        .children()
        .find(|node| node.kind() == SyntaxKind::CFG_META)
    {
        return cfg.children().any(|predicate| {
            is_test_atom(&predicate)
                || (predicate.kind() == SyntaxKind::CFG_COMPOSITE
                    && predicate
                        .first_token()
                        .is_some_and(|token| token.text() == "all")
                    && predicate.children().any(|arg| is_test_atom(&arg)))
        });
    }
    attr.path().is_some_and(|path| {
        let path = path_text(&path);
        path == "test" || path.ends_with("::test") || path == "rstest"
    })
}

/// For `.map_err(|name| body)`, the parameter name and the closure.
///
/// `None` for other calls, and for closures whose parameter is not a plain name (e.g. `|_|`).
fn map_err_closure(call: &ast::MethodCallExpr) -> Option<(String, ast::ClosureExpr)> {
    if call.name_ref()?.text() != "map_err" {
        return None;
    }
    let Some(ast::Expr::ClosureExpr(closure)) = call.arg_list()?.args().next() else {
        return None;
    };
    let params: Vec<ast::Param> = closure.param_list()?.params().collect();
    let [param] = params.as_slice() else {
        return None;
    };
    let Some(ast::Pat::IdentPat(pat)) = param.pat() else {
        return None;
    };
    Some((pat.name()?.text().to_string(), closure))
}

/// The text of a path with all whitespace and comments removed, e.g. `tokio::runtime::Runtime::new`.
fn path_text(path: &ast::Path) -> String {
    path.syntax()
        .descendants_with_tokens()
        .filter_map(ra_ap_syntax::NodeOrToken::into_token)
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_owned())
        .collect()
}

/// The path of a macro call, e.g. `re_log::warn` for `re_log::warn!(…)` or `::re_log::warn!(…)`.
fn macro_path(call: &ast::MacroCall) -> Option<String> {
    call.path()
        .map(|path| path_text(&path).trim_start_matches("::").to_owned())
}

/// A macro invocation.
pub struct MacroInvocation {
    /// E.g. `re_log::warn` for `re_log::warn!(…)`.
    pub path: String,

    /// The arguments.
    pub token_tree: ast::TokenTree,

    /// Where the invocation starts.
    pub offset: TextSize,
}

/// All macro invocations in a file, including those nested in the arguments of other macros.
///
/// The parser does not expand macros, so `format!(…)` inside `assert_eq!(…)` is only tokens,
/// not an [`ast::MacroCall`]. We find those by looking for `path!(…)` in the token trees.
fn macro_invocations(file: &SourceFile) -> Vec<MacroInvocation> {
    let mut invocations = Vec::new();
    for call in file.syntax().descendants().filter_map(ast::MacroCall::cast) {
        if let (Some(path), Some(token_tree)) = (macro_path(&call), call.token_tree()) {
            collect_nested_invocations(&token_tree, &mut invocations);
            // Not the start of `call`, which includes its attributes and doc comments.
            let offset = call
                .path()
                .map_or(call.syntax().text_range().start(), |path| {
                    path.syntax().text_range().start()
                });
            invocations.push(MacroInvocation {
                path,
                token_tree,
                offset,
            });
        }
    }
    invocations
}

fn collect_nested_invocations(token_tree: &ast::TokenTree, invocations: &mut Vec<MacroInvocation>) {
    let children: Vec<SyntaxElement> = token_tree
        .syntax()
        .children_with_tokens()
        .filter(|element| !element.kind().is_trivia())
        .collect();
    for (index, child) in children.iter().enumerate() {
        let Some(nested) = child.as_node().cloned().and_then(ast::TokenTree::cast) else {
            continue;
        };
        if 2 <= index && children[index - 1].kind() == T![!] {
            let bang = index - 1;
            let is_colon = |i: usize| children[i].kind() == T![:];
            // Walk back over `a::b::c`, with an optional leading `::`.
            let mut start = bang;
            while 0 < start && children[start - 1].kind() == SyntaxKind::IDENT {
                start -= 1;
                if 2 <= start && is_colon(start - 1) && is_colon(start - 2) {
                    start -= 2;
                } else {
                    break;
                }
            }
            if start < bang
                && children[start..bang]
                    .iter()
                    .any(|c| c.kind() == SyntaxKind::IDENT)
            {
                let path: String = children[start..bang]
                    .iter()
                    .filter_map(SyntaxElement::as_token)
                    .map(SyntaxToken::text)
                    .collect();
                let path = path.trim_start_matches("::").to_owned();
                invocations.push(MacroInvocation {
                    path,
                    token_tree: nested.clone(),
                    offset: children[start].text_range().start(),
                });
            }
        }
        collect_nested_invocations(&nested, invocations);
    }
}

/// The token tree of an attribute such as `#[pyclass(eq, frozen)]`.
fn attr_token_tree(attr: &ast::Attr) -> Option<ast::TokenTree> {
    attr.syntax().descendants().find_map(ast::TokenTree::cast)
}

/// Is this a variable name we use for errors, such as `err` or `parse_err` (but not `stderr`)?
fn is_error_ident(name: &str) -> bool {
    name == "err" || name.ends_with("_err")
}

/// `Result` methods whose single-parameter closure receives the error.
///
/// The `Option` methods of the same names take closures without parameters.
const ERROR_CLOSURE_METHODS: &[&str] = &["inspect_err", "map_err", "or_else", "unwrap_or_else"];

/// The variables bound to an error under `root`: in `Err(x)` patterns,
/// and the parameter of closures such as `.map_err(|x| …)`.
fn error_bindings(root: &SyntaxNode) -> Vec<ast::IdentPat> {
    let mut bindings = Vec::new();
    for node in root.descendants() {
        if let Some(pat) = ast::TupleStructPat::cast(node.clone()) {
            let is_err = pat
                .path()
                .and_then(|path| path.segment())
                .and_then(|segment| segment.name_ref())
                .is_some_and(|name| name.text() == "Err");
            let fields: Vec<ast::Pat> = pat.fields().collect();
            if is_err && let [ast::Pat::IdentPat(binding)] = fields.as_slice() {
                bindings.push(binding.clone());
            }
        } else if let Some(call) = ast::MethodCallExpr::cast(node) {
            let is_error_method = call
                .name_ref()
                .is_some_and(|name| ERROR_CLOSURE_METHODS.contains(&name.text().as_str()));
            let Some(ast::Expr::ClosureExpr(closure)) =
                call.arg_list().and_then(|args| args.args().next())
            else {
                continue;
            };
            let params: Vec<ast::Param> = closure
                .param_list()
                .map(|list| list.params().collect())
                .unwrap_or_default();
            if is_error_method
                && let [param] = params.as_slice()
                && let Some(ast::Pat::IdentPat(binding)) = param.pat()
            {
                bindings.push(binding);
            }
        }
    }
    bindings
}

#[cfg(test)]
pub mod tests {
    use crate::{source::SourceFile, suppression};

    /// Run all rules on `src`, and return `"line: rule"` for each remaining diagnostic.
    pub fn lint(src: &str) -> Vec<String> {
        let file = SourceFile::parse("test.rs".to_owned(), src.to_owned());
        let rule_names: Vec<&str> = super::RULES.iter().map(|rule| rule.name).collect();
        let rules: Vec<&super::Rule> = super::RULES.iter().collect();
        let diagnostics = super::check_file(&file, &rules);
        let mut result: Vec<(usize, String)> =
            suppression::apply_markers(&file, diagnostics, &rule_names, &[])
                .into_iter()
                .map(|diagnostic| (file.line(diagnostic.offset) + 1, diagnostic.rule.to_owned()))
                .collect();
        result.sort();
        result
            .into_iter()
            .map(|(line, rule)| format!("{line}: {rule}"))
            .collect()
    }

    /// Like [`lint`], but only keep the diagnostics of one rule.
    pub fn lint_rule(rule: &str, src: &str) -> Vec<String> {
        lint(src)
            .into_iter()
            .filter(|diagnostic| diagnostic.ends_with(&format!(": {rule}")))
            .collect()
    }

    #[test]
    fn macro_bodies() {
        let src = r"cfg_select! {
    unix => {
        x.map_err(|e| e);
    }
}
";
        assert_eq!(lint(src), vec!["3: error_name"]);

        let src = r"fn f() {
    tokio::select! {
        r = fut => match r {
            Err(e) => {}
        },
    }
}
";
        assert_eq!(lint(src), vec!["4: error_name"]);

        let src = r#"fn f() {
    m!({
        format!("{err:?}");
    });
}
"#;
        assert_eq!(
            lint(src),
            vec!["3: debug_formatted_error"],
            "found once, not once per parse"
        );

        let src = r"fn f() {
    quote! {
        #docs
        struct #name;
    };
}
";
        assert_eq!(
            lint(src),
            Vec::<String>::new(),
            "`quote!` templates are not parsed"
        );
    }

    #[test]
    fn rules_are_sorted_and_unique() {
        let names: Vec<&str> = super::RULES.iter().map(|rule| rule.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted);
    }
}
