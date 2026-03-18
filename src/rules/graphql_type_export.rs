use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: in InputType/OutputType files, non-default-export GraphQL type
/// definitions (variables assigned to `new NonNullType(...)`, `new ObjectType(...)`, etc.)
/// should be inlined or moved to their own file. +50 per violation.
pub struct GraphqlTypeExport;

const SCORE_PER_VIOLATION: u32 = 100;

const GRAPHQL_TYPE_CONSTRUCTORS: &[&str] = &[
    "NonNullType",
    "ObjectType",
    "StringType",
    "BooleanType",
    "IntType",
    "FloatType",
    "ListType",
    "InputObjectType",
    "EnumType",
    "UnionType",
    "InterfaceType",
    "ScalarType",
];

impl Rule for GraphqlTypeExport {
    fn name(&self) -> &str {
        "graphql-type-export"
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/graphql-type-export.md"
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        let path_str = path.to_string_lossy();
        if !path_str.contains("InputType") && !path_str.contains("OutputType") {
            return vec![];
        }

        let score = config.rule_score("graphql-type-export", SCORE_PER_VIOLATION);
        let mut violations = Vec::new();
        find_non_default_graphql_types(tree.root_node(), source, &mut violations, self, score);
        violations
    }
}

/// Find top-level variable declarations that assign a `new <GraphqlType>(...)`.
/// These are non-default-export type definitions that should be inlined.
fn find_non_default_graphql_types(
    root: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &GraphqlTypeExport,
    score: u32,
) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            // const Foo = new ObjectType(...)
            "lexical_declaration" | "variable_declaration" => {
                let mut inner = child.walk();
                for decl in child.children(&mut inner) {
                    if decl.kind() == "variable_declarator" {
                        if let (Some(name_node), Some(value_node)) = (
                            decl.child_by_field_name("name"),
                            decl.child_by_field_name("value"),
                        ) {
                            if is_graphql_type_constructor(value_node, source) {
                                let var_name = &source[name_node.byte_range()];
                                let line = source.lines().nth(child.start_position().row).unwrap_or("");
                                violations.push(RuleViolation {
                                    rule_name: rule.name().to_string(),
                                    doc_url: rule.doc_url().to_string(),
                                    score,
                                    code_sample: line.trim().to_string(),
                                    fix_suggestion: format!("inline {var_name} into the default export or move it to its own type file"),
                                });
                            }
                        }
                    }
                }
            }
            // Skip export default — that's fine
            "export_statement" => {}
            _ => {}
        }
    }
}

/// Check if a node is a `new <GraphqlType>(...)` expression.
fn is_graphql_type_constructor(node: tree_sitter::Node, source: &str) -> bool {
    if node.kind() == "new_expression" {
        if let Some(constructor) = node.child_by_field_name("constructor") {
            let name = &source[constructor.byte_range()];
            return GRAPHQL_TYPE_CONSTRUCTORS.contains(&name);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse_js;

    fn check(source: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        GraphqlTypeExport.check(source, Path::new("src/graphql/OutputType/index.js"), &tree, &mut cache, &config)
    }

    fn check_with_path(source: &str, path: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        GraphqlTypeExport.check(source, Path::new(path), &tree, &mut cache, &config)
    }

    // --- Violations ---

    #[test]
    fn test_non_default_object_type() {
        let source = r#"const MyType = new ObjectType({ name: "MyType" });"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].score, 100);
    }

    #[test]
    fn test_non_default_non_null_type() {
        let source = r#"const MyType = new NonNullType(new ObjectType({ name: "MyType" }));"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_non_default_list_type() {
        let source = r#"const Items = new ListType(new ObjectType({ name: "Item" }));"#;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_real_world_example() {
        let source = r##"
import { BooleanType, ListType, NonNullType, ObjectType, StringType } from "#library/graphql/types";

const UpdateMyPushNotificationPreferenceConfigOutput_Item = new NonNullType(
    new ObjectType({
        fields: () => ({
            column: { type: StringType },
            enabled: { type: BooleanType },
        }),
        name: "UpdateMyPushNotificationPreferenceConfigOutput_Item",
    })
);

export default new NonNullType(
    new ObjectType({
        fields: () => ({
            preferences: { type: new NonNullType(new ListType(UpdateMyPushNotificationPreferenceConfigOutput_Item)) },
        }),
        name: "UpdateMyPushNotificationPreferenceConfigOutput",
    })
);
"##;
        let violations = check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].code_sample.contains("UpdateMyPushNotificationPreferenceConfigOutput_Item"));
    }

    #[test]
    fn test_multiple_non_default_types() {
        let source = r#"
const TypeA = new ObjectType({ name: "A" });
const TypeB = new NonNullType(new ObjectType({ name: "B" }));
export default new ObjectType({ name: "Main" });
"#;
        let violations = check(source);
        assert_eq!(violations.len(), 2);
        assert_eq!(violations.iter().map(|v| v.score).sum::<u32>(), 200);
    }

    // --- Path filtering ---

    #[test]
    fn test_output_type_path() {
        let violations = check_with_path(
            r#"const T = new ObjectType({ name: "T" });"#,
            "src/graphql/mutations/UpdateUser/OutputType/index.js",
        );
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_input_type_path() {
        let violations = check_with_path(
            r#"const T = new ObjectType({ name: "T" });"#,
            "src/graphql/mutations/UpdateUser/InputType/index.js",
        );
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_non_type_path_skipped() {
        let violations = check_with_path(
            r#"const T = new ObjectType({ name: "T" });"#,
            "src/graphql/mutations/UpdateUser/resolver.js",
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn test_random_path_skipped() {
        let violations = check_with_path(
            r#"const T = new ObjectType({ name: "T" });"#,
            "src/services/user.js",
        );
        assert!(violations.is_empty());
    }

    // --- No violations ---

    #[test]
    fn test_default_export_only() {
        let source = r#"export default new ObjectType({ name: "MyType" });"#;
        let violations = check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_non_graphql_variable() {
        let violations = check("const x = 42;");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_non_graphql_new_expression() {
        let violations = check("const x = new Map();");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let violations = check("");
        assert!(violations.is_empty());
    }

    #[test]
    fn test_import_only() {
        let violations = check(r##"import { ObjectType } from "#library/graphql/types";"##);
        assert!(violations.is_empty());
    }

    // --- Fix suggestion ---

    #[test]
    fn test_fix_suggestion_contains_var_name() {
        let source = r#"const MyHelper = new ObjectType({ name: "MyHelper" });"#;
        let violations = check(source);
        assert!(violations[0].fix_suggestion.contains("MyHelper"));
        assert!(violations[0].fix_suggestion.contains("inline"));
    }

    // --- Metadata ---

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check(r#"const T = new ObjectType({ name: "T" });"#);
        assert_eq!(violations[0].rule_name, "graphql-type-export");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check(r#"const T = new ObjectType({ name: "T" });"#);
        assert!(violations[0].doc_url.starts_with("https://"));
    }
}
