# Special Convocation Seven-Day Window Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transformar a `Janela de Convocação` em uma simulação especial de `7 dias`, com avanço diário, propostas persistentes, decisão estratégica das equipes e preenchimento progressivo dos grids de `Production` e `Endurance`.

**Architecture:** O backend passa a tratar a janela especial como um estado próprio e temporário, separado do mercado normal, com pool de candidatos, propostas, vagas especiais e log diário. O frontend reutiliza a linguagem visual da `PreSeasonView`, mas muda o comportamento para operar por dia, mostrando o grid especial evoluindo e uma tabela lateral de pilotos elegíveis filtrada por campeonato.

**Tech Stack:** Rust, rusqlite, Tauri, React, Zustand, cargo test, Vitest.

---

## File Map

- `src-tauri/src/convocation/pipeline.rs`
  Responsável por abrir a janela, gerar vagas especiais e coordenar o avanço diário.
- `src-tauri/src/commands/convocation.rs`
  Expõe comandos para carregar o estado da janela, aceitar proposta, avançar dia e finalizar o fluxo.
- `src-tauri/src/commands/career.rs`
  Entrega o payload agregado consumido pela store e pela UI.
- `src-tauri/src/commands/career_types.rs`
  Define os tipos serializados do estado da janela, ofertas, candidatos e log diário.
- `src-tauri/src/db/migrations.rs`
  Cria as estruturas persistentes necessárias para a janela diária.
- `src-tauri/src/db/queries/contracts.rs`
  Continua protegendo a fronteira entre contrato regular e especial.
- `src-tauri/src/convocation/`
  Deve ganhar módulos focados para estado da janela, mercado diário e regras de equipe, em vez de concentrar tudo em um arquivo só.
- `src/stores/useCareerStore.js`
  Coordena carregamento da janela, aceite de proposta, avanço diário e encerramento.
- `src/components/season/ConvocationView.jsx`
  Renderiza a nova experiência diária, grid atualizado e tabela de elegíveis.
- `src/components/season/ConvocationView.test.jsx`
  Cobre a nova hierarquia visual e o comportamento do dia.
- `src/pages/Dashboard.jsx`
  Continua roteando a fase especial para a tela correta.

---

## Chunk 1: Persisted daily window state

### Task 1: Modelar estado persistente da janela especial de 7 dias

**Files:**
- Create: `src-tauri/src/convocation/window_state.rs`
- Modify: `src-tauri/src/convocation/mod.rs`
- Modify: `src-tauri/src/db/migrations.rs`
- Test: `src-tauri/src/convocation/window_state.rs`

- [ ] **Step 1: Write the failing tests**

Adicionar testes cobrindo:
- abertura da janela com `day = 1`;
- limite máximo de `7` dias;
- persistência do status da janela (`aberta`, `encerrada`, `resolvida`);
- independência entre estado especial e contrato regular.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test window_state --manifest-path src-tauri/Cargo.toml`

Expected: FAIL porque o estado diário ainda não existe.

- [ ] **Step 3: Write minimal implementation**

Criar um módulo dedicado com um shape mínimo como:

```rust
pub struct SpecialWindowState {
    pub career_id: String,
    pub day: i32,
    pub status: String,
    pub player_result: Option<String>,
}
```

Persistir esse estado via migração dedicada da convocation window.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test window_state --manifest-path src-tauri/Cargo.toml`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/convocation/window_state.rs src-tauri/src/convocation/mod.rs src-tauri/src/db/migrations.rs
git commit -m "feat: persist special convocation window state"
```

### Task 2: Persistir pool elegível e vagas especiais

**Files:**
- Create: `src-tauri/src/convocation/candidate_pool.rs`
- Create: `src-tauri/src/convocation/team_slots.rs`
- Modify: `src-tauri/src/convocation/pipeline.rs`
- Test: `src-tauri/src/convocation/candidate_pool.rs`
- Test: `src-tauri/src/convocation/team_slots.rs`

- [ ] **Step 1: Write the failing tests**

Cobrir:
- geração do pool elegível ao abrir a janela;
- filtro inicial correto para `Production` e `Endurance`;
- criação das vagas especiais por equipe/classe;
- status inicial dos candidatos como `livre`.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test candidate_pool --manifest-path src-tauri/Cargo.toml`
Run: `cargo test team_slots --manifest-path src-tauri/Cargo.toml`

Expected: FAIL porque pool e vagas ainda não existem como estado dedicado.

- [ ] **Step 3: Write minimal implementation**

Criar dois módulos focados:

```rust
pub struct SpecialCandidate {
    pub driver_id: String,
    pub origin_category: String,
    pub license: String,
    pub desirability: i32,
    pub status: String,
}

pub struct SpecialTeamSlot {
    pub team_id: String,
    pub special_category: String,
    pub class_name: String,
    pub team_strength: i32,
    pub market_profile: String,
    pub status: String,
}
```

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test candidate_pool --manifest-path src-tauri/Cargo.toml`
Run: `cargo test team_slots --manifest-path src-tauri/Cargo.toml`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/convocation/candidate_pool.rs src-tauri/src/convocation/team_slots.rs src-tauri/src/convocation/pipeline.rs
git commit -m "feat: seed special candidate pool and team slots"
```

## Chunk 2: Offer lifecycle and daily market simulation

### Task 3: Persistir propostas especiais com estados diários

**Files:**
- Create: `src-tauri/src/convocation/offers.rs`
- Modify: `src-tauri/src/convocation/mod.rs`
- Modify: `src-tauri/src/convocation/pipeline.rs`
- Modify: `src-tauri/src/commands/career_types.rs`
- Test: `src-tauri/src/convocation/offers.rs`

- [ ] **Step 1: Write the failing tests**

Cobrir:
- criação de propostas no dia correto;
- múltiplas propostas pendentes para o mesmo piloto;
- apenas uma `aceita_ativa` por piloto por dia;
- persistência entre dias;
- expiração quando a equipe fecha com outro piloto.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test offers --manifest-path src-tauri/Cargo.toml`

Expected: FAIL porque o ciclo de vida das ofertas ainda não existe.

- [ ] **Step 3: Write minimal implementation**

Implementar um shape como:

```rust
pub struct SpecialOffer {
    pub id: String,
    pub driver_id: String,
    pub team_id: String,
    pub day_created: i32,
    pub status: String,
}
```

Adicionar helpers para:
- aceitar uma proposta e colocar outras em espera;
- carregar só as ofertas vivas;
- expirar ofertas derrotadas pelo fechamento da vaga.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test offers --manifest-path src-tauri/Cargo.toml`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/convocation/offers.rs src-tauri/src/convocation/mod.rs src-tauri/src/convocation/pipeline.rs src-tauri/src/commands/career_types.rs
git commit -m "feat: add daily special offer lifecycle"
```

### Task 4: Implementar motor de decisão das equipes no fechamento do dia

**Files:**
- Create: `src-tauri/src/convocation/market_engine.rs`
- Modify: `src-tauri/src/convocation/pipeline.rs`
- Modify: `src-tauri/src/convocation/team_slots.rs`
- Test: `src-tauri/src/convocation/market_engine.rs`
- Test: `src-tauri/src/convocation/pipeline.rs`

- [ ] **Step 1: Write the failing tests**

Cobrir:
- equipes fortes mirando pilotos mais cobiçados;
- perfis `agressiva`, `paciente`, `oportunista`, `conservadora`;
- equipes podendo segurar vaga;
- fechamento com melhor piloto entre os que aceitaram;
- não fechamento quando ninguém aceitou.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test market_engine --manifest-path src-tauri/Cargo.toml`
Run: `cargo test convocation --manifest-path src-tauri/Cargo.toml`

Expected: FAIL porque o fechamento estratégico ainda não existe.

- [ ] **Step 3: Write minimal implementation**

Criar um motor isolado para:
- decidir se a equipe age no dia;
- escolher alvos;
- resolver disputa entre pilotos que aceitaram;
- produzir resultado determinístico o bastante para testes.

Exemplo de assinatura:

```rust
pub fn resolve_day_for_slot(
    slot: &SpecialTeamSlot,
    accepted_candidates: &[SpecialCandidate],
    day: i32,
) -> DayResolution
```

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test market_engine --manifest-path src-tauri/Cargo.toml`
Run: `cargo test convocation --manifest-path src-tauri/Cargo.toml`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/convocation/market_engine.rs src-tauri/src/convocation/pipeline.rs src-tauri/src/convocation/team_slots.rs
git commit -m "feat: resolve special market days with team strategy"
```

### Task 5: Registrar log diário para UI e auditoria do mercado

**Files:**
- Create: `src-tauri/src/convocation/daily_log.rs`
- Modify: `src-tauri/src/convocation/pipeline.rs`
- Modify: `src-tauri/src/commands/career_types.rs`
- Test: `src-tauri/src/convocation/daily_log.rs`

- [ ] **Step 1: Write the failing tests**

Cobrir:
- registro de propostas emitidas;
- registro de propostas expiradas;
- registro de pilotos convocados;
- registro de equipes que seguraram vaga.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test daily_log --manifest-path src-tauri/Cargo.toml`

Expected: FAIL porque o log diário ainda não existe.

- [ ] **Step 3: Write minimal implementation**

Criar um módulo simples com eventos serializáveis para o frontend.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test daily_log --manifest-path src-tauri/Cargo.toml`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/convocation/daily_log.rs src-tauri/src/convocation/pipeline.rs src-tauri/src/commands/career_types.rs
git commit -m "feat: add special convocation daily market log"
```

## Chunk 3: Commands and aggregate payloads

### Task 6: Expor comandos de carregar janela, aceitar proposta e avançar dia

**Files:**
- Modify: `src-tauri/src/commands/convocation.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/career_types.rs`
- Test: `src-tauri/src/commands/convocation.rs`

- [ ] **Step 1: Write the failing command tests**

Cobrir:
- carregar estado completo da janela;
- aceitar uma proposta como escolha ativa do dia;
- rejeitar segunda proposta aceita no mesmo dia;
- avançar o dia e receber diff do mercado;
- encerrar a janela no dia 7.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test commands::convocation --manifest-path src-tauri/Cargo.toml`

Expected: FAIL porque o novo fluxo diário ainda não está exposto.

- [ ] **Step 3: Write minimal implementation**

Adicionar comandos Tauri equivalentes a:

```rust
#[tauri::command]
pub fn get_special_window_state(...) -> Result<SpecialWindowPayload, String>

#[tauri::command]
pub fn accept_special_offer_for_day(...) -> Result<SpecialWindowPayload, String>

#[tauri::command]
pub fn advance_special_window_day(...) -> Result<SpecialWindowPayload, String>
```

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test commands::convocation --manifest-path src-tauri/Cargo.toml`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/convocation.rs src-tauri/src/lib.rs src-tauri/src/commands/career_types.rs
git commit -m "feat: expose special convocation day-by-day commands"
```

### Task 7: Entregar payload agregado para dashboard e store

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/commands/career_types.rs`
- Modify: `src-tauri/src/db/queries/contracts.rs`
- Test: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Write the failing integration tests**

Cobrir:
- `load_career` retornando estado diário da janela;
- tabela de elegíveis só com pilotos sem time especial;
- separação entre especial e mercado normal;
- `Production` filtrando licenças compatíveis;
- `Endurance` filtrando elite elegível.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test load_career --manifest-path src-tauri/Cargo.toml`

Expected: FAIL porque o payload agregado ainda não inclui a nova janela diária.

- [ ] **Step 3: Write minimal implementation**

Ampliar o payload serializado com:
- estado da janela;
- ofertas do jogador;
- grid especial atual;
- candidatos elegíveis por filtro;
- log do último fechamento.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test load_career --manifest-path src-tauri/Cargo.toml`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/career.rs src-tauri/src/commands/career_types.rs src-tauri/src/db/queries/contracts.rs
git commit -m "feat: include daily special window payload in career load"
```

## Chunk 4: Frontend day-by-day experience

### Task 8: Adaptar a store para operar a janela especial por dia

**Files:**
- Modify: `src/stores/useCareerStore.js`
- Modify: `src/stores/useCareerStore.test.js`

- [ ] **Step 1: Write the failing store tests**

Cobrir:
- carregar dia atual da janela;
- aceitar uma proposta como escolha ativa;
- impedir segundo aceite ativo no mesmo dia;
- avançar o dia;
- atualizar grid, elegíveis e log após avanço.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `npx vitest run src/stores/useCareerStore.test.js`

Expected: FAIL porque a store ainda trata a janela como decisão estática.

- [ ] **Step 3: Write minimal implementation**

Adicionar ações explícitas como:
- `loadSpecialWindowState`
- `acceptSpecialOfferForDay`
- `advanceSpecialWindowDay`

Manter a limpeza do estado especial ao voltar ao eixo normal.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `npx vitest run src/stores/useCareerStore.test.js`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/stores/useCareerStore.js src/stores/useCareerStore.test.js
git commit -m "feat: drive special convocation window by daily store state"
```

### Task 9: Redesenhar a ConvocationView para dia atual, grid vivo e tabela de elegíveis

**Files:**
- Modify: `src/components/season/ConvocationView.jsx`
- Modify: `src/components/season/ConvocationView.test.jsx`
- Test: `src/pages/Dashboard.test.jsx`

- [ ] **Step 1: Write the failing UI tests**

Cobrir:
- cabeçalho com `Dia X/7`;
- botão de avançar dia;
- grid sendo atualizado com convocados;
- tabela lateral mostrando apenas pilotos sem time especial;
- filtros `Production` e `Endurance`;
- informação de proposta aceita ativa do dia;
- desaparecimento do piloto elegível quando convocado.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `npx vitest run src/components/season/ConvocationView.test.jsx src/pages/Dashboard.test.jsx`

Expected: FAIL porque a view ainda é uma janela estática.

- [ ] **Step 3: Write minimal implementation**

Reaproveitar a base visual atual, mas trocar a semântica para:
- progresso diário no header;
- grid principal com convocações consolidadas;
- coluna lateral de elegíveis filtrados;
- painel do jogador com propostas vivas e escolha ativa do dia;
- CTA principal de avanço para o próximo dia.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `npx vitest run src/components/season/ConvocationView.test.jsx src/pages/Dashboard.test.jsx`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/components/season/ConvocationView.jsx src/components/season/ConvocationView.test.jsx src/pages/Dashboard.test.jsx
git commit -m "feat: render special convocation as seven-day live market"
```

## Chunk 5: Boundary, regressions, and wrap-up

### Task 10: Blindar a fronteira entre mercado normal e especial

**Files:**
- Modify: `src-tauri/src/db/queries/contracts.rs`
- Modify: `src/components/season/PreSeasonView.jsx`
- Modify: `src/components/season/PreSeasonView.test.jsx`
- Test: `src-tauri/src/commands/career.rs`

- [ ] **Step 1: Write the failing regression tests**

Cobrir:
- `Production` e `Endurance` continuando invisíveis na `PreSeasonView`;
- estado da janela especial não vazando para agentes livres regulares;
- contratos especiais não abrindo vagas normais.

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run: `cargo test preseason --manifest-path src-tauri/Cargo.toml`
Run: `npx vitest run src/components/season/PreSeasonView.test.jsx`

Expected: FAIL se o novo fluxo acidentalmente contaminar o mercado normal.

- [ ] **Step 3: Write minimal implementation**

Ajustar filtros e queries necessários para manter a fronteira já aprovada.

- [ ] **Step 4: Run the targeted tests to verify they pass**

Run: `cargo test preseason --manifest-path src-tauri/Cargo.toml`
Run: `npx vitest run src/components/season/PreSeasonView.test.jsx`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/queries/contracts.rs src/components/season/PreSeasonView.jsx src/components/season/PreSeasonView.test.jsx src-tauri/src/commands/career.rs
git commit -m "test: lock special convocation boundary away from preseason"
```

### Task 11: Run end-to-end verification for the seven-day special window

**Files:**
- Modify: `docs/superpowers/specs/2026-04-11-special-convocation-seven-day-window-design.md`
- Modify: `docs/superpowers/plans/2026-04-11-special-convocation-seven-day-window.md`

- [ ] **Step 1: Run backend verification**

Run:
- `cargo test convocation --manifest-path src-tauri/Cargo.toml`
- `cargo test load_career --manifest-path src-tauri/Cargo.toml`
- `cargo test preseason --manifest-path src-tauri/Cargo.toml`

Expected: PASS.

- [ ] **Step 2: Run frontend verification**

Run:
- `npx vitest run src/components/season/ConvocationView.test.jsx src/components/season/PreSeasonView.test.jsx src/stores/useCareerStore.test.js src/pages/Dashboard.test.jsx`

Expected: PASS.

- [ ] **Step 3: Update docs if implementation deviated**

Registrar qualquer ajuste de nomenclatura, payload ou comportamento descoberto durante a execução.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/specs/2026-04-11-special-convocation-seven-day-window-design.md docs/superpowers/plans/2026-04-11-special-convocation-seven-day-window.md
git commit -m "docs: finalize seven-day special convocation implementation notes"
```
