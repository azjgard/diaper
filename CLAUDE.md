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

## Tech Stack

- Rust
- tree-sitter with tree-sitter-javascript for parsing
