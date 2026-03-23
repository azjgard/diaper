use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: Sequelize query results should use .get({ plain: true }) to convert
/// model instances to plain objects. This applies to findOne/findByPk results
/// and array mapping of findAll results.
pub struct SequelizePlainGet;

const SCORE_PER_VIOLATION: u32 = 25;

impl Rule for SequelizePlainGet {
    fn name(&self) -> &str {
        "sequelize-plain-get"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/sequelize-plain-get.md"
    }

    fn description(&self) -> &str {
        "Sequelize results should use .get({ plain: true })"
    }

    fn default_score(&self) -> u32 {
        SCORE_PER_VIOLATION
    }

    fn examples(&self) -> (&[&str], &[&str]) {
        (
            &[".then((model) => ({ ...ctx, model }))"],
            &[".then((model) => ({ ...ctx, model: model?.get({ plain: true }) }))"],
        )
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        if super::is_excluded_file(path) {
            return vec![];
        }

        // Only applies to files in queries/ directories
        let path_str = path.to_string_lossy();
        if !path_str.contains("/queries/") {
            return vec![];
        }

        let score = config.rule_score("sequelize-plain-get", SCORE_PER_VIOLATION);
        let mut violations = Vec::new();

        find_violations(tree.root_node(), source, &mut violations, self, score);
        violations
    }
}

/// Find .then() callbacks that don't use .get({ plain: true }).
fn find_violations(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &SequelizePlainGet,
    score: u32,
) {
    // Look for call expressions that might be .then() on Sequelize queries
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            if func.kind() == "member_expression" {
                if let Some(prop) = func.child_by_field_name("property") {
                    let prop_name = &source[prop.byte_range()];
                    if prop_name == "then" {
                        // Check the object being called .then() on
                        if let Some(obj) = func.child_by_field_name("object") {
                            if is_sequelize_query(obj, source) {
                                // Check the callback argument
                                if let Some(args) = node.child_by_field_name("arguments") {
                                    check_then_callback(args, source, violations, rule, score, obj);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_violations(child, source, violations, rule, score);
    }
}

/// Check if a node is a Sequelize query call (findAll, findOne, etc.)
fn is_sequelize_query(node: tree_sitter::Node, source: &str) -> bool {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            if func.kind() == "member_expression" {
                if let Some(prop) = func.child_by_field_name("property") {
                    let method = &source[prop.byte_range()];
                    return matches!(method, "findAll" | "findOne" | "findByPk" | "findOrCreate" | "findAndCountAll");
                }
            }
        }
    }
    // Check if it's a chain (e.g., Model.findAll({}).then(...).then(...))
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            if func.kind() == "member_expression" {
                if let Some(obj) = func.child_by_field_name("object") {
                    return is_sequelize_query(obj, source);
                }
            }
        }
    }
    false
}

/// Check the .then() callback to see if it properly uses .get({ plain: true })
fn check_then_callback(
    args: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &SequelizePlainGet,
    score: u32,
    query_node: tree_sitter::Node,
) {
    let mut cursor = args.walk();
    for child in args.children(&mut cursor) {
        if child.kind() == "arrow_function" || child.kind() == "function" {
            // Get the parameter name(s)
            let params = get_callback_params(child, source);
            if params.is_empty() {
                continue;
            }

            // Determine if this is a findAll (array) or findOne (single) query
            let is_array_query = is_find_all_query(query_node, source);

            // Check the callback body
            let body = match child.child_by_field_name("body") {
                Some(b) => b,
                None => continue,
            };

            let body_text = &source[body.byte_range()];

            // Check if the callback properly handles the result
            for param in &params {
                if is_array_query {
                    // For arrays, should use .map(x => x.get({ plain: true }))
                    if body_text.contains(param) && !uses_map_plain_get(body_text, param) {
                        // Check if the param is used in the return without transformation
                        if uses_param_without_plain_get(body, source, param) {
                            let line = source.lines().nth(child.start_position().row).unwrap_or("");
                            let singular = singularize(param);
                            violations.push(RuleViolation {
                                rule_name: rule.name().to_string(),
                                doc_url: rule.doc_url().to_string(),
                                score,
                                code_sample: line.trim().to_string(),
                                fix_suggestion: format!("use {param}.map(({singular}) => {singular}.get({{ plain: true }}))"),
                            });
                        }
                    }
                } else {
                    // For single results, should use .get({ plain: true }) or ?.get({ plain: true })
                    if body_text.contains(param) && !uses_single_plain_get(body_text, param) {
                        // Check if the param is used in the return without transformation
                        if uses_param_without_plain_get(body, source, param) {
                            let line = source.lines().nth(child.start_position().row).unwrap_or("");
                            violations.push(RuleViolation {
                                rule_name: rule.name().to_string(),
                                doc_url: rule.doc_url().to_string(),
                                score,
                                code_sample: line.trim().to_string(),
                                fix_suggestion: format!("use {param}?.get({{ plain: true }}) or check existence before calling .get()"),
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Get parameter names from a callback function
fn get_callback_params(node: tree_sitter::Node, source: &str) -> Vec<String> {
    let mut params = Vec::new();

    // For arrow functions, parameters might be in "parameters" field or the first child
    if let Some(param_node) = node.child_by_field_name("parameter") {
        // Single parameter arrow function: (x) => ...
        params.push(source[param_node.byte_range()].to_string());
    } else if let Some(params_node) = node.child_by_field_name("parameters") {
        // Multiple parameters: (a, b) => ...
        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            if child.kind() == "identifier" {
                params.push(source[child.byte_range()].to_string());
            }
        }
    }

    params
}

/// Simple singularization: removes trailing 's' or 'es' from a word.
fn singularize(word: &str) -> String {
    if word.ends_with("ies") {
        // e.g., "companies" -> "company"
        format!("{}y", &word[..word.len() - 3])
    } else if word.ends_with("ses") || word.ends_with("xes") || word.ends_with("ches") || word.ends_with("shes") {
        // e.g., "buses" -> "bus", "boxes" -> "box"
        word[..word.len() - 2].to_string()
    } else if word.ends_with("s") && !word.ends_with("ss") {
        // e.g., "users" -> "user", but not "class" -> "clas"
        word[..word.len() - 1].to_string()
    } else {
        word.to_string()
    }
}

/// Check if this is a findAll/findAndCountAll query (returns array)
fn is_find_all_query(node: tree_sitter::Node, source: &str) -> bool {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            if func.kind() == "member_expression" {
                if let Some(prop) = func.child_by_field_name("property") {
                    let method = &source[prop.byte_range()];
                    return matches!(method, "findAll" | "findAndCountAll");
                }
            }
        }
    }
    false
}

/// Check if body uses .map with .get({ plain: true })
fn uses_map_plain_get(body_text: &str, _param: &str) -> bool {
    // Look for patterns like:
    // - param.map((x) => x.get({ plain: true }))
    // - param.map(x => x.get({ plain: true }))
    body_text.contains(".map(") && body_text.contains(".get({") && body_text.contains("plain: true")
}

/// Check if body uses .get({ plain: true }) for single results
fn uses_single_plain_get(body_text: &str, param: &str) -> bool {
    // Look for patterns like:
    // - param.get({ plain: true })
    // - param?.get({ plain: true })
    let get_pattern = format!("{param}.get({{");
    let optional_get_pattern = format!("{param}?.get({{");

    (body_text.contains(&get_pattern) || body_text.contains(&optional_get_pattern))
        && body_text.contains("plain: true")
}

/// Check if a parameter is used in return without .get({ plain: true })
fn uses_param_without_plain_get(body: tree_sitter::Node, source: &str, param: &str) -> bool {
    // Look for the parameter being spread or assigned without transformation
    let body_text = &source[body.byte_range()];

    // Check for direct spread: { ...ctx, result: param }
    // or shorthand: { ...ctx, param }
    let spread_pattern = format!("...ctx, {param}");
    let assign_pattern = format!(": {param}");
    let shorthand_pattern = format!(", {param} }}");
    let shorthand_pattern2 = format!(", {param},");

    // If body already uses .get({ plain: true }), it's fine
    if uses_single_plain_get(body_text, param) || uses_map_plain_get(body_text, param) {
        return false;
    }

    // Check for patterns that indicate the param is being returned without transformation
    body_text.contains(&spread_pattern)
        || body_text.contains(&assign_pattern)
        || body_text.contains(&shorthand_pattern)
        || body_text.contains(&shorthand_pattern2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse_js;

    fn check(source: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        SequelizePlainGet.check(source, Path::new("src/queries/get-users/index.js"), &tree, &mut cache, &config)
    }

    fn check_with_path(source: &str, path: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        SequelizePlainGet.check(source, Path::new(path), &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_findall_without_map_plain_get() {
        let source = r#"export default (ctx) => {
            return Model.findAll({ where: {} }).then((models) => ({ ...ctx, models }));
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 25);
        // Should suggest: models.map((model) => model.get({ plain: true }))
        assert!(violations[0].fix_suggestion.contains("models.map((model) => model.get("));
    }

    #[test]
    fn test_findone_without_plain_get() {
        let source = r#"export default (ctx) => {
            return Model.findOne({ where: { id: 1 } }).then((model) => ({ ...ctx, model }));
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("?.get("));
    }

    #[test]
    fn test_findbypk_without_plain_get() {
        let source = r#"export default (ctx) => {
            return Model.findByPk(ctx.id).then((model) => ({ ...ctx, model }));
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_findone_with_key_assign() {
        let source = r#"export default (ctx) => {
            return Model.findOne({ where: { id: 1 } }).then((result) => ({ ...ctx, user: result }));
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    // --- No violations ---

    #[test]
    fn test_findall_with_map_plain_get() {
        let source = r#"export default (ctx) => {
            return Model.findAll({ where: {} }).then((models) => ({
                ...ctx,
                models: models.map((m) => m.get({ plain: true }))
            }));
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_findone_with_optional_plain_get() {
        let source = r#"export default (ctx) => {
            return Model.findOne({ where: { id: 1 } }).then((model) => ({
                ...ctx,
                model: model?.get({ plain: true })
            }));
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_findone_with_plain_get() {
        let source = r#"export default (ctx) => {
            return Model.findOne({ where: { id: 1 } }).then((model) => ({
                ...ctx,
                model: model.get({ plain: true })
            }));
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_findone_with_existence_check() {
        let source = r#"export default (ctx) => {
            return Model.findOne({ where: { id: 1 } }).then((model) => {
                if (!model) throw new Error('Not found');
                return { ...ctx, model: model.get({ plain: true }) };
            });
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_not_in_queries_directory() {
        let source = r#"export default (ctx) => {
            return Model.findAll({ where: {} }).then((models) => ({ ...ctx, models }));
        };"#;
        let violations = check_with_path(source, "src/steps/process-data/index.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_spec_file_excluded() {
        let source = r#"export default (ctx) => {
            return Model.findAll({ where: {} }).then((models) => ({ ...ctx, models }));
        };"#;
        let violations = check_with_path(source, "src/queries/get-users/index.spec.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_non_sequelize_then() {
        let source = r#"export default (ctx) => {
            return fetchData().then((data) => ({ ...ctx, data }));
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    // --- Edge cases ---

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_then_callback() {
        let source = r#"export default (ctx) => {
            return Model.findAll({ where: {} });
        };"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_chained_then_calls() {
        // Only the first .then() that handles the query result should be checked
        let source = r#"export default (ctx) => {
            return Model.findAll({ where: {} })
                .then((models) => ({ ...ctx, models }))
                .then((ctx) => ctx);
        };"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let source = r#"export default (ctx) => {
            return Model.findAll({}).then((models) => ({ ...ctx, models }));
        };"#;
        let violations = check(source);
        assert_eq!(violations[0].rule_name, "sequelize-plain-get");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let source = r#"export default (ctx) => {
            return Model.findAll({}).then((models) => ({ ...ctx, models }));
        };"#;
        let violations = check(source);
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
