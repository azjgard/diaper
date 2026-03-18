# diaper

A fast JavaScript code smell scorer built with Rust and tree-sitter. Think of it like ESLint, but focused on structural code smells and designed to help AI agents write better code without constant human babysitting.

Instead of warnings and errors, diaper scores files with **stink points**. Each rule has a configurable score, a documentation backlink for agents to learn from, and a code sample showing what triggered it.

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
  +100  async-await  async function handleRequest() {  docs
  +100  async-await  const data = await fetch('/api');  docs
  +100  upward-relative-import  import ... from "../../core/db"  docs
  +10   ternary-operator  const x = a ? b : c;  docs
  +30   file-too-long  430 lines  docs
  Game over. The couch is ruined.
```

## Stink Tiers

| Score | Tier | Emoji | Message |
|-------|------|-------|---------|
| 0-30 | Damp | 💧 | Basically dry! |
| 31-70 | Wet | 💦 | A little dirty, but this is what diapers are made for. |
| 71-99 | Soiled | 🤢 | You should probably change this -- rash imminent. |
| 100+ | BLOWOUT | 💩 | Game over. The couch is ruined. |

## Rules

| Rule | Default Score | Description |
|------|-------------|-------------|
| `async-await` | 100 per use | Flags `async`/`await` keywords (excludes `index.spec.js` and `/migrations`) |
| `file-too-long` | 10 per 50 lines over 200 | Files over 200 lines accumulate stink |
| `non-default-export` | 50 per export | Named (non-default) exported functions |
| `ternary-operator` | 10 single / 60 nested | Ternary expressions, with higher penalty for nesting |
| `upward-relative-import` | 100 per import | Imports using `../` paths (unless path contains "shared") |
| `pipe-property-init` | 100 per property | Properties set on `{ ...ctx }` in pipe flow functions that aren't initialized in the parent pipe call |

Every rule links to documentation so agents can understand *why* a smell exists and how to fix it.

## Configuration

Run `diaper init` to generate a `diaper.yml` with all defaults:

```yaml
rules:
  async-await: 100
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

Override any value. Missing properties fall back to defaults.

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
make check    # cargo run -- check
make watch    # cargo run -- watch
```

## License

MIT
