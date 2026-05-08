# World Teams History Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the dark fixed-grid world team history atlas with family filters, movable years, team logos/colors, and standings integration.

**Architecture:** Add a focused Rust command that reads `team_season_archive` and current `teams`, then create a React tab that renders the fixed-grid slopegraph. Wire it from standings team double click, while preserving the existing single-click team dossier drawer behavior.

**Tech Stack:** Rust/Tauri, SQLite/rusqlite, React, Vitest, existing `TeamLogoMark` and `TeamHistoryDrawer`.

---

## File Structure

- Create: `src-tauri/src/commands/global_team_history.rs` for the backend payload.
- Modify: `src-tauri/src/commands/career_types.rs` for serializable DTOs.
- Modify: `src-tauri/src/commands/career_commands.rs` and `src-tauri/src/lib.rs` to expose the command.
- Create: `src/pages/tabs/GlobalTeamsTab.jsx` for the world team atlas.
- Create: `src/pages/tabs/GlobalTeamsTab.test.jsx` for UI behavior.
- Modify: `src/pages/Dashboard.jsx` and `src/pages/Dashboard.test.jsx` for hidden tab routing.
- Modify: `src/pages/tabs/StandingsTab.jsx` and tests so single click opens team dossier and double click opens the world atlas.

## Chunk 1: Backend Contract

- [ ] Write failing Rust test for `get_global_team_history_in_base_dir` returning Mazda family bands with regular and special slots.
- [ ] Implement DTOs and command using `team_season_archive`.
- [ ] Register the Tauri command.

## Chunk 2: Frontend Atlas

- [ ] Write failing React test for loading, family filters, year movement, logo/name rendering, inactive bands, and callback behavior.
- [ ] Implement `GlobalTeamsTab.jsx` with fixed grid SVG rendering.
- [ ] Reuse `TeamLogoMark` and avoid extra swatches.

## Chunk 3: Navigation

- [ ] Write/adjust tests for dashboard hidden tab and standings team single/double click.
- [ ] Wire `StandingsTab` double click to global teams and single click to `TeamHistoryDrawer`.
- [ ] Verify focused tests and formatting/build checks.
