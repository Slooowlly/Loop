# Career Types Extraction Design

## Context

`src-tauri/src/commands/career.rs` concentrates command handlers, helper functions, tests, and a large set of payload/response structs in the same file. The first safe slice of the monolith reduction is to move only the type declarations out of the file, keeping runtime behavior unchanged.

## Chosen Direction

Extract the command-facing DTOs and response blocks into a dedicated sibling module:

- add `src-tauri/src/commands/career_types.rs`
- re-export nothing globally; `career.rs` imports the types it needs directly
- keep helper functions and command implementations inside `career.rs` for now

## Scope

In scope:

- move the response/request structs currently declared at the top of `career.rs`
- add module wiring in `src-tauri/src/commands/mod.rs`
- adjust imports in `career.rs`
- add a structural regression test proving the extraction happened

Out of scope:

- moving helper functions
- splitting commands into separate files
- changing public command names or payload shapes
