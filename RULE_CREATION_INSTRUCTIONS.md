# How to Create a New Diaper Rule

Step-by-step instructions for adding a new rule to diaper.

## Before you start

Read these two example rules to understand the patterns and conventions:

1. **`src/rules/async_await.rs`** — Good example of an AST-walking rule with path-based exclusions, multiple node type matching, and comprehensive test coverage including edge cases (strings, comments, variable names).

2. **`src/rules/upward_relative_import.rs`** — Good example of a rule that extracts data from AST nodes (import sources), applies business logic (path checks), and handles both `import` and `require()` syntax.

## Step 1: Create the rule file

Create `src/rules/your_rule_name.rs`. Use snake_case for the filename.

### File structure

```rust
use std::path::Path;

use super::{Rule, RuleViolation};

/// Rule: one-line description of what this rule detects.
/// Include the score and what triggers it.
pub struct YourRuleName;

const SCORE_PER_VIOLATION: u32 = 50; // default score for this rule

impl Rule for YourRuleName {
    fn name(&self) -> &str {
        "your-rule-name" // kebab-case, used as config key in diaper.yml
    }

    fn doc_url(&self) -> &str {
        "https://github.com/jordin/diaper/blob/main/docs/rules/your-rule-name.md"
    }

    fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, _cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
        // Look up configurable score, falling back to the constant default
        let score = config.rule_score("your-rule-name", SCORE_PER_VIOLATION);

        let mut violations = Vec::new();

        // Walk the tree-sitter AST to find violations
        collect_violations(tree.root_node(), source, &mut violations, self, score);

        violations
    }
}

/// Recursive AST walker. Separate from the Rule impl for clarity.
fn collect_violations(
    node: tree_sitter::Node,
    source: &str,
    violations: &mut Vec<RuleViolation>,
    rule: &YourRuleName,
    score: u32,
) {
    // Check if this node matches what you're looking for
    if node.kind() == "some_node_type" {
        let line = source.lines().nth(node.start_position().row).unwrap_or("");
        violations.push(RuleViolation {
            rule_name: rule.name().to_string(),
            doc_url: rule.doc_url().to_string(),
            score,
            code_sample: line.trim().to_string(),
        });
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_violations(child, source, violations, rule, score);
    }
}
```

### Conventions

- **`SCORE_PER_VIOLATION`**: Always define as a `const u32` at the top of the file. This is the hardcoded default.
- **Config lookup**: Always call `config.rule_score("your-rule-name", SCORE_PER_VIOLATION)` at the start of `check()` and pass the result to helper functions. Never use the constant directly in violation structs.
- **`code_sample`**: Should be just the relevant code — no descriptive prefix. Use `line.trim().to_string()` for the line containing the violation, or a synthetic sample like `format!("import ... from \"{path}\"")` if the raw line isn't meaningful.
- **AST walking**: Extract the recursive walk into a standalone function (not a method on the struct). Pass `rule: &YourRuleName` and `score: u32` as parameters.
- **Unused parameters**: Prefix with underscore (`_path`, `_cache`, `_tree`) if not needed.

## Step 2: Register the rule

### In `src/rules/mod.rs`

1. Add the module declaration (keep alphabetical):
```rust
pub mod your_rule_name;
```

2. Add to `all_rules()` (keep alphabetical):
```rust
Box::new(your_rule_name::YourRuleName),
```

### In `src/config.rs`

1. Add a default constant:
```rust
pub const DEFAULT_YOUR_RULE_NAME: u32 = 50;
```

2. Add to the `generate_default_config()` function's rules section:
```rust
  your-rule-name: {DEFAULT_YOUR_RULE_NAME}
```

## Step 3: Write tests

Tests go in a `#[cfg(test)] mod tests` block at the bottom of the rule file.

### Test helper

Always start with this helper function:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse_js;

    fn check(source: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        YourRuleName.check(source, Path::new("src/foo.js"), &tree, &mut cache, &config)
    }
```

If your rule cares about file paths, add a second helper:

```rust
    fn check_with_path(source: &str, path: &str) -> Vec<RuleViolation> {
        let tree = parse_js(source).unwrap();
        let mut cache = super::super::AstCache::new();
        let config = crate::config::Config::default();
        YourRuleName.check(source, Path::new(path), &tree, &mut cache, &config)
    }
```

### Required test categories

Organize tests with comment headers. Every rule MUST have tests in each category:

```rust
    // --- Violations ---
    // Tests that SHOULD produce violations. Assert count and score.

    // --- No violations ---
    // Tests for code that should NOT trigger the rule.
    // Include near-misses and similar-looking-but-valid patterns.

    // --- Edge cases ---
    // Empty file, comments, strings, template literals,
    // multi-line patterns, mixed valid/invalid, etc.

    // --- Metadata ---
    // Always include these two:

    #[test]
    fn test_violation_has_correct_rule_name() {
        let violations = check(/* triggering source */);
        assert_eq!(violations[0].rule_name, "your-rule-name");
    }

    #[test]
    fn test_violation_has_doc_url() {
        let violations = check(/* triggering source */);
        assert!(violations[0].doc_url.starts_with("https://"));
    }
```

### Edge cases to always test

- Empty file: `check("")`
- Code in comments: `check("// <triggering code>")`
- Code in strings: `check(r#"const x = "<triggering pattern>";"#)`
- Multiple violations in one file
- Score arithmetic: `violations.iter().map(|v| v.score).sum::<u32>()`

## Step 4: Build and test

```
cargo build    # must compile with no warnings
cargo test     # all tests must pass
```

## Step 5: Cross-file rules (advanced)

If your rule needs to read other files, use `cache` instead of `_cache`:

```rust
fn check(&self, source: &str, path: &Path, tree: &tree_sitter::Tree, cache: &mut super::AstCache, config: &crate::config::Config) -> Vec<RuleViolation> {
    // Read another file's AST
    let other_path = path.parent().unwrap().join("index.js");
    if let Some((other_source, other_tree)) = cache.get_or_parse(&other_path) {
        // Walk other_tree to find what you need
    }
}
```

See `src/rules/pipe_property_init.rs` for a complete example of a cross-file rule.

For cross-file rule tests, create temp directories with multiple files:

```rust
let dir = tempfile::tempdir().unwrap();
let sub = dir.path().join("steps");
fs::create_dir(&sub).unwrap();
fs::write(sub.join("step.js"), "/* flow function source */").unwrap();
fs::write(dir.path().join("index.js"), "/* pipe call site */").unwrap();
```

## Tree-sitter tips

- Use `node.kind()` to check node type (e.g. `"function_declaration"`, `"import_statement"`, `"call_expression"`)
- Use `node.child_by_field_name("name")` to get named fields
- Use `&source[node.byte_range()]` to get the text of a node
- Use `node.start_position().row` to get the line number
- To see what node types exist, parse a sample and print `tree.root_node().to_sexp()`
- Common JS node types: `import_statement`, `export_statement`, `function_declaration`, `arrow_function`, `call_expression`, `ternary_expression`, `await_expression`, `return_statement`, `object`, `pair`, `spread_element`, `string`, `string_fragment`

## Checklist

- [ ] Rule file created in `src/rules/`
- [ ] `SCORE_PER_VIOLATION` constant defined
- [ ] Config lookup via `config.rule_score()` in `check()`
- [ ] Module registered in `src/rules/mod.rs`
- [ ] Rule added to `all_rules()` in `src/rules/mod.rs`
- [ ] Default constant added to `src/config.rs`
- [ ] Rule added to `generate_default_config()` in `src/config.rs`
- [ ] Tests: violations, no violations, edge cases, metadata
- [ ] `cargo build` clean (no warnings)
- [ ] `cargo test` all passing
