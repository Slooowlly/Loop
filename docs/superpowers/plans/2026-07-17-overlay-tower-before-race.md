# Overlay Tower Before Race Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mostrar a torre no treino e na classificação mesmo antes de existirem posições oficiais.

**Architecture:** O roster YAML é a fonte completa da grade e a telemetria é associada opcionalmente por `CarIdx`. A ordenação separa classificados de não classificados; snapshots/histórico são isolados e validados pelo `SubSessionID`. O canvas recebe `pos = 0` como sentinela e converte apenas a apresentação para travessão.

**Tech Stack:** Rust, Tauri, JavaScript, Vitest, Vite.

---

## Chunk 1: Ordenação e inclusão no backend

### Task 1: Completar a chave de ordenação da torre

**Files:**
- Modify: `src-tauri/src/iracing_sdk/race_monitor.rs`
- Modify: `src-tauri/src/commands/overlay.rs`
- Modify: `src-tauri/src/iracing_sdk/result_bridge.rs` (literal de teste compatível com o novo campo)
- Test: `src-tauri/src/commands/overlay.rs`

- [x] Expor um snapshot ao vivo das voltas de quali sem misturá-lo ao `RaceHistory`, com reset por `SubSessionID` e testes de regressão.
- [x] Usar o roster YAML como fonte e associar telemetria opcional, preservando carros fora do mundo.
- [x] Isolar `RaceHistory` por `SubSessionID` e aplicar fallback de melhor volta por sessão.
- [x] Escrever testes unitários que exijam classificados antes dos não classificados, posição oficial crescente, melhor quali crescente e número como desempate/fallback.
- [x] Rodar o teste antes da implementação e confirmar falha pelas funções ausentes.
- [x] Implementar a menor chave/join/fallback capazes de satisfazer os testes.
- [x] Rodar novamente os testes focados e confirmar sucesso.

## Chunk 2: Posição ainda não oficial no canvas

### Task 2: Desenhar travessão no lugar de zero

**Files:**
- Modify: `src/overlay/towerCanvas.js`
- Create: `src/overlay/towerCanvas.test.js`

- [x] Escrever teste unitário para posição positiva e para zero/negativa.
- [x] Rodar `npm run test:ui -- src/overlay/towerCanvas.test.js` e confirmar falha pela função ausente.
- [x] Implementar um formatador pequeno e usá-lo nos dois estilos de linha do canvas.
- [x] Rodar novamente o teste focado e confirmar sucesso.

## Chunk 3: Verificação integrada

### Task 3: Validar regressões e compilação

**Files:**
- Verify only

- [x] Rodar `npm run test:ui` (290 passaram; 3 falhas externas em Calendar/News).
- [x] Rodar `npm run build` (sucesso).
- [x] Rodar `cargo test --manifest-path src-tauri/Cargo.toml --lib` (testes do overlay passaram; suíte geral revelou falhas externas de migração e foi interrompida após cenários históricos longos).
- [x] Rodar `cargo check --manifest-path src-tauri/Cargo.toml` (sucesso).
- [x] Revisar arquivos do escopo e confirmar que nenhuma alteração alheia foi sobrescrita.
