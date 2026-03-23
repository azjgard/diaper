use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::{AstCache, Rule, RuleViolation};

/// Rule: Sequelize model queries (findOne, findAll, findByPk) must specify
/// explicit `attributes` arrays. "id" must always be included. Any fields
/// used in `where` or `order` clauses must also be in `attributes`.
pub struct RequireQueryAttributes;

const SCORE_PER_VIOLATION: u32 = 10;
const QUERY_METHODS: &[&str] = &["findOne", "findAll", "findByPk"];

impl Rule for RequireQueryAttributes {
    fn name(&self) -> &str {
        "require-query-attributes"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/require-query-attributes.md"
    }

    fn description(&self) -> &str {
        "Sequelize queries missing explicit attributes"
    }

    fn default_score(&self) -> u32 {
        SCORE_PER_VIOLATION
    }

    fn examples(&self) -> (&[&str], &[&str]) {
        (
            &["Model.findAll({ order: [['name', 'desc']] })"],
            &["Model.findAll({ order: [['name', 'desc']], attributes: ['id', 'name'] })"],
        )
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, cache: &mut AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        if super::is_excluded_file(path) {
            return vec![];
        }

        let score = config.rule_score("require-query-attributes", SCORE_PER_VIOLATION);
        let root = tree.root_node();

        let models = collect_model_imports(root, source);
        if models.is_empty() {
            return vec![];
        }

        let external_attrs = collect_external_attributes(root, source, path, cache);

        let mut violations = Vec::new();
        find_query_violations(root, source, &models, &external_attrs, &mut violations, self, score);
        violations
    }
}

/// Collect model names imported from "#models" or "#models/*".
fn collect_model_imports(root: tree_sitter::Node, source: &str) -> HashSet<String> {
    let mut models = HashSet::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() != "import_statement" {
            continue;
        }

        // Find the import source string
        let import_source = match find_import_source(child, source) {
            Some(s) => s,
            None => continue,
        };

        if !import_source.starts_with("#models") {
            continue;
        }

        // Extract destructured names
        let mut inner = child.walk();
        for c in child.children(&mut inner) {
            if c.kind() == "import_clause" {
                collect_named_imports(c, source, &mut models);
            }
        }
    }

    models
}

/// Find the source string of an import statement.
fn find_import_source<'a>(node: tree_sitter::Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "string" {
            return extract_string_content(child, source);
        }
    }
    None
}

/// Collect named import specifiers from an import clause.
fn collect_named_imports(node: tree_sitter::Node, source: &str, models: &mut HashSet<String>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "named_imports" {
            let mut inner = child.walk();
            for spec in child.children(&mut inner) {
                if spec.kind() == "import_specifier" {
                    let name = &source[spec.byte_range()];
                    // Handle "Foo as Bar" — use the local name
                    let local = name.split(" as ").last().unwrap_or(name).trim();
                    models.insert(local.to_string());
                }
            }
        }
    }
}

/// Load external attributes from ./attributes/index.js if imported.
fn collect_external_attributes(
    root: tree_sitter::Node,
    source: &str,
    path: &Path,
    cache: &mut AstCache,
) -> HashMap<String, Vec<String>> {
    let mut result = HashMap::new();

    // Check if file imports from "./attributes"
    let mut cursor = root.walk();
    let mut has_attributes_import = false;
    for child in root.children(&mut cursor) {
        if child.kind() == "import_statement" {
            if let Some(src) = find_import_source(child, source) {
                if src == "./attributes" {
                    has_attributes_import = true;
                    break;
                }
            }
        }
    }

    if !has_attributes_import {
        return result;
    }

    let attr_path = match path.parent() {
        Some(p) => p.join("attributes/index.js"),
        None => return result,
    };

    if let Some((attr_source, _attr_tree)) = cache.get_or_parse(&attr_path) {
        let attr_source = attr_source.clone();
        let re_tree = match super::parse_js(&attr_source) {
            Some(t) => t,
            None => return result,
        };
        extract_default_export_object(re_tree.root_node(), &attr_source, &mut result);
    }

    result
}

/// Extract key-value pairs from a default export object.
fn extract_default_export_object(
    root: tree_sitter::Node,
    source: &str,
    result: &mut HashMap<String, Vec<String>>,
) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "export_statement" {
            let has_default = child.children(&mut child.walk()).any(|c| c.kind() == "default");
            if !has_default {
                continue;
            }
            let mut inner = child.walk();
            for c in child.children(&mut inner) {
                if c.kind() == "object" {
                    let mut obj_cursor = c.walk();
                    for pair in c.children(&mut obj_cursor) {
                        if pair.kind() == "pair" {
                            if let (Some(key), Some(value)) = (
                                pair.child_by_field_name("key"),
                                pair.child_by_field_name("value"),
                            ) {
                                let key_text = &source[key.byte_range()];
                                if value.kind() == "array" {
                                    let attrs = extract_string_array(value, source);
                                    result.insert(key_text.to_string(), attrs);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Walk the AST looking for model query calls.
fn find_query_violations(
    node: tree_sitter::Node,
    source: &str,
    models: &HashSet<String>,
    external_attrs: &HashMap<String, Vec<String>>,
    violations: &mut Vec<RuleViolation>,
    rule: &RequireQueryAttributes,
    score: u32,
) {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            if func.kind() == "member_expression" {
                if let (Some(object), Some(property)) = (
                    func.child_by_field_name("object"),
                    func.child_by_field_name("property"),
                ) {
                    let obj_text = &source[object.byte_range()];
                    let method = &source[property.byte_range()];

                    if models.contains(obj_text) && QUERY_METHODS.contains(&method) {
                        if let Some(args) = node.child_by_field_name("arguments") {
                            // findByPk takes (id, opts) — options is second arg
                            // findOne/findAll takes (opts) — options is first arg
                            let opts_index = if method == "findByPk" { 1 } else { 0 };

                            if let Some(opts) = args.named_child(opts_index) {
                                if opts.kind() == "object" {
                                    validate_query_object(opts, source, external_attrs, violations, rule, score, node, obj_text);
                                }
                            } else if method != "findByPk" {
                                let line = source.lines().nth(node.start_position().row).unwrap_or("");
                                violations.push(RuleViolation {
                                    rule_name: rule.name().to_string(),
                                    doc_url: rule.doc_url().to_string(),
                                    score,
                                    code_sample: line.trim().to_string(),
                                    fix_suggestion: format!("specify explicit attributes including 'id' for {obj_text}"),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_query_violations(child, source, models, external_attrs, violations, rule, score);
    }
}

/// Validate a query options object for attributes, where, order, include.
fn validate_query_object(
    obj: tree_sitter::Node,
    source: &str,
    external_attrs: &HashMap<String, Vec<String>>,
    violations: &mut Vec<RuleViolation>,
    rule: &RequireQueryAttributes,
    score: u32,
    call_node: tree_sitter::Node,
    model_name: &str,
) {
    let mut attributes_node = None;
    let mut where_node = None;
    let mut order_node = None;
    let mut include_node = None;

    let mut cursor = obj.walk();
    for child in obj.children(&mut cursor) {
        if child.kind() == "pair" {
            if let Some(key) = child.child_by_field_name("key") {
                let key_text = &source[key.byte_range()];
                match key_text {
                    "attributes" => attributes_node = child.child_by_field_name("value"),
                    "where" => where_node = child.child_by_field_name("value"),
                    "order" => order_node = child.child_by_field_name("value"),
                    "include" => include_node = child.child_by_field_name("value"),
                    _ => {}
                }
            }
        }
    }

    let line = source.lines().nth(call_node.start_position().row).unwrap_or("");

    // No attributes at all
    let attrs = match attributes_node {
        None => {
            let mut missing = vec!["id".to_string()];

            if let Some(w) = where_node {
                missing.extend(extract_where_keys(w, source));
            }
            if let Some(o) = order_node {
                missing.extend(extract_order_columns(o, source));
            }
            missing.sort();
            missing.dedup();

            let missing_str = missing.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(", ");
            violations.push(RuleViolation {
                rule_name: rule.name().to_string(),
                doc_url: rule.doc_url().to_string(),
                score,
                code_sample: line.trim().to_string(),
                fix_suggestion: format!("specify {missing_str} in explicit queried attributes for {model_name}"),
            });

            // Still check includes
            if let Some(inc) = include_node {
                validate_includes(inc, source, external_attrs, violations, rule, score);
            }
            return;
        }
        Some(n) => resolve_attributes(n, source, external_attrs),
    };

    let attrs_set: HashSet<&str> = attrs.iter().map(|s| s.as_str()).collect();
    let mut missing: Vec<String> = Vec::new();

    // Check "id" is present
    if !attrs_set.contains("id") {
        missing.push("id".to_string());
    }

    // Check where keys
    if let Some(w) = where_node {
        for key in extract_where_keys(w, source) {
            if !attrs_set.contains(key.as_str()) {
                missing.push(key);
            }
        }
    }

    // Check order columns
    if let Some(o) = order_node {
        for col in extract_order_columns(o, source) {
            if !attrs_set.contains(col.as_str()) {
                missing.push(col);
            }
        }
    }

    missing.sort();
    missing.dedup();

    if !missing.is_empty() {
        let missing_str = missing.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(", ");
        violations.push(RuleViolation {
            rule_name: rule.name().to_string(),
            doc_url: rule.doc_url().to_string(),
            score,
            code_sample: line.trim().to_string(),
            fix_suggestion: format!("specify {missing_str} in explicit queried attributes for {model_name}"),
        });
    }

    // Check includes
    if let Some(inc) = include_node {
        validate_includes(inc, source, external_attrs, violations, rule, score);
    }
}

/// Validate each model in an include array.
fn validate_includes(
    include_node: tree_sitter::Node,
    source: &str,
    external_attrs: &HashMap<String, Vec<String>>,
    violations: &mut Vec<RuleViolation>,
    rule: &RequireQueryAttributes,
    score: u32,
) {
    if include_node.kind() != "array" {
        return;
    }

    let mut cursor = include_node.walk();
    for child in include_node.children(&mut cursor) {
        if child.kind() == "object" {
            let include_model = extract_include_model_name(child, source)
                .unwrap_or("unknown");
            validate_query_object(child, source, external_attrs, violations, rule, score, child, include_model);
        }
    }
}

/// Extract the model name from an include object's `model: ModelName` pair.
fn extract_include_model_name<'a>(obj: tree_sitter::Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = obj.walk();
    for child in obj.children(&mut cursor) {
        if child.kind() == "pair" {
            if let Some(key) = child.child_by_field_name("key") {
                if &source[key.byte_range()] == "model" {
                    if let Some(value) = child.child_by_field_name("value") {
                        return Some(&source[value.byte_range()]);
                    }
                }
            }
        }
    }
    None
}

/// Resolve an attributes value to a list of attribute names.
/// Handles array literals (["id", "name"]) and member expressions
/// (attributes.competition) by looking up the key in external_attrs.
fn resolve_attributes(
    node: tree_sitter::Node,
    source: &str,
    external_attrs: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    if node.kind() == "array" {
        return extract_string_array(node, source);
    }

    // Handle member expressions like `attributes.competition`
    if node.kind() == "member_expression" {
        if let Some(prop) = node.child_by_field_name("property") {
            let key = &source[prop.byte_range()];
            if let Some(attrs) = external_attrs.get(key) {
                return attrs.clone();
            }
        }
    }

    Vec::new()
}

/// Extract string values from an array node.
fn extract_string_array(node: tree_sitter::Node, source: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "string" {
            if let Some(content) = extract_string_content(child, source) {
                strings.push(content.to_string());
            }
        }
    }
    strings
}

/// Extract keys from a where object, skipping computed properties like [Op.and].
fn extract_where_keys(node: tree_sitter::Node, source: &str) -> Vec<String> {
    let mut keys = Vec::new();
    if node.kind() != "object" {
        return keys;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "pair" {
            if let Some(key) = child.child_by_field_name("key") {
                // Skip computed properties like [Op.and]
                if key.kind() == "computed_property_name" {
                    continue;
                }
                let key_text = &source[key.byte_range()];
                keys.push(key_text.to_string());
            }
        }
    }

    keys
}

/// Extract column names from an order array (first element of each inner array).
fn extract_order_columns(node: tree_sitter::Node, source: &str) -> Vec<String> {
    let mut columns = Vec::new();
    if node.kind() != "array" {
        return columns;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "array" {
            // First element of inner array is the column name
            if let Some(first) = child.named_child(0) {
                if first.kind() == "string" {
                    if let Some(content) = extract_string_content(first, source) {
                        columns.push(content.to_string());
                    }
                }
            }
        }
    }

    columns
}

/// Extract the content of a string node (without quotes).
fn extract_string_content<'a>(node: tree_sitter::Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "string_fragment" {
            return Some(&source[child.byte_range()]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse_js;
    use std::fs;

    fn check(source: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        RequireQueryAttributes.check(source, Path::new("src/queries/foo.js"), &tree, &mut cache, &config)
    }

    fn check_with_path(source: &str, path: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        RequireQueryAttributes.check(source, Path::new(path), &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_no_attributes_findone() {
        let source = r##"
import { User } from "#models";
User.findOne({ where: { id: 1 } });
"##;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 10);
        assert!(violations[0].fix_suggestion.contains("'id'"));
    }

    #[test]
    fn test_no_attributes_findall() {
        let source = r##"
import { User } from "#models";
User.findAll({ where: { status: "active" } });
"##;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'id'"));
        assert!(violations[0].fix_suggestion.contains("'status'"));
    }

    #[test]
    fn test_no_attributes_findbypk() {
        let source = r##"
import { User } from "#models";
User.findByPk(1, { where: { status: "active" } });
"##;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_missing_id() {
        let source = r##"
import { User } from "#models";
User.findOne({ attributes: ["name", "email"] });
"##;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'id'"));
    }

    #[test]
    fn test_empty_attributes_missing_id() {
        let source = r##"
import { User } from "#models";
User.findOne({ attributes: [] });
"##;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'id'"));
    }

    #[test]
    fn test_where_key_not_in_attributes() {
        let source = r##"
import { User } from "#models";
User.findOne({ attributes: ["id", "name"], where: { email: "test@test.com" } });
"##;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'email'"));
    }

    #[test]
    fn test_order_column_not_in_attributes() {
        let source = r##"
import { User } from "#models";
User.findAll({ attributes: ["id", "name"], order: [["createdAt", "DESC"]] });
"##;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'createdAt'"));
    }

    #[test]
    fn test_multiple_missing() {
        let source = r##"
import { User } from "#models";
User.findAll({
    attributes: ["id"],
    where: { status: "active", role: "admin" },
    order: [["createdAt", "DESC"]],
});
"##;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'createdAt'"));
        assert!(violations[0].fix_suggestion.contains("'role'"));
        assert!(violations[0].fix_suggestion.contains("'status'"));
    }

    #[test]
    fn test_include_missing_attributes() {
        let source = r##"
import { User, Post } from "#models";
User.findOne({
    attributes: ["id", "name"],
    include: [{ model: Post }],
});
"##;
        let violations = check(source);
        assert!(violations.len() >= 1);
    }

    #[test]
    fn test_include_empty_attributes_missing_id() {
        let source = r##"
import { User, Post } from "#models";
User.findOne({
    attributes: ["id", "name"],
    include: [{ model: Post, attributes: [] }],
});
"##;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'id'"));
    }

    #[test]
    fn test_real_world_example() {
        let source = r##"
import { ReviewLocation, ReviewSourceReviewLocation } from "#models";

export default () => {
    return ReviewLocation.findAll({
        attributes: ["errorMessage", "id", "source"],
        include: [{
            attributes: [],
            model: ReviewSourceReviewLocation,
            required: true,
        }],
        order: [["syncedAt", "ASC NULLS FIRST"]],
        raw: true,
        where: { syncStatus: "active" },
    });
};
"##;
        let violations = check(source);
        // syncedAt missing, syncStatus missing, include attributes missing id
        assert!(violations.len() >= 2);
    }

    // --- No violations ---

    #[test]
    fn test_all_correct() {
        let source = r##"
import { User } from "#models";
User.findOne({
    attributes: ["id", "name", "email", "createdAt"],
    where: { email: "test@test.com" },
    order: [["createdAt", "DESC"]],
});
"##;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_no_model_imports() {
        let source = r#"
import { something } from "other-package";
something.findOne({ where: { id: 1 } });
"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_non_query_method() {
        let source = r##"
import { User } from "#models";
User.create({ name: "test" });
"##;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_computed_where_key_skipped() {
        let source = r##"
import { User } from "#models";
User.findAll({
    attributes: ["id", "name"],
    where: { [Op.and]: [{ name: "test" }] },
});
"##;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_excluded_spec_file() {
        let source = r##"
import { User } from "#models";
User.findOne({ where: { id: 1 } });
"##;
        let violations = check_with_path(source, "src/index.spec.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_excluded_migration_file() {
        let source = r##"
import { User } from "#models";
User.findOne({ where: { id: 1 } });
"##;
        let violations = check_with_path(source, "src/migrations/001.js");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_include_with_all_attributes() {
        let source = r##"
import { User, Post } from "#models";
User.findOne({
    attributes: ["id", "name"],
    include: [{ model: Post, attributes: ["id", "title"] }],
});
"##;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    // --- Cross-file attributes ---

    #[test]
    fn test_external_attributes_file_resolves_member_expression() {
        let dir = tempfile::tempdir().unwrap();

        // Create attributes/index.js with id, name, email, status
        let attrs_dir = dir.path().join("attributes");
        fs::create_dir(&attrs_dir).unwrap();
        fs::write(attrs_dir.join("index.js"), r#"
export default {
    users: ["id", "name", "email", "status"],
};
"#).unwrap();

        // Create the query file using attributes.users
        let query_path = dir.path().join("query.js");
        let source = r##"
import { User } from "#models";
import attributes from "./attributes";

User.findAll({
    attributes: attributes.users,
    where: { status: "active" },
});
"##;
        fs::write(&query_path, source).unwrap();

        let tree = parse_js(source).unwrap();
        let mut cache = AstCache::new();
        let config = crate::config::Config::default();
        let violations = RequireQueryAttributes.check(source, &query_path, &tree, &mut cache, &config);
        // All required attributes are in the external file — no violations
        assert!(violations.is_empty());
    }

    #[test]
    fn test_external_attributes_file_missing_where_key() {
        let dir = tempfile::tempdir().unwrap();

        let attrs_dir = dir.path().join("attributes");
        fs::create_dir(&attrs_dir).unwrap();
        fs::write(attrs_dir.join("index.js"), r#"
export default {
    users: ["id", "name", "email"],
};
"#).unwrap();

        let query_path = dir.path().join("query.js");
        let source = r##"
import { User } from "#models";
import attributes from "./attributes";

User.findAll({
    attributes: attributes.users,
    where: { status: "active" },
});
"##;
        fs::write(&query_path, source).unwrap();

        let tree = parse_js(source).unwrap();
        let mut cache = AstCache::new();
        let config = crate::config::Config::default();
        let violations = RequireQueryAttributes.check(source, &query_path, &tree, &mut cache, &config);
        // "status" is in where but not in external attributes
        assert_eq!(violations.len(), 1);
        assert!(violations[0].fix_suggestion.contains("'status'"));
    }

    #[test]
    fn test_external_attributes_with_includes() {
        let dir = tempfile::tempdir().unwrap();

        let attrs_dir = dir.path().join("attributes");
        fs::create_dir(&attrs_dir).unwrap();
        fs::write(attrs_dir.join("index.js"), r#"
export default {
    competition: ["companyId", "endsAt", "id", "startsAt", "type"],
    scorecardMetric: ["archivedAt", "definition", "id", "type"],
};
"#).unwrap();

        let query_path = dir.path().join("query.js");
        let source = r##"
import { Competition, ScorecardMetric } from "#models";
import attributes from "./attributes";

Competition.findAll({
    attributes: attributes.competition,
    include: [
        { attributes: attributes.scorecardMetric, model: ScorecardMetric, required: true, where: { archivedAt: null } },
    ],
    where: { companyId, type: "instance" },
});
"##;
        fs::write(&query_path, source).unwrap();

        let tree = parse_js(source).unwrap();
        let mut cache = AstCache::new();
        let config = crate::config::Config::default();
        let violations = RequireQueryAttributes.check(source, &query_path, &tree, &mut cache, &config);
        // All attributes present in external file, including for the include
        assert!(violations.is_empty());
    }

    // --- Multiple queries ---

    #[test]
    fn test_multiple_queries_in_file() {
        let source = r##"
import { User, Post } from "#models";
User.findOne({ attributes: ["id", "name"] });
Post.findAll({ where: { status: "active" } });
"##;
        let violations = check(source);
        // Post.findAll has no attributes
        assert!(violations.len() >= 1);
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let source = r##"
import { User } from "#models";
User.findOne({ where: { id: 1 } });
"##;
        let violations = check(source);
        assert_eq!(violations[0].rule_name, "require-query-attributes");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let source = r##"
import { User } from "#models";
User.findOne({ where: { id: 1 } });
"##;
        let violations = check(source);
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
