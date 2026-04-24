# PT-3 Player Proposals Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implementar leitura, resposta e validacao final das propostas do jogador durante a pre-temporada.

**Architecture:** A logica de negocio fica em `commands/career.rs`, apoiada por queries pequenas em `db/queries/market_proposals.rs` e uma helper reutilizavel em `db/queries/teams.rs`. Noticias sao geradas com helpers do modulo `news`.

**Tech Stack:** Rust, Tauri, rusqlite, serde, testes unitarios existentes do projeto.

---

## Chunk 1: Queries e contratos de dados

### Task 1: CRUD de market proposals

**Files:**
- Create: `src-tauri/src/db/queries/market_proposals.rs`
- Modify: `src-tauri/src/db/queries/mod.rs`
- Test: `src-tauri/src/db/queries/market_proposals.rs`

- [ ] Escrever testes para buscar propostas pendentes, atualizar status e expirar restantes.
- [ ] Implementar as queries minimas.
- [ ] Exportar o modulo em `db/queries/mod.rs`.

### Task 2: Remocao reutilizavel de piloto da equipe

**Files:**
- Modify: `src-tauri/src/db/queries/teams.rs`
- Test: `src-tauri/src/db/queries/teams.rs`

- [ ] Escrever teste para limpar `piloto_1_id` ou `piloto_2_id`.
- [ ] Implementar `remove_pilot_from_team`.

## Chunk 2: Commands do jogador

### Task 3: Listagem enriquecida de propostas

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/commands/career_commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/commands/career.rs`

- [ ] Escrever testes para retornar apenas propostas pendentes e enriquecer com dados da equipe.
- [ ] Implementar `PlayerProposalView`.
- [ ] Implementar `get_player_proposals_in_base_dir` e comando Tauri.

### Task 4: Aceitar ou recusar proposta

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/commands/career_commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/news/generator.rs`
- Test: `src-tauri/src/commands/career.rs`
- Test: `src-tauri/src/news/generator.rs`

- [ ] Escrever testes de aceite, recusa, expiracao das demais propostas e noticias.
- [ ] Implementar `ProposalResponse`.
- [ ] Implementar fluxo completo de aceite.
- [ ] Implementar fluxo de recusa com propostas emergenciais ou force-place.
- [ ] Expor `respond_to_proposal`.

## Chunk 3: Finalizacao protegida

### Task 5: Endurecer finalize_preseason

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Test: `src-tauri/src/commands/career.rs`

- [ ] Escrever testes para bloquear plano incompleto, propostas pendentes e ausencia de equipe.
- [ ] Implementar validacoes adicionais.
- [ ] Gerar noticia de abertura da temporada ao finalizar com sucesso.

## Chunk 4: Verificacao

### Task 6: Validacao final

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/db/queries/market_proposals.rs`
- Modify: `src-tauri/src/db/queries/teams.rs`
- Modify: `src-tauri/src/news/generator.rs`

- [ ] Rodar `cargo fmt --manifest-path src-tauri/Cargo.toml`.
- [ ] Rodar testes focados novos.
- [ ] Rodar `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] Rodar `cargo build --manifest-path src-tauri/Cargo.toml`.
