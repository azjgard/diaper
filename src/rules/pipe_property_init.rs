use std::path::Path;

use super::{AstCache, Rule, RuleViolation};

/// Rule: pipe flow functions must only set properties that are initialized
/// in the parent pipe's initial context object.
///
/// A "pipe flow function" is a file with a default export function that takes
/// a single `ctx` parameter and returns `{ ...ctx, someProp: value }`.
///
/// The pipe call site (in ../index.js or ../../index.js) looks like:
///   pipe({ propA: null, propB: '' }).flow(fn1).flow(fn2).run()
///
/// Any property set in the return `{ ...ctx, ... }` that isn't in the
/// pipe's initial object is a violation (+100 stink each).
pub struct PipePropertyInit;

const SCORE_PER_VIOLATION: u32 = 100;

impl Rule for PipePropertyInit {
    fn name(&self) -> &str {
        "pipe-property-init"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/pipe-property-init.md"
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, cache: &mut AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        let score = config.rule_score("pipe-property-init", SCORE_PER_VIOLATION);
        // Step 1: Is this a pipe flow function?
        let ctx_props = match find_ctx_spread_properties(tree.root_node(), source) {
            Some(props) if !props.is_empty() => props,
            _ => return vec![],
        };

        // Step 2: Find the pipe call site in ../index.js or ../../index.js
        let parent_dir = match path.parent() {
            Some(p) => p,
            None => return vec![],
        };

        // Check ../index.js and ../../index.js relative to the file
        let mut candidates = Vec::new();
        if let Some(one_up) = parent_dir.parent() {
            candidates.push(one_up.join("index.js"));
        }
        if let Some(two_up) = parent_dir.parent().and_then(|p| p.parent()) {
            candidates.push(two_up.join("index.js"));
        }

        let mut pipe_init_props: Option<Vec<String>> = None;
        let mut pipe_path: Option<String> = None;

        for candidate in &candidates {
            if let Some((idx_source, _idx_tree)) = cache.get_or_parse(candidate) {
                let idx_source = idx_source.clone();
                let re_tree = super::parse_js(&idx_source).unwrap();
                if let Some(props) = find_pipe_init_properties(re_tree.root_node(), &idx_source) {
                    pipe_init_props = Some(props);
                    pipe_path = Some(candidate.to_string_lossy().to_string());
                    break;
                }
            }
        }

        let init_props = match pipe_init_props {
            Some(props) => props,
            None => return vec![],
        };
        let pipe_location = pipe_path.unwrap_or_else(|| "parent index.js".to_string());

        // Step 3: Report violations for properties not in the pipe init
        let mut violations = Vec::new();
        for prop in &ctx_props {
            if !init_props.contains(prop) {
                violations.push(RuleViolation {
                    rule_name: self.name().to_string(),
                    doc_url: self.doc_url().to_string(),
                    score,
                    code_sample: format!("{{ ...ctx, {prop}: ... }}"),
                    fix_suggestion: format!("initialize \"{prop}\" in pipe constructor in {pipe_location}"),
                });
            }
        }

        violations
    }
}

/// Check if a file has a default export function with a single `ctx` parameter
/// that returns `{ ...ctx, prop: value }`. Returns the list of property names
/// being set (excluding the spread).
fn find_ctx_spread_properties(root: tree_sitter::Node, source: &str) -> Option<Vec<String>> {
    // Find default export
    let export = find_default_export_function(root, source)?;

    // Check it has a single parameter named "ctx"
    let params = export.child_by_field_name("parameters")?;
    let param_count = params.named_child_count();
    if param_count != 1 {
        return None;
    }
    let param = params.named_child(0)?;
    let param_name = &source[param.byte_range()];
    if param_name != "ctx" {
        return None;
    }

    // Find return statements with { ...ctx, ... } objects
    let mut all_props = Vec::new();
    collect_return_spread_props(export, source, &mut all_props);

    if all_props.is_empty() {
        return None;
    }

    // Deduplicate
    all_props.sort();
    all_props.dedup();
    Some(all_props)
}

/// Find the function node from a default export statement.
fn find_default_export_function<'a>(root: tree_sitter::Node<'a>, _source: &str) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "export_statement" {
            let has_default = child.children(&mut child.walk()).any(|c| c.kind() == "default");
            if !has_default {
                continue;
            }

            // Find the function declaration or expression inside
            let mut inner = child.walk();
            for c in child.children(&mut inner) {
                match c.kind() {
                    "function_declaration" | "function" => return Some(c),
                    _ => {
                        // Could be an arrow function in a variable or direct
                        if c.kind() == "arrow_function" {
                            return Some(c);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Recursively find return statements that return `{ ...ctx, prop: val }` objects.
/// Collects the property names (not the spread).
fn collect_return_spread_props(node: tree_sitter::Node, source: &str, props: &mut Vec<String>) {
    if node.kind() == "return_statement" {
        // Get the returned expression
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "object" {
                if object_has_ctx_spread(child, source) {
                    collect_object_property_names(child, source, props);
                }
            }
            // Handle parenthesized expression: return ({ ...ctx, prop: val })
            if child.kind() == "parenthesized_expression" {
                let mut inner = child.walk();
                for c in child.children(&mut inner) {
                    if c.kind() == "object" && object_has_ctx_spread(c, source) {
                        collect_object_property_names(c, source, props);
                    }
                }
            }
        }
    }

    // Also handle arrow functions with implicit return (no braces)
    // e.g. export default (ctx) => ({ ...ctx, prop: val })
    if node.kind() == "arrow_function" {
        if let Some(body) = node.child_by_field_name("body") {
            if body.kind() == "parenthesized_expression" {
                let mut cursor = body.walk();
                for child in body.children(&mut cursor) {
                    if child.kind() == "object" && object_has_ctx_spread(child, source) {
                        collect_object_property_names(child, source, props);
                        return; // Arrow implicit return — don't recurse further
                    }
                }
            }
            if body.kind() == "object" && object_has_ctx_spread(body, source) {
                collect_object_property_names(body, source, props);
                return;
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_return_spread_props(child, source, props);
    }
}

/// Check if an object literal has `...ctx` as a spread element.
fn object_has_ctx_spread(object: tree_sitter::Node, source: &str) -> bool {
    let mut cursor = object.walk();
    for child in object.children(&mut cursor) {
        if child.kind() == "spread_element" {
            let spread_text = &source[child.byte_range()];
            if spread_text.trim() == "...ctx" {
                return true;
            }
        }
    }
    false
}

/// Collect property names from an object literal (excluding spread elements).
fn collect_object_property_names(object: tree_sitter::Node, source: &str, props: &mut Vec<String>) {
    let mut cursor = object.walk();
    for child in object.children(&mut cursor) {
        if child.kind() == "pair" {
            if let Some(key) = child.child_by_field_name("key") {
                let key_text = &source[key.byte_range()];
                props.push(key_text.to_string());
            }
        }
        // Shorthand property: { someVar } is same as { someVar: someVar }
        if child.kind() == "shorthand_property_identifier" {
            let text = &source[child.byte_range()];
            props.push(text.to_string());
        }
    }
}

/// Find the initial properties in a pipe() call in the given file.
/// Looks for: pipe({ propA: ..., propB: ... }).flow(...)...
fn find_pipe_init_properties(root: tree_sitter::Node, source: &str) -> Option<Vec<String>> {
    let mut result = None;
    search_pipe_call(root, source, &mut result);
    result
}

/// Recursively search for a pipe() call and extract its initial object properties.
fn search_pipe_call(node: tree_sitter::Node, source: &str, result: &mut Option<Vec<String>>) {
    if result.is_some() {
        return;
    }

    // Look for call_expression where the function is "pipe"
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            let func_text = &source[func.byte_range()];
            if func_text == "pipe" {
                if let Some(args) = node.child_by_field_name("arguments") {
                    // First argument should be an object
                    if let Some(first_arg) = args.named_child(0) {
                        if first_arg.kind() == "object" {
                            let mut props = Vec::new();
                            collect_object_property_names(first_arg, source, &mut props);
                            // Also collect spread properties — shorthand
                            *result = Some(props);
                            return;
                        }
                    }
                }
            }
        }
    }

    // Also check member expressions — pipe({...}).flow(...) means
    // the pipe() call is inside a member_expression chain
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        search_pipe_call(child, source, result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse_js;
    use std::fs;

    // Helper: create a temp directory with flow function + index.js
    struct TestSetup {
        _dir: tempfile::TempDir,
        flow_path: std::path::PathBuf,
    }

    fn setup_test(flow_source: &str, index_source: &str) -> TestSetup {
        let dir = tempfile::tempdir().unwrap();

        // Create subdirectory for the flow function
        let sub = dir.path().join("steps");
        fs::create_dir(&sub).unwrap();

        let flow_path = sub.join("myStep.js");
        fs::write(&flow_path, flow_source).unwrap();

        // Write index.js in parent
        let index_path = dir.path().join("index.js");
        fs::write(&index_path, index_source).unwrap();

        TestSetup {
            _dir: dir,
            flow_path,
        }
    }

    fn setup_test_grandparent(flow_source: &str, index_source: &str) -> TestSetup {
        let dir = tempfile::tempdir().unwrap();

        // Create nested subdirectory for the flow function
        let sub1 = dir.path().join("steps");
        fs::create_dir(&sub1).unwrap();
        let sub2 = sub1.join("detail");
        fs::create_dir(&sub2).unwrap();

        let flow_path = sub2.join("myStep.js");
        fs::write(&flow_path, flow_source).unwrap();

        // Write index.js in grandparent
        let index_path = dir.path().join("index.js");
        fs::write(&index_path, index_source).unwrap();

        TestSetup {
            _dir: dir,
            flow_path,
        }
    }

    fn check_file(setup: &TestSetup) -> Vec<RuleViolation> {
        let source = fs::read_to_string(&setup.flow_path).unwrap();
        let tree = parse_js(&source).unwrap();
        let mut cache = AstCache::new();
        let config = crate::config::Config::default();
        PipePropertyInit.check(&source, &setup.flow_path, &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_uninitialized_property() {
        let setup = setup_test(
            r#"export default function myStep(ctx) {
                return { ...ctx, propA: 'value', propC: 'oops' };
            }"#,
            r#"pipe({ propA: null, propB: '' }).flow(myStep).run();"#,
        );
        let violations = check_file(&setup);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 100);
        assert!(violations[0].code_sample.contains("propC"));
    }

    #[test]
    fn test_multiple_uninitialized_properties() {
        let setup = setup_test(
            r#"export default function myStep(ctx) {
                return { ...ctx, bad1: 1, bad2: 2 };
            }"#,
            r#"pipe({ goodProp: null }).flow(myStep).run();"#,
        );
        let violations = check_file(&setup);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations.iter().map(|v| v.score).sum::<u32>(), 200);
    }

    // --- No violations ---

    #[test]
    fn test_all_properties_initialized() {
        let setup = setup_test(
            r#"export default function myStep(ctx) {
                return { ...ctx, propA: 'updated' };
            }"#,
            r#"pipe({ propA: null, propB: '' }).flow(myStep).run();"#,
        );
        let violations = check_file(&setup);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_not_a_flow_function_no_ctx() {
        let setup = setup_test(
            r#"export default function myStep(data) {
                return { ...data, propC: 'value' };
            }"#,
            r#"pipe({ propA: null }).flow(myStep).run();"#,
        );
        let violations = check_file(&setup);
        assert!(violations.is_empty()); // Not a flow function — param isn't "ctx"
    }

    #[test]
    fn test_not_a_flow_function_multiple_params() {
        let setup = setup_test(
            r#"export default function myStep(ctx, extra) {
                return { ...ctx, propC: 'value' };
            }"#,
            r#"pipe({ propA: null }).flow(myStep).run();"#,
        );
        let violations = check_file(&setup);
        assert!(violations.is_empty()); // Not a flow function — multiple params
    }

    #[test]
    fn test_no_default_export() {
        let setup = setup_test(
            r#"export function myStep(ctx) {
                return { ...ctx, propC: 'value' };
            }"#,
            r#"pipe({ propA: null }).flow(myStep).run();"#,
        );
        let violations = check_file(&setup);
        assert!(violations.is_empty()); // Not a default export
    }

    #[test]
    fn test_no_spread_ctx() {
        let setup = setup_test(
            r#"export default function myStep(ctx) {
                return { propA: 'value' };
            }"#,
            r#"pipe({ propA: null }).flow(myStep).run();"#,
        );
        let violations = check_file(&setup);
        assert!(violations.is_empty()); // No ...ctx spread
    }

    #[test]
    fn test_no_index_js() {
        // Flow function with no parent index.js
        let dir = tempfile::tempdir().unwrap();
        let flow_path = dir.path().join("myStep.js");
        fs::write(&flow_path, r#"export default function myStep(ctx) {
            return { ...ctx, propC: 'value' };
        }"#).unwrap();

        let source = fs::read_to_string(&flow_path).unwrap();
        let tree = parse_js(&source).unwrap();
        let mut cache = AstCache::new();
        let config = crate::config::Config::default();
        let violations = PipePropertyInit.check(&source, &flow_path, &tree, &mut cache, &config);
        assert!(violations.is_empty()); // No pipe call site found
    }

    // --- Grandparent index.js ---

    #[test]
    fn test_pipe_in_grandparent_index() {
        let setup = setup_test_grandparent(
            r#"export default function myStep(ctx) {
                return { ...ctx, propA: 'value', propC: 'oops' };
            }"#,
            r#"pipe({ propA: null, propB: '' }).flow(myStep).run();"#,
        );
        let violations = check_file(&setup);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].code_sample.contains("propC"));
    }

    // --- Arrow function flow ---

    #[test]
    fn test_arrow_function_flow() {
        let setup = setup_test(
            r#"export default (ctx) => ({ ...ctx, propA: 'value', propC: 'oops' });"#,
            r#"pipe({ propA: null }).flow(myStep).run();"#,
        );
        let violations = check_file(&setup);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].code_sample.contains("propC"));
    }

    #[test]
    fn test_arrow_function_with_body() {
        let setup = setup_test(
            r#"export default (ctx) => {
                const extra = compute();
                return { ...ctx, propA: extra, propC: 'oops' };
            };"#,
            r#"pipe({ propA: null }).flow(myStep).run();"#,
        );
        let violations = check_file(&setup);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].code_sample.contains("propC"));
    }

    // --- Pipe with runAsync ---

    #[test]
    fn test_pipe_with_run_async() {
        let setup = setup_test(
            r#"export default function myStep(ctx) {
                return { ...ctx, propC: 'oops' };
            }"#,
            r#"pipe({ propA: null }).flow(myStep).runAsync();"#,
        );
        let violations = check_file(&setup);
        assert_eq!(violations.len(), 1);
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let setup = setup_test(
            r#"export default function myStep(ctx) {
                return { ...ctx, propC: 'oops' };
            }"#,
            r#"pipe({ propA: null }).flow(myStep).run();"#,
        );
        let violations = check_file(&setup);
        assert_eq!(violations[0].rule_name, "pipe-property-init");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let setup = setup_test(
            r#"export default function myStep(ctx) {
                return { ...ctx, propC: 'oops' };
            }"#,
            r#"pipe({ propA: null }).flow(myStep).run();"#,
        );
        let violations = check_file(&setup);
        assert!(violations[0].doc_url.starts_with("https://"));
    }

    // --- Empty/edge cases ---

    #[test]
    fn test_empty_file() {
        let source = "";
        let tree = parse_js(source).unwrap();
        let mut cache = AstCache::new();
        let config = crate::config::Config::default();
        let violations = PipePropertyInit.check(source, Path::new("test.js"), &tree, &mut cache, &config);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_non_js_file() {
        let source = "const x = 1;";
        let tree = parse_js(source).unwrap();
        let mut cache = AstCache::new();
        let config = crate::config::Config::default();
        let violations = PipePropertyInit.check(source, Path::new("test.js"), &tree, &mut cache, &config);
        assert!(violations.is_empty());
    }
}
