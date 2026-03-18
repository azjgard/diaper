# diaper

A fast JavaScript code smell scorer built with Rust and tree-sitter. Think of it like ESLint, but focused on structural code smells and designed to help AI agents write better code without constant human babysitting.

Instead of warnings and errors, diaper scores files with **stink points**. Each rule has a configurable score, a fix suggestion, and a documentation reference for agents to learn from.

## Install

Requires Rust 1.85+.

```sh
cargo build --release
# optionally symlink into your PATH
ln -sf $(pwd)/target/release/diaper ~/bin/diaper
```

## Usage

```sh
# Check unstaged git changes
diaper check

# Check a specific file
diaper check path/to/file.js

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
  +10   ternary-operator  const x = a ? b : c;
    replace ternary with if/else for readability
    https://github.com/jordin/diaper/blob/main/docs/rules/ternary-operator.md
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
| `async-await` | 100 per use | Flags `async`/`await` keywords (excludes `index.spec.js` and `/migrations`) |
| `ctx-destructure` | 10 per access | Direct `ctx.foo` access in pipe flow functions instead of destructuring |
| `file-too-long` | 10 per 50 lines over 200 | Files over 200 lines accumulate stink |
| `non-default-export` | 50 per function | Any function in a file that isn't the default export (including local functions) |
| `pipe-property-init` | 100 per property | Properties set on `{ ...ctx }` in pipe flow functions not initialized in the parent pipe call |
| `ternary-operator` | 10 single / 60 nested | Ternary expressions, with higher penalty for nesting |
| `upward-relative-import` | 100 per import | Imports using `../` paths (unless path contains "shared") |

Every rule includes a fix suggestion and links to documentation so agents can understand *why* a smell exists and how to fix it.

## Configuration

Run `diaper init` to generate a `diaper.yml` with all defaults:

```yaml
rules:
  async-await: 100
  ctx-destructure: 10
  file-too-long: 10
  non-default-export: 50
  ternary-single: 10
  ternary-nested: 60
  upward-relative-import: 100
  pipe-property-init: 100

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
