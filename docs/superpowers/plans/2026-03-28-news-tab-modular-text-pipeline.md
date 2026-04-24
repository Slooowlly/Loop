# News Tab Modular Text Pipeline Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Substituir o pipeline textual legado da `NewsTab` por um contrato modular com blocos por intenção e oito variações por tipo editorial.

**Architecture:** O backend continuará lendo `NewsItem`, mas passará a classificá-los em tipos editoriais e transformá-los em `NewsTabStory` modulares já prontos para a UI. O frontend deixará de derivar resumo e texto sintético e passará a renderizar `headline`, `deck` e `blocks[]` diretamente do snapshot.

**Tech Stack:** Rust + Tauri commands, React, Vitest

---

## File Map

- Modify: `src-tauri/src/commands/career_types.rs`
  - Evoluir o contrato de `NewsTabStory` e adicionar `NewsTabStoryBlock`.
- Modify: `src-tauri/src/commands/news_tab.rs`
  - Classificar `NewsItem`, gerar copy modular, escolher variações e montar o novo snapshot.
- Modify: `src/pages/tabs/NewsTab.jsx`
  - Renderizar o contrato modular no leitor principal e na coluna lateral.
- Modify: `src/pages/tabs/NewsTab.test.jsx`
  - Cobrir o novo shape e a renderização dos blocos.
- Optional modify: `src-tauri/src/commands/news_tab.rs` tests
  - Validar contrato e seleção de variações.

## Chunk 1: Contract And Backend Shape

### Task 1: Add a failing backend contract test for modular stories

**Files:**
- Modify: `src-tauri/src/commands/news_tab.rs`

- [ ] **Step 1: Write the failing test**

Add a test near the existing `news_tab_snapshot` tests asserting that each story now exposes:

- `headline`
- `deck`
- `blocks.len() == 3`
- non-empty block labels

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test news_tab_snapshot --manifest-path src-tauri/Cargo.toml`
Expected: FAIL because `NewsTabStory` does not yet expose the modular fields.

- [ ] **Step 3: Add the new contract types**

Update `src-tauri/src/commands/career_types.rs`:

- add `NewsTabStoryBlock { label, text }`
- replace or extend `NewsTabStory` with:
  - `headline`
  - `deck`
  - `blocks: Vec<NewsTabStoryBlock>`

Keep temporary compatibility fields only if needed for a narrow transition.

- [ ] **Step 4: Run the targeted test again**

Run: `cargo test news_tab_snapshot --manifest-path src-tauri/Cargo.toml`
Expected: still FAIL, but now for missing story-building logic rather than missing types.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/career_types.rs src-tauri/src/commands/news_tab.rs
git commit -m "feat: add modular news tab story contract"
```

## Chunk 2: Editorial Type Classifier

### Task 2: Add editorial-type classification for `NewsItem`

**Files:**
- Modify: `src-tauri/src/commands/news_tab.rs`
- Test: `src-tauri/src/commands/news_tab.rs`

- [ ] **Step 1: Write failing tests for classification**

Add tests asserting classification outcomes for at least:

- `NewsType::Corrida` -> `Corrida`
- `NewsType::Incidente` / `Lesao` -> `Incidente`
- `NewsType::Mercado` -> `Mercado`
- `Promocao`, `Rebaixamento`, `Aposentadoria` -> `Estrutural`

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test news_tab_snapshot --manifest-path src-tauri/Cargo.toml`
Expected: FAIL because classifier function does not exist.

- [ ] **Step 3: Implement minimal classifier**

Add focused helper(s) in `src-tauri/src/commands/news_tab.rs`:

- `enum EditorialStoryType`
- `fn classify_editorial_story_type(item: &NewsItem) -> EditorialStoryType`

Rules:

- `Corrida` can also be refined into `Piloto` or `Equipe` later by entity shape if needed.
- Keep first cut deterministic and simple.

- [ ] **Step 4: Run tests**

Run: `cargo test news_tab_snapshot --manifest-path src-tauri/Cargo.toml`
Expected: PASS for classifier tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/news_tab.rs
git commit -m "feat: classify news tab items by editorial type"
```

## Chunk 3: Copy Pools And Variation Selection

### Task 3: Add modular copy pools with 8 variations per type

**Files:**
- Modify: `src-tauri/src/commands/news_tab.rs`

- [ ] **Step 1: Write failing tests for variation boundaries**

Add tests asserting:

- each editorial type resolves one of 8 variation slots
- generated blocks remain exactly 3
- labels match the approved trio for the type

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test news_tab_snapshot --manifest-path src-tauri/Cargo.toml`
Expected: FAIL because variation selection and copy pools do not exist.

- [ ] **Step 3: Implement copy model**

In `src-tauri/src/commands/news_tab.rs`, add:

- `EditorialBlockSpec`
- `EditorialStoryPayload`
- one pool per type with 8 variations

Approved labels:

- `Corrida`: `Resumo`, `Impacto`, `Leitura`
- `Incidente`: `Ocorrido`, `Consequencia`, `Estado`
- `Piloto`: `Momento`, `Pressao`, `Sinal`
- `Equipe`: `Movimento`, `Resposta`, `Panorama`
- `Mercado`: `Movimento`, `Impacto`, `Proximo passo`
- `Estrutural`: `Mudanca`, `Efeito`, `Panorama`

- [ ] **Step 4: Implement deterministic variation selection**

Use a stable seed from item identity, for example:

- item id
- type
- round
- season

Goal: same story should not reshuffle every render.

- [ ] **Step 5: Run tests**

Run: `cargo test news_tab_snapshot --manifest-path src-tauri/Cargo.toml`
Expected: PASS for variation shape and block-label tests.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/news_tab.rs
git commit -m "feat: add modular copy pools for news tab stories"
```

## Chunk 4: Build Snapshot Stories From Modular Payload

### Task 4: Replace `build_stories` excerpt logic with modular story assembly

**Files:**
- Modify: `src-tauri/src/commands/news_tab.rs`
- Test: `src-tauri/src/commands/news_tab.rs`

- [ ] **Step 1: Write failing tests for `build_stories` output**

Cover that:

- `headline` comes from modular payload
- `deck` is no longer a raw excerpt
- `blocks` are populated and typed correctly

- [ ] **Step 2: Run tests**

Run: `cargo test news_tab_snapshot --manifest-path src-tauri/Cargo.toml`
Expected: FAIL because `build_stories` still maps `titulo` and `texto` directly.

- [ ] **Step 3: Implement modular assembly**

Refactor `build_stories` so that it:

1. classifies each `NewsItem`
2. derives facts from item/context
3. builds `EditorialStoryPayload`
4. maps payload into `NewsTabStory`

Keep:

- `meta_label`
- `time_label`
- `entity_label`
- `race_label`
- `accent_tone`

These still support the UI and filters.

- [ ] **Step 4: Remove old excerpt dependency from the backend path**

Delete or stop using:

- `build_story_excerpt`
- any `deck` derived by char truncation for `NewsTab`

- [ ] **Step 5: Run tests**

Run: `cargo test news_tab_snapshot --manifest-path src-tauri/Cargo.toml`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/news_tab.rs
git commit -m "refactor: build modular stories for news tab snapshot"
```

## Chunk 5: Frontend Rendering Migration

### Task 5: Render modular fields in `NewsTab.jsx`

**Files:**
- Modify: `src/pages/tabs/NewsTab.jsx`
- Test: `src/pages/tabs/NewsTab.test.jsx`

- [ ] **Step 1: Write failing frontend test**

Extend the existing reading test to assert:

- headline comes from the modular contract
- the three block labels are rendered from `story.blocks`
- the deck appears in the lateral list if adopted there
- frontend no longer synthesizes `Por que importa`

- [ ] **Step 2: Run test to verify failure**

Run: `npm test -- --run src/pages/tabs/NewsTab.test.jsx`
Expected: FAIL because the component still reads `summary` and `body_text`.

- [ ] **Step 3: Update the frontend**

In `src/pages/tabs/NewsTab.jsx`:

- main reader:
  - use `story.headline`
  - render `story.blocks.map(...)`
- lateral list:
  - use `story.deck` as the short supporting text
- remove frontend-generated `buildStoryWhyItMatters`
- remove any remaining dependence on excerpt-style fields if no longer needed

- [ ] **Step 4: Update tests and fixtures**

In `src/pages/tabs/NewsTab.test.jsx`:

- update mocked stories to the new shape
- keep click-to-switch behavior assertions

- [ ] **Step 5: Run tests**

Run: `npm test -- --run src/pages/tabs/NewsTab.test.jsx`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/pages/tabs/NewsTab.jsx src/pages/tabs/NewsTab.test.jsx
git commit -m "refactor: render modular news stories in news tab"
```

## Chunk 6: Regression Verification

### Task 6: Verify the full flow end to end

**Files:**
- Modify if needed: `src-tauri/src/commands/news_tab.rs`
- Modify if needed: `src/pages/tabs/NewsTab.jsx`
- Test: existing backend and frontend suites

- [ ] **Step 1: Run backend snapshot tests**

Run: `cargo test news_tab_snapshot --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 2: Run frontend news tab tests**

Run: `npm test -- --run src/pages/tabs/NewsTab.test.jsx`
Expected: PASS

- [ ] **Step 3: Run production build**

Run: `npm run build`
Expected: build completes successfully

- [ ] **Step 4: Manual review checklist**

Confirm:

- category switching still works
- primary/contextual filters still narrow the story set
- lateral click still swaps the open story locally
- new copy feels aligned with the block-based layout

- [ ] **Step 5: Final commit**

```bash
git add src-tauri/src/commands/career_types.rs src-tauri/src/commands/news_tab.rs src/pages/tabs/NewsTab.jsx src/pages/tabs/NewsTab.test.jsx
git commit -m "feat: replace legacy news tab text pipeline with modular stories"
```
