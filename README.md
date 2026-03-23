# `[--diaper--]`

> **Note:** diaper is currently in beta.

![diaper check output](https://pub-2e7c0956321d48409c49627cc2bb6d79.r2.dev/images/out/5576e4256c6c48ceba9425ccac34ce3d.png)

A fast JavaScript code smell scorer built with Rust and tree-sitter. Think of it like ESLint, but focused on structural code smells and designed to help AI agents write better code without constant human babysitting.

Instead of warnings and errors, diaper scores files with **stink points**. Each rule has a configurable score, a fix suggestion, and a documentation reference for agents to learn from.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/azjgard/diaper/main/install.sh | bash
```

### Claude Code integration

Install the Claude Code hooks:

```sh
diaper install-hooks
```

This installs two [Claude Code hooks](https://docs.anthropic.com/en/docs/claude-code/hooks):

1. **Stop hook** — Runs `diaper check` against unstaged JavaScript files whenever Claude finishes a task. If there are unresolved violations, Claude is blocked from stopping and the violations are injected into context for it to fix. Claude can accept non-blowout violations by running `touch /tmp/diaper-check-accept`.
2. **Pre-edit hook** — Before Claude edits a `.js` file, checks if the file has a sibling `index.spec.js`. If not, injects a reminder into Claude's context to consider adding tests. This does not block the edit.

> **Note:** During the closed beta, both hooks only run in the `api-gateway` project.

For the best experience, run Claude in bypass permissions mode so it can create the sentinel file without prompting:

```sh
claude --dangerously-skip-permissions
```

### From source

Requires Rust 1.85+.

```sh
cargo build --release
ln -sf $(pwd)/target/release/diaper ~/bin/diaper
```

## Usage

```sh
# Check unstaged git changes
diaper check

# Check a specific file
diaper check path/to/file.js

# Only run specific rules
diaper check --rule file-too-long --rule async-await
diaper check -r file-too-long,async-await,nested-ternary

# Check with JSON output (for tooling/agents)
diaper check --json

# Watch for changes and re-run checks
diaper watch

# Generate a diaper.yml config file with defaults
diaper init
```

## Output

```
src/api/handler.js  BLOWOUT 💩 (340)
  +100  async-await  async function handleRequest() {
    remove async/await and use synchronous patterns or callbacks
    https://github.com/jordin/diaper/blob/main/docs/rules/async-await.md
  +100  upward-relative-import  import ... from "../../core/db"
    use an alias or move the import to a shared module instead of "../../core/db"
    ./docs/rules/upward-relative-import.md
  +60   nested-ternary  const x = a ? b ? c : d : e;
    extract nested ternary (2 levels) into a sub-function with early returns for each branch
    https://github.com/jordin/diaper/blob/main/docs/rules/nested-ternary.md
  +30   file-too-long  430 lines
    split file into smaller modules (currently 430 lines, threshold 200)
    https://github.com/jordin/diaper/blob/main/docs/rules/file-too-long.md
```

Each violation shows: score, rule name, code sample, fix suggestion (green), and docs path (gray).

## Exit Codes

- **0** — no files reached BLOWOUT tier
- **1** — at least one file hit BLOWOUT (score >= 100, configurable)

## Stink Tiers

| Score | Tier | Emoji |
|-------|------|-------|
| 0-30 | Damp | 💧 |
| 31-70 | Wet | 💦 |
| 71-99 | Soiled | 🤢 |
| 100+ | BLOWOUT | 💩 |

## Rules

| Rule | Default Score | Description |
|------|-------------|-------------|
| `async-await` | 100 per use | Flags `async`/`await` keywords |
| `async-promise-return` | 15 per return | Non-Promise returns in `-async` folder functions |
| `ctx-destructure` | 10 per access | Direct `ctx.foo` access instead of destructuring |
| `distinct-array` | 20 per pattern | Manual array dedup instead of `distinct()` |
| `file-too-long` | 10 per 50 lines over 200 | Files over 200 lines accumulate stink |
| `graphql-type-export` | 100 per type | GraphQL types not using default export in type files |
| `missing-test` | 50 per file | Files with functions or logic missing a sibling `index.spec.js` |
| `mock-models` | 100 per mock | `jest.mock("#models", ...)` in `index.spec.js` files |
| `nested-ternary` | 60 per nested ternary | Nested ternary expressions (2+ levels deep) |
| `non-default-export` | 50 per function | Any function that isn't the default export |
| `non-idempotent-migration` | 50 per call | `addColumn`/`removeColumn` in migrations |
| `pipe-property-init` | 100 per property | Pipe flow properties not initialized in parent pipe call |
| `reduce-param-name` | 70 per call | `.reduce()` callback first param not named `prevVal` |
| `require-query-attributes` | 10 per query | Sequelize queries missing explicit `attributes` |
| `short-iter-param` | 15 per call | Iterator callback item param 3 chars or less |
| `sql-table-alias` | 100 per alias | Table aliases in raw SQL queries |
| `unsorted-string-array` | 5 per array | String arrays not in alphabetical order |
| `upward-relative-import` | 100 per import | Imports using `../` paths (unless path contains "shared") |

Most rules exclude `index.spec.js` and `/migrations/` paths. Exceptions: `mock-models` only runs on `index.spec.js`, `non-idempotent-migration` only runs on migration files. Every rule includes a fix suggestion and documentation link.

## Configuration

Run `diaper init` to generate a `diaper.yml` with all defaults:

```yaml
rules:
  async-await: 100
  async-promise-return: 15
  ctx-destructure: 10
  file-too-long: 10
  graphql-type-export: 100
  mock-models: 100
  non-default-export: 50
  non-idempotent-migration: 50
  pipe-property-init: 100
  reduce-param-name: 70
  require-query-attributes: 10
  short-iter-param: 15
  ternary-nested: 60
  unsorted-string-array: 5
  upward-relative-import: 100

levels:
  damp: 0
  wet: 31
  soiled: 71
  blowout: 100
```

Rules can also specify a local docs path for agents to reference:

```yaml
rules:
  async-await: 100
  non-default-export:
    score: 50
    docs: ./docs/rules/non-default-export.md
  pipe-property-init:
    score: 100
    docs: ./docs/conventions/pipes/context-initialization/index.md
```

Override any value. Missing properties fall back to defaults. Bare scores and full objects can be mixed freely.

## JSON Output

`diaper check --json` outputs:

```json
[
  {
    "path": "src/handler.js",
    "stinkScore": 200,
    "violations": [
      {
        "rule": "async-await",
        "stinkScore": 100,
        "codeSample": "async function handle() {",
        "fixSuggestion": "remove async/await and use synchronous patterns or callbacks",
        "reference": "https://github.com/jordin/diaper/blob/main/docs/rules/async-await.md"
      }
    ]
  }
]
```

## Adding Rules

See [RULE_CREATION_INSTRUCTIONS.md](RULE_CREATION_INSTRUCTIONS.md) for a step-by-step guide to creating new rules, including conventions, test requirements, and a complete checklist.

## Development

```sh
make build    # cargo build
make test     # cargo test
make release  # cargo build --release
make check    # cargo run -- check
make watch    # cargo run -- watch
```

## License

MIT
