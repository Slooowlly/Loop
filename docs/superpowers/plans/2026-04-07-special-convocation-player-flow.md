# Special Convocation Player Flow Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tornar `Production` e `Endurance` jogáveis para o jogador dentro do bloco especial, com convocações especiais próprias, reaproveitamento da UI existente e avanço rápido quando o jogador não entra no especial.

**Architecture:** O grid especial continua sendo montado pelo pipeline de convocação, mas ganha uma camada explícita de convocações do jogador e uma decisão binária de entrada. O frontend reaproveita a experiência de transição de temporada para a janela de convocação e o dashboard normal passa a resolver `categoria_especial_ativa` quando ela existir, sem criar um fluxo paralelo de campeonato regular.

**Tech Stack:** Rust, rusqlite, Tauri, React, Zustand, cargo test, Vitest.

---

## Chunk 1: Backend state for player convocation

### Task 1: Persistir convocações especiais do jogador

**Files:**
- Create: `src-tauri/src/convocation/player_offers.rs`
- Modify: `src-tauri/src/convocation/mod.rs`
- Modify: `src-tauri/src/convocation/pipeline.rs`
- Modify: `src-tauri/src/db/migrations.rs`
- Test: `src-tauri/src/convocation/player_offers.rs`
- Test: `src-tauri/src/convocation/pipeline.rs`

- [ ] **Step 1: Write the failing tests**

Adicionar testes cobrindo:
- geração de convocações especiais para o jogador após `run_convocation_window`;
- formato mínimo da oferta (`team_id`, `special_category`, `class_name`, `papel`, `status`);
- ausência de mistura com propostas do mercado normal.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test convocation`

Expected: FAIL porque as convocações do jogador ainda não existem como estado próprio.

- [ ] **Step 3: Write minimal implementation**

Implementar um módulo dedicado para ofertas especiais do jogador e persisti-las na janela de convocação.

Exemplo de shape esperado:

```rust
pub struct PlayerSpecialOffer {
    pub id: String,
    pub player_driver_id: String,
    pub team_id: String,
    pub team_name: String,
    pub special_category: String,
    pub class_name: String,
    pub papel: TeamRole,
    pub status: String,
}
```

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test convocation`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/convocation/player_offers.rs src-tauri/src/convocation/mod.rs src-tauri/src/convocation/pipeline.rs src-tauri/src/db/migrations.rs
git commit -m "feat: persist player special convocation offers"
```

### Task 2: Expor comandos para listar e responder convocações especiais

**Files:**
- Modify: `src-tauri/src/commands/convocation.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/convocation/player_offers.rs`
- Test: `src-tauri/src/commands/convocation.rs`

- [ ] **Step 1: Write the failing command tests**

Cobrir:
- listagem das convocações do jogador;
- aceite de uma convocação;
- recusa de uma convocação;
- rejeição de múltiplos aceites simultâneos.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test commands::convocation`

Expected: FAIL porque os comandos ainda não existem.

- [ ] **Step 3: Write minimal implementation**

Adicionar comandos Tauri como:

```rust
#[tauri::command]
pub fn get_player_special_offers(...) -> Result<Vec<PlayerSpecialOffer>, String>

#[tauri::command]
pub fn respond_player_special_offer(...) -> Result<PlayerSpecialOfferResponse, String>
```

Aceitar uma oferta precisa:
- marcar a escolhida como aceita;
- marcar as demais como recusadas/expiradas;
- ativar o contrato especial do jogador;
- preencher `categoria_especial_ativa`.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test commands::convocation`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/convocation.rs src-tauri/src/lib.rs src-tauri/src/convocation/player_offers.rs
git commit -m "feat: add player special convocation commands"
```

## Chunk 2: Special block gameplay semantics

### Task 3: Fazer o jogador correr de verdade no bloco especial

**Files:**
- Modify: `src-tauri/src/commands/race.rs`
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/db/queries/drivers.rs`
- Modify: `src-tauri/src/db/queries/contracts.rs`
- Test: `src-tauri/src/commands/race.rs`
- Test: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Write the failing tests**

Cobrir:
- leitura da categoria ativa do jogador preferindo `categoria_especial_ativa`;
- simulação de corrida especial usando lineup especial correto;
- `load_career` e dados de dashboard refletindo o especial quando ativo.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test race`
Run: `cargo test career`

Expected: FAIL porque o fluxo ainda depende só de `categoria_atual`.

- [ ] **Step 3: Write minimal implementation**

Ajustar resolução de categoria ativa do jogador e consultas derivadas.

Regra central esperada:

```rust
let categoria_ativa = player
    .categoria_especial_ativa
    .clone()
    .or(player.categoria_atual.clone());
```

As telas e comandos que dependem da categoria do jogador devem usar essa resolução.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test race`
Run: `cargo test career`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/race.rs src-tauri/src/commands/career.rs src-tauri/src/db/queries/drivers.rs src-tauri/src/db/queries/contracts.rs
git commit -m "feat: route player flow through active special category"
```

### Task 4: Implementar a simulação rápida quando o jogador ficar fora

**Files:**
- Modify: `src-tauri/src/commands/convocation.rs`
- Modify: `src-tauri/src/convocation/pipeline.rs`
- Modify: `src-tauri/src/commands/race.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/commands/convocation.rs`
- Test: `src-tauri/src/convocation/pipeline.rs`

- [ ] **Step 1: Write the failing tests**

Cobrir:
- comando de simulação rápida do bloco especial;
- avanço até `PosEspecial` quando o jogador não está no especial;
- limpeza correta antes da entrada no mercado normal.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test convocation`

Expected: FAIL porque `simulate_special_block` ainda não existe de verdade.

- [ ] **Step 3: Write minimal implementation**

Implementar um comando real de simulação em lote do bloco especial, ou alinhar a store a um novo comando equivalente.

Esse comando deve:
- simular todas as corridas especiais pendentes;
- permitir `encerrar_bloco_especial`;
- deixar `run_pos_especial` pronto para ser executado em seguida.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test convocation`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/convocation.rs src-tauri/src/convocation/pipeline.rs src-tauri/src/commands/race.rs src-tauri/src/lib.rs
git commit -m "feat: support fast simulation of special block when player is absent"
```

## Chunk 3: Frontend reuse and player interaction

### Task 5: Reaproveitar a tela de transição para a JanelaConvocacao

**Files:**
- Create: `src/components/season/ConvocationView.jsx`
- Create: `src/components/season/ConvocationView.test.jsx`
- Modify: `src/pages/Dashboard.jsx`
- Modify: `src/stores/useCareerStore.js`
- Modify: `src/stores/useCareerStore.test.js`

- [ ] **Step 1: Write the failing UI/store tests**

Cobrir:
- exibição da tela de convocação quando `showConvocation` estiver ativo;
- carregamento das ofertas especiais do jogador;
- ações de aceitar e recusar.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `npm test -- useCareerStore.test.js ConvocationView.test.jsx Dashboard.test.jsx`

Expected: FAIL porque a tela e o fluxo ainda não existem.

- [ ] **Step 3: Write minimal implementation**

Criar `ConvocationView.jsx` reaproveitando a lógica visual de cards/listas da transição de temporada, mas mostrando apenas:
- `Production`;
- `Endurance`;
- classes;
- equipes;
- ofertas do jogador.

No store:
- carregar ofertas especiais;
- responder ofertas;
- confirmar início do bloco especial;
- chamar simulação rápida quando o jogador ficar fora.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `npm test -- useCareerStore.test.js ConvocationView.test.jsx Dashboard.test.jsx`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/components/season/ConvocationView.jsx src/components/season/ConvocationView.test.jsx src/pages/Dashboard.jsx src/stores/useCareerStore.js src/stores/useCareerStore.test.js
git commit -m "feat: reuse season transition UI for special convocation window"
```

### Task 6: Fazer dashboard e telas derivadas respeitarem o especial ativo

**Files:**
- Modify: `src/pages/tabs/StandingsTab.jsx`
- Modify: `src/pages/tabs/CalendarTab.jsx`
- Modify: `src/pages/tabs/MyTeamTab.jsx`
- Modify: `src/pages/tabs/NextRaceTab.jsx`
- Modify: `src/components/layout/Header.jsx`
- Test: `src/pages/tabs/NextRaceTab.test.jsx`
- Test: `src/pages/Dashboard.test.jsx`

- [ ] **Step 1: Write the failing tests**

Cobrir:
- resolução da categoria exibida usando o especial ativo;
- ausência de referência à categoria regular em paralelo;
- próxima corrida apontando para o especial quando aceito.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `npm test -- NextRaceTab.test.jsx Dashboard.test.jsx`

Expected: FAIL porque a UI ainda assume só a categoria regular.

- [ ] **Step 3: Write minimal implementation**

Centralizar no frontend uma resolução simples da categoria do jogador:

```js
const activeCategory =
  player?.categoria_especial_ativa || playerTeam?.categoria || season?.categoria_jogador;
```

Aplicar isso apenas onde a UI depende da categoria jogável atual.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `npm test -- NextRaceTab.test.jsx Dashboard.test.jsx`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/pages/tabs/StandingsTab.jsx src/pages/tabs/CalendarTab.jsx src/pages/tabs/MyTeamTab.jsx src/pages/tabs/NextRaceTab.jsx src/components/layout/Header.jsx src/pages/tabs/NextRaceTab.test.jsx src/pages/Dashboard.test.jsx
git commit -m "feat: make dashboard consume active special category"
```

## Chunk 4: Integrated verification

### Task 7: Rodar verificação focada de ponta a ponta

**Files:**
- Verify: `src-tauri/src/convocation/player_offers.rs`
- Verify: `src-tauri/src/convocation/pipeline.rs`
- Verify: `src-tauri/src/commands/convocation.rs`
- Verify: `src-tauri/src/commands/race.rs`
- Verify: `src-tauri/src/commands/career.rs`
- Verify: `src/components/season/ConvocationView.jsx`
- Verify: `src/stores/useCareerStore.js`
- Verify: `src/pages/Dashboard.jsx`

- [ ] **Step 1: Run focused backend tests**

Run: `cargo test convocation`
Run: `cargo test race`
Run: `cargo test career`

Expected: PASS.

- [ ] **Step 2: Run focused frontend tests**

Run: `npm test -- useCareerStore.test.js ConvocationView.test.jsx Dashboard.test.jsx NextRaceTab.test.jsx`

Expected: PASS.

- [ ] **Step 3: Run a smoke flow around the special season**

Run: `cargo test special`

Expected: PASS for the player acceptance/rejection path and special teardown path.

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "test: verify player special convocation flow"
```
