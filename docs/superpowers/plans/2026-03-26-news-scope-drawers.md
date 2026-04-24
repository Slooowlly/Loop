# News Scope Families Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Manter as familias compactas da aba de noticias e fazer os campeonatos compartilhados respeitarem a classe ativa em todo o recorte.

**Architecture:** `NewsScopeDrawers.jsx` continua controlando a familia aberta e passa a anexar `scope_class` quando o usuario entra em `Production` ou `Endurance` por uma familia especifica. `NewsTab.jsx` propaga esse subescopo para a snapshot. No backend, `news_tab.rs` usa `scope_id + scope_class` para filtrar historias, briefing e chips contextuais em campeonatos multiclasses.

**Tech Stack:** React, Vitest, Testing Library, Rust, Tauri

---

## Chunk 1: Class-aware shared scopes

### Task 1: Cobrir o envio e o filtro de classe com testes

**Files:**
- Modify: `src/pages/tabs/NewsTab.test.jsx`
- Modify: `src-tauri/src/commands/news_tab.rs`

- [ ] **Step 1: Escrever teste falhando no frontend para `scope_class`**

Adicionar teste que clique em `Production` ou `Endurance` dentro de uma familia e confirme o `scope_class`.

- [ ] **Step 2: Escrever teste falhando no backend para recorte multiclasses**

Adicionar teste que prove que `production_challenger + mazda` exclui historias e filtros de BMW/Toyota.

- [ ] **Step 3: Rodar o teste e confirmar falha**

Run: `npm test -- src/pages/tabs/NewsTab.test.jsx` e `cargo test test_news_tab_snapshot_shared_scope_class --manifest-path src-tauri/Cargo.toml`
Expected: falha porque o recorte ainda nao respeita a classe compartilhada.

### Task 2: Propagar `scope_class` do drawer ate a snapshot

**Files:**
- Modify: `src/pages/tabs/NewsTab.jsx`
- Modify: `src/pages/tabs/NewsScopeDrawers.jsx`

- [ ] **Step 4: Anexar `scope_class` aos itens compartilhados**

Configurar `Production` e `Endurance` com a classe da familia correspondente.

- [ ] **Step 5: Persistir `scope_class` no estado da aba**

Enviar o novo campo no `invoke` da snapshot.

- [ ] **Step 6: Ajustar destaque visual da familia ativa**

Garantir que o item compartilhado destaque a familia correta quando o filtro vier de `scope_class`.

### Task 3: Aplicar o recorte no backend

**Files:**
- Modify: `src-tauri/src/commands/career_types.rs`
- Modify: `src-tauri/src/commands/news_tab.rs`

- [ ] **Step 7: Aceitar `scope_class` no request/meta**

Adicionar o campo opcional nos tipos serializados.

- [ ] **Step 8: Filtrar equipes e historias por classe**

Usar `scope_id + scope_class` para montar o conjunto ativo de equipes e historias.

- [ ] **Step 9: Filtrar briefing e chips contextuais pela mesma classe**

Aplicar o mesmo conjunto ativo em briefing, filtros de pilotos e filtros de equipes.

### Task 4: Verificacao final

**Files:**
- Modify: `docs/superpowers/specs/2026-03-26-news-scope-drawers-design.md`
- Modify: `docs/superpowers/plans/2026-03-26-news-scope-drawers.md`
- Verify: `package.json`

- [ ] **Step 10: Rodar os testes da aba**

Run: `npm test -- src/pages/tabs/NewsTab.test.jsx`
Expected: PASS

- [ ] **Step 11: Rodar o teste dedicado do backend**

Run: `cargo test test_news_tab_snapshot_shared_scope_class --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 12: Rodar o build**

Run: `npm run build`
Expected: build concluido com sucesso
