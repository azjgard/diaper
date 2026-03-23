# diaper

A fast CLI tool for parsing JavaScript files, built with Rust and tree-sitter.

## Core Tenets

1. **SIMPLE SIMPLE SIMPLE.** This is our mantra. Prioritize unbraided, simple code above all else.
2. **Avoid unnecessary abstraction like the plague.** If it doesn't need a trait, don't make one. If it doesn't need a wrapper, don't wrap it. Write the obvious thing.
3. **Repetition over coupling.** It's OK to repeat yourself. Prefer context files that semantically link similar implementations over coupling different project pieces together. Independent pieces stay independent.
4. **No cleverness.** If a junior dev can't read it and understand it in 30 seconds, it's too clever.

## Development Rules

- **Always write tests for EVERYTHING.** No exceptions.
- **Always run tests after making a batch of changes.** Never skip this.
- **Always run `cargo build` after changes to confirm it compiles.** Never skip this.
- **Be comprehensive in testing.** Too many tests >> too few tests. Test happy paths, edge cases, error cases, and boundary conditions.
- **Commit and summarize after finishing every batch of changes.**
- **Push to remote after committing.** Always `git push` after a commit.

## Creating Rules

See [RULE_CREATION_INSTRUCTIONS.md](RULE_CREATION_INSTRUCTIONS.md) for step-by-step instructions on creating new rules, including file structure, conventions, registration, testing requirements, and a checklist.

## Releases

Use `scripts/release.sh <version>` to cut a release. It bumps `Cargo.toml`, commits, tags, pushes, builds macOS + Linux binaries, and creates a GitHub release.

```sh
bash scripts/release.sh 0.4.0-beta
```

After the release is created, update the auto-generated release notes to match the format of prior releases:

1. **Find the previous tag** by running `git tag --sort=-creatordate | head -5`. The previous tag is the one immediately before the new release — do NOT assume it from conversation context, as other agents or sessions may have created tags you're unaware of.
2. **Scope the notes** to only commits in `git log <previous-tag>..<new-tag>`. Do not include changes from earlier releases.
3. **Organize into sections** (omit empty sections):
   - **New Features** — New CLI flags, commands, or user-facing capabilities
   - **New Rules** — New rules with name, default score, and one-line description
   - **Rule Changes** — Behavioral changes to existing rules (exclusions, renames, scope changes)
   - **Bug Fixes & Docs** — Corrections to rule behavior or examples
   - **Infrastructure** — Build, CI, e2e, or internal tooling changes
4. **Add a screenshot** of the most relevant new feature or change using `upload-terminal-image` (available at `~/bin/upload-terminal-image`). This tool runs a command via `termshot`, uploads the screenshot, and prints a public URL.
   - Create a small wrapper script in `/tmp/` that `cd`s into the project dir, sets up any needed state, and runs the command. Don't use `bash -c` — termshot doesn't handle it well.
   - Run `termshot` without `--show-cmd` (omit the flag) so the wrapper script path doesn't appear in the image.
   - Keep the output short (2-3 violations max) so it fits in termshot's pseudo terminal buffer. Use a small sample file if needed.
   - Example workflow:
     ```sh
     # Create wrapper script
     cat > /tmp/ss.sh << 'EOF'
     #!/bin/bash
     cd /Users/jordin/projects/diaper
     ./target/release/diaper rules --verbose
     EOF
     chmod +x /tmp/ss.sh

     # Capture and upload (without --show-cmd)
     tmpdir=$(mktemp -d)
     termshot --filename "$tmpdir/out.png" -- /tmp/ss.sh >&2
     ~/bin/upload-image "$tmpdir/out.png"
     rm -rf "$tmpdir"
     ```
   - Embed in release notes as `![description](url)`
5. **End with a Full Changelog link** comparing the previous tag to the new one: `https://github.com/azjgard/diaper/compare/<previous-tag>...<new-tag>`

Use `gh release edit <tag> --notes "..."` to update.

## Tech Stack

- Rust
- tree-sitter with tree-sitter-javascript for parsing
