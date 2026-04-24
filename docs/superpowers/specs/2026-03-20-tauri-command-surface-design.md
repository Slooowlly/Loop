# Tauri Command Surface Cleanup Design

## Context

The production Tauri app currently registers three backend commands that are not used by the frontend application flow:

- `verify_database`
- `test_create_driver`
- `test_list_drivers`

They are useful as local diagnostics, but exposing them through `invoke_handler` unnecessarily widens the production command surface.

## Chosen Direction

Apply a conservative cleanup:

- Remove the three diagnostic functions from the production `invoke_handler`.
- Keep the helper functions in `career.rs` for internal/debug use.
- Mark them as internal diagnostics instead of production-facing commands.
- Add a regression test that fails if they are re-exposed accidentally.

## Scope

In scope:

- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/career.rs`
- a focused source-contract test in `scripts/tests`

Out of scope:

- deleting the helper implementations
- changing the normal frontend/backend flow
- broader `career.rs` refactoring
