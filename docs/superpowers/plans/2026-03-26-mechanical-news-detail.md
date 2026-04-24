# Mechanical News Detail Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** fazer o sistema de noticias usar os detalhes mecanicos exatos da simulacao, permitir destaque editorial para falhas mecanicas sem DNF e gerar um resumo extra para corridas caoticas.

**Architecture:** a implementacao fica concentrada em `src-tauri/src/news/generator.rs`, reutilizando os `IncidentResult` ja produzidos pela simulacao. O gerador passa a usar `incident.description` como fato primario para mecanicos, amplia a selecao do incidente principal e cria uma noticia-resumo para corridas com volume anormal de incidentes.

**Tech Stack:** Rust, Tauri, modulo de noticias backend, testes unitarios com `cargo test`.

---

## Chunk 1: Fatos mecanicos individuais

### Task 1: Cobrir noticias mecanicas com teste primeiro

**Files:**
- Modify: `src-tauri/src/news/generator.rs`
- Test: `src-tauri/src/news/generator.rs`

- [ ] **Step 1: Write the failing test**
Adicionar um teste que crie um `IncidentResult` mecanico com `description` especifica como `"Piloto X abandona com problema no cambio - sincronizador falhou"` e valide que `build_incident_news_item` preserva esse detalhe no corpo da noticia.

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test --manifest-path src-tauri/Cargo.toml test_incident_news_mechanical_uses_exact_description -- --exact`
Expected: FAIL porque o gerador ainda usa texto generico de template mecanico.

- [ ] **Step 3: Write minimal implementation**
Alterar o branch `IncidentType::Mechanical` em `build_incident_news_item` para usar `incident.description` como base factual do corpo, preservando apenas o enquadramento editorial necessario.

- [ ] **Step 4: Run test to verify it passes**
Run: `cargo test --manifest-path src-tauri/Cargo.toml test_incident_news_mechanical_uses_exact_description -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**
```bash
git add src-tauri/src/news/generator.rs
git commit -m "feat: use exact mechanical incident details in news"
```

### Task 2: Tornar mecanicos sem DNF elegiveis editorialmente

**Files:**
- Modify: `src-tauri/src/news/generator.rs`
- Test: `src-tauri/src/news/generator.rs`

- [ ] **Step 1: Write the failing test**
Adicionar um teste para `select_primary_incident` em que a melhor opcao disponivel seja um `Mechanical` sem DNF e validar que ele passa a ser selecionado acima de incidentes ainda menos relevantes.

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test --manifest-path src-tauri/Cargo.toml test_select_primary_incident_allows_mechanical_non_dnf -- --exact`
Expected: FAIL porque o ranking atual ignora esse caso.

- [ ] **Step 3: Write minimal implementation**
Atualizar `incident_priority` para incluir `IncidentType::Mechanical` sem DNF abaixo de colisao major e acima de erro major generico, sem quebrar a ordem atual de casos mais graves.

- [ ] **Step 4: Run test to verify it passes**
Run: `cargo test --manifest-path src-tauri/Cargo.toml test_select_primary_incident_allows_mechanical_non_dnf -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**
```bash
git add src-tauri/src/news/generator.rs
git commit -m "feat: allow non-dnf mechanical incidents in editorial selection"
```

## Chunk 2: Corrida caotica

### Task 3: Adicionar noticia-resumo para corridas anormais

**Files:**
- Modify: `src-tauri/src/news/generator.rs`
- Test: `src-tauri/src/news/generator.rs`

- [ ] **Step 1: Write the failing test**
Adicionar um teste para `generate_news_from_race` com varios incidentes misturando DNF mecanico e dano mecanico sem DNF, validando que uma noticia-resumo extra e criada e que ela menciona os detalhes exatos dos incidentes.

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test --manifest-path src-tauri/Cargo.toml test_generate_news_from_race_adds_incident_summary_for_chaotic_race -- --exact`
Expected: FAIL porque ainda nao existe resumo de corrida caotica.

- [ ] **Step 3: Write minimal implementation**
Criar um helper no `news/generator` para detectar corrida caotica, selecionar os incidentes mais importantes e montar uma unica noticia-resumo com fatos oriundos de `incident.description`, incluindo contexto de `damage_origin_segment` quando houver.

- [ ] **Step 4: Run test to verify it passes**
Run: `cargo test --manifest-path src-tauri/Cargo.toml test_generate_news_from_race_adds_incident_summary_for_chaotic_race -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**
```bash
git add src-tauri/src/news/generator.rs
git commit -m "feat: add chaotic race incident summary news"
```

### Task 4: Proteger a hierarquia editorial existente

**Files:**
- Modify: `src-tauri/src/news/generator.rs`
- Test: `src-tauri/src/news/generator.rs`

- [ ] **Step 1: Write the failing test**
Adicionar um teste que combine colisao critica com problema mecanico sem DNF e valide que a colisao critica continua sendo o incidente principal.

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test --manifest-path src-tauri/Cargo.toml test_select_primary_incident_keeps_critical_collision_above_mechanical_non_dnf -- --exact`
Expected: FAIL se a nova prioridade interferir incorretamente no ranking.

- [ ] **Step 3: Write minimal implementation**
Ajustar o ranking ou os criterios de desempate apenas se necessario, mantendo colisao critica e casos de DNF forte no topo.

- [ ] **Step 4: Run test to verify it passes**
Run: `cargo test --manifest-path src-tauri/Cargo.toml test_select_primary_incident_keeps_critical_collision_above_mechanical_non_dnf -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**
```bash
git add src-tauri/src/news/generator.rs
git commit -m "test: preserve incident editorial hierarchy"
```

## Chunk 3: Verificacao

### Task 5: Rodar a verificacao final

**Files:**
- Modify: `src-tauri/src/news/generator.rs` only if verification reveals issues

- [ ] **Step 1: Run focused news tests**
Run: `cargo test --manifest-path src-tauri/Cargo.toml news::generator -- --nocapture`
Expected: PASS

- [ ] **Step 2: Run the full backend test suite**
Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 3: Run a backend build**
Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 4: Record any failures honestly and fix before completion**
Se algum comando falhar, corrigir e repetir a verificacao antes de encerrar.
