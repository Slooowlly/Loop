# Driver Detail Modal Stability Design

## Context

`DriverDetailModal.jsx` currently mixes fetch orchestration, transition state, adjacent-driver navigation, and almost all dossier rendering in one file. The feature works in the happy path, but there is a real state bug: when `careerId` or `driverId` is unavailable, the fetch routine exits early and can leave the drawer stuck in loading mode.

## Chosen Direction

Apply a focused bugfix plus a moderate refactor:

- Keep `DriverDetailModal.jsx` as the public container used by `StandingsTab`.
- Add an explicit guard so missing `careerId` or `driverId` exits the loading state safely.
- Extract the heavier dossier tab sections and shared formatting helpers into a dedicated companion module.
- Keep drawer animation, portal mounting, and adjacent-driver navigation behavior unchanged.

## Interaction and State Design

- Opening the drawer still fetches data on demand through the existing Tauri command.
- If the modal cannot fetch because identifiers are missing, it must stop loading immediately and render a safe empty/error state instead of spinning forever.
- Close timing, ESC handling, backdrop handling, and edge navigator timing remain in the container because they are tightly coupled to the drawer shell.

## Refactor Boundary

In scope:

- Fix the loading-state bug in `DriverDetailModal.jsx`.
- Extract dossier sections such as current moment, form, career, and market into a smaller shared module.
- Remove obvious dead markup inside the modal file if it is no longer needed.
- Add focused tests covering the guard and the extraction contract.

Out of scope:

- Redesigning the dossier UI.
- Changing backend payloads.
- Large-scale tab or state-management rewrites.
- Season-finalization or market-module work.
