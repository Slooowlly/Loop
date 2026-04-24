# Seasonal Car Build Strategy Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implementar um sistema sazonal de construcao do carro em que as pistas carregam pesos de aceleracao/potencia/dirigibilidade, as equipes escolhem um perfil tecnico na offseason e a simulacao converte esse encaixe em `effective_car_performance`.

**Architecture:** A implementacao fica dividida em cinco frentes pequenas. Primeiro adicionamos pesos explicitos ao catalogo de pista e um helper canonicamente testado de `track_delta`. Depois persistimos o perfil de construcao do carro nas equipes com migration segura. Em seguida ligamos esse valor ao `SimDriver`, ensinamos a IA da preseason a escolher o perfil da proxima temporada e, por fim, expomos o perfil para debug/UI minima.

**Tech Stack:** Rust, Tauri, rusqlite, serde, rand, Vitest, React

---

## Chunk 1: Pistas Com Pesos De Carro

### Task 1: Cobrir a nova estrutura de pesos por pista

**Files:**
- Modify: `src-tauri/src/simulation/track_profile.rs`
- Test: `src-tauri/src/simulation/track_profile.rs`

- [ ] **Step 1: Write the failing tests**

Adicionar testes unitarios cobrindo:
- uma pista `Balanced` de referencia neutra (`Sebring` ou `Imola`) com pesos esperados;
- uma pista extrema de potencia (`Monza`);
- uma pista extrema de aceleracao (`Tsukuba`);
- uma pista extrema de dirigibilidade (`Ledenon`);
- pista desconhecida usando fallback `35 / 30 / 35`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test track_profile`
Expected: FAIL porque `TrackSimulationData` ainda nao possui os novos campos.

- [ ] **Step 3: Write minimal implementation**

Em `src-tauri/src/simulation/track_profile.rs`:
- estender `TrackSimulationData` com `acceleration_weight`, `power_weight`, `handling_weight`;
- atualizar `TrackSimulationData::new(...)` para receber os tres pesos;
- preencher a tabela aprovada no brainstorming para todas as pistas explicitamente mapeadas;
- definir fallback `Technical` neutro com `35 / 30 / 35`;
- adicionar helper puro para retornar os pesos da baseline balanceada.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test track_profile`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/simulation/track_profile.rs
git commit -m "feat: add car attribute weights to track profiles"
```

### Task 2: Criar o calculo canonico de encaixe pista x carro

**Files:**
- Create: `src-tauri/src/simulation/car_build.rs`
- Modify: `src-tauri/src/simulation/mod.rs`
- Test: `src-tauri/src/simulation/car_build.rs`

- [ ] **Step 1: Write the failing tests**

Adicionar testes unitarios para:
- `Balanced` resultar em `delta ~= 0` em qualquer pista;
- perfil certo em `Tsukuba` gerar delta positivo;
- perfil errado em `Monza` gerar delta negativo;
- `clamp` em `-6.0` / `+6.0`;
- comparacao direta entre `AccelerationExtreme` e `PowerExtreme` em pista de aceleracao.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test car_build`
Expected: FAIL porque o modulo e os helpers ainda nao existem.

- [ ] **Step 3: Write minimal implementation**

Em `src-tauri/src/simulation/car_build.rs`:
- criar enum `CarBuildProfile` com os 7 perfis aprovados;
- criar helper `weights_for_profile(profile) -> (f64, f64, f64)`;
- criar helper `profile_cost_tier(profile)` ou equivalente;
- implementar `team_match_score`, `balanced_match_score`, `track_advantage`, `track_delta`;
- documentar a formula canonica:

```rust
advantage = team_match - balanced_match;
delta = (advantage / 2.5).clamp(-6.0, 6.0);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test car_build`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/simulation/mod.rs src-tauri/src/simulation/car_build.rs
git commit -m "feat: add seasonal car build scoring helpers"
```

## Chunk 2: Persistencia Do Perfil Da Equipe

### Task 3: Persistir o perfil de construcao do carro nas equipes

**Files:**
- Modify: `src-tauri/src/models/team.rs`
- Modify: `src-tauri/src/db/queries/teams.rs`
- Modify: `src-tauri/src/db/migrations.rs`
- Test: `src-tauri/src/db/queries/teams.rs`
- Test: `src-tauri/src/models/team.rs`

- [ ] **Step 1: Write the failing tests**

Adicionar testes cobrindo:
- `Team::from_template_with_rng` inicializando times novos com `Balanced`;
- `insert_team` + `get_team_by_id` preservando `car_build_profile`;
- leitura de row legado sem coluna preenchida caindo em `Balanced`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test team`
Expected: FAIL porque `Team` e o schema ainda nao conhecem `car_build_profile`.

- [ ] **Step 3: Write minimal implementation**

Em `src-tauri/src/models/team.rs`:
- adicionar enum serializavel `CarBuildProfile` reutilizando o tipo canonico de `simulation/car_build.rs` ou movendo o enum para um modulo compartilhado;
- adicionar campo `car_build_profile` ao `Team`;
- inicializar times novos como `Balanced`.

Em `src-tauri/src/db/migrations.rs`:
- adicionar coluna `car_build_profile TEXT NOT NULL DEFAULT 'balanced'` em `teams`;
- garantir compatibilidade com banco novo e migracao incremental.

Em `src-tauri/src/db/queries/teams.rs`:
- incluir a coluna em `insert_team`, `update_team` e `team_from_row`;
- fazer fallback seguro para `'balanced'` em saves legados.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test team`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/models/team.rs src-tauri/src/db/queries/teams.rs src-tauri/src/db/migrations.rs
git commit -m "feat: persist seasonal car build profile on teams"
```

## Chunk 3: Integrar O Carro Efetivo Na Simulacao

### Task 4: Aplicar o ajuste da pista ao `SimDriver`

**Files:**
- Modify: `src-tauri/src/simulation/context.rs`
- Modify: `src-tauri/src/commands/race.rs`
- Test: `src-tauri/src/simulation/context.rs`
- Test: `src-tauri/src/commands/race.rs`

- [ ] **Step 1: Write the failing tests**

Adicionar testes cobrindo:
- `SimDriver` recebendo `effective_car_performance` diferente do `team.car_performance` bruto quando a pista favorece o perfil;
- `Balanced` mantendo `effective_car_performance ~= base`;
- grid de corrida em pista de potencia favorecendo time focado em potencia sobre outro com mesma base e perfil errado.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test sim_driver`
Expected: FAIL porque `SimDriver::from_driver_and_team` ainda nao conhece pista nem perfil.

- [ ] **Step 3: Write minimal implementation**

Em `src-tauri/src/simulation/context.rs`:
- criar uma variante orientada a evento, por exemplo `SimDriver::from_driver_team_and_track(...)`;
- calcular `effective_car_performance` usando `track_delta` + `team.car_performance`;
- opcionalmente armazenar tambem `base_car_performance` e `car_track_delta` para debug.

Em `src-tauri/src/commands/race.rs`:
- trocar a construcao de `SimDriver` para usar o `track_id` da corrida;
- manter `qualifying.rs` e `race.rs` sem reescrever a formula central, reaproveitando `driver.car_performance`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test sim_driver`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/simulation/context.rs src-tauri/src/commands/race.rs
git commit -m "feat: apply track-aware car performance in simulation"
```

### Task 5: Proteger o efeito com testes de regressao numerica

**Files:**
- Modify: `src-tauri/src/simulation/qualifying.rs`
- Modify: `src-tauri/src/simulation/race.rs`
- Test: `src-tauri/src/simulation/qualifying.rs`
- Test: `src-tauri/src/simulation/race.rs`

- [ ] **Step 1: Write the failing tests**

Adicionar testes de regressao cobrindo:
- mesmo piloto + mesma base de carro + perfis opostos produzindo ordem diferente em `Monza`;
- mesmo piloto + mesma base de carro + perfis opostos produzindo ordem diferente em `Tsukuba`;
- `Balanced` ficando entre extremos na pista certa.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test simulation::qualifying cargo test simulation::race`
Expected: FAIL com a grade antiga sem sensibilidade ao novo perfil.

- [ ] **Step 3: Write minimal implementation**

Se necessario:
- ajustar builders de teste para popular `car_build_profile`;
- manter o restante da logica de quali/corrida inalterado.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test simulation::qualifying`
Expected: PASS

Run: `cargo test simulation::race`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/simulation/qualifying.rs src-tauri/src/simulation/race.rs
git commit -m "test: cover car build impact across qualifying and race"
```

## Chunk 4: IA Da Offseason Escolhendo O Carro Da Temporada

### Task 6: Ensinar a preseason a escolher o perfil do carro

**Files:**
- Modify: `src-tauri/src/market/preseason.rs`
- Modify: `src-tauri/src/db/queries/teams.rs`
- Modify: `src-tauri/src/commands/career.rs`
- Test: `src-tauri/src/market/preseason.rs`

- [ ] **Step 1: Write the failing tests**

Adicionar testes de preseason cobrindo pelo menos:
- equipe forte, rica e segura escolhendo `Balanced` em calendario misto;
- equipe pobre e sob risco escolhendo `PowerExtreme` ou `PowerIntermediate` quando o calendario favorece potencia;
- equipe em contexto de promocao/rebaixamento olhando para o calendario da categoria seguinte quando aplicavel;
- `budget` afetando a decisao sem bloquear totalmente um perfil melhor.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test initialize_preseason`
Expected: FAIL porque nenhuma escolha de `car_build_profile` ocorre hoje.

- [ ] **Step 3: Write minimal implementation**

Em `src-tauri/src/market/preseason.rs`:
- criar helper puro de score por perfil:
  - `calendar_fit`
  - `strategy_bias`
  - `budget_bias`
  - `car_strength_bias`
  - `movement_bias`
- aplicar a escolha para todas as equipes relevantes antes de simular o mercado/plano da temporada;
- persistir o perfil escolhido com `update_team`.

Se o arquivo ficar grande demais, extrair a logica para um novo modulo focado, por exemplo `src-tauri/src/market/car_build_strategy.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test initialize_preseason`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/market/preseason.rs src-tauri/src/db/queries/teams.rs src-tauri/src/commands/career.rs
git commit -m "feat: choose seasonal car build profiles during preseason"
```

## Chunk 5: Observabilidade E UI Minima

### Task 7: Expor o perfil atual da equipe para a carreira

**Files:**
- Modify: `src-tauri/src/commands/career_types.rs`
- Modify: `src-tauri/src/commands/career.rs`
- Test: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Write the failing test**

Adicionar teste cobrindo que `player_team` serializa `car_build_profile` para a UI.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test player_team`
Expected: FAIL porque `TeamSummary` ainda nao exporta o novo campo.

- [ ] **Step 3: Write minimal implementation**

Em `src-tauri/src/commands/career_types.rs`:
- adicionar `car_build_profile: String`.

Em `src-tauri/src/commands/career.rs`:
- incluir o valor em `build_team_summary(...)`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test player_team`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/career_types.rs src-tauri/src/commands/career.rs
git commit -m "feat: expose team car build profile in career payload"
```

### Task 8: Mostrar o perfil do carro na tela da equipe

**Files:**
- Modify: `src/pages/tabs/MyTeamTab.jsx`
- Test: `src/pages/tabs/MyTeamTab.test.jsx`

- [ ] **Step 1: Write the failing test**

Adicionar teste cobrindo que a aba `My Team` renderiza o perfil do carro atual com label legivel.

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/pages/tabs/MyTeamTab.test.jsx`
Expected: FAIL porque a tela ainda nao mostra `car_build_profile`.

- [ ] **Step 3: Write minimal implementation**

Em `src/pages/tabs/MyTeamTab.jsx`:
- exibir um resumo do perfil atual (`Balanceado`, `Foco em Potencia`, etc.);
- manter o restante dos medidores intacto.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/pages/tabs/MyTeamTab.test.jsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/pages/tabs/MyTeamTab.jsx src/pages/tabs/MyTeamTab.test.jsx
git commit -m "feat: show seasonal car build profile on my team tab"
```

## Chunk 6: Verificacao Final

### Task 9: Rodar a suite focada do backend

**Files:**
- Verify only: `src-tauri/src/simulation/track_profile.rs`
- Verify only: `src-tauri/src/simulation/car_build.rs`
- Verify only: `src-tauri/src/simulation/context.rs`
- Verify only: `src-tauri/src/market/preseason.rs`
- Verify only: `src-tauri/src/db/queries/teams.rs`

- [ ] **Step 1: Run focused backend tests**

Run: `cargo test track_profile`
Expected: PASS

Run: `cargo test car_build`
Expected: PASS

Run: `cargo test sim_driver`
Expected: PASS

Run: `cargo test initialize_preseason`
Expected: PASS

Run: `cargo test team`
Expected: PASS

### Task 10: Rodar a verificacao focada do frontend

**Files:**
- Verify only: `src/pages/tabs/MyTeamTab.jsx`

- [ ] **Step 1: Run focused frontend tests**

Run: `npx vitest run src/pages/tabs/MyTeamTab.test.jsx`
Expected: PASS

- [ ] **Step 2: Review manual smoke points**

Verificar na UI:
- `My Team` mostra o perfil do carro atual;
- o profile carregado bate com a equipe do jogador;
- a carreira nao quebra ao carregar save legado.
