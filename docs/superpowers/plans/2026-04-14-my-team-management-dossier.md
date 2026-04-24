# My Team Management Dossier Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** implementar a nova direcao da aba `Minha Equipe`, com dossie financeiro dominante, ranking no fim e blocos esportivos mais compactos.

**Architecture:** A mudanca fica concentrada em `MyTeamTab`, mas depende de duas frentes pequenas. Primeiro consolidamos o layout e os componentes visuais da nova aba usando os campos financeiros ja expostos. Depois ampliamos o payload para suportar extrato por categorias e linha do tempo mais rica, ligando isso ao frontend com testes focados. O ranking da categoria entra como bloco final reutilizando padroes visuais ja presentes no app.

**Tech Stack:** React, Tauri, Zustand, Vitest

---

## File Structure

- Modify: `src/pages/tabs/MyTeamTab.jsx`
  Responsavel pelo layout principal da aba, novos blocos de dossie financeiro, timeline ampliada e ranking final.
- Modify: `src/pages/tabs/MyTeamTab.test.jsx`
  Cobertura da nova hierarquia visual e leituras financeiras.
- Modify: `src-tauri/src/commands/career_types.rs`
  Expor payload financeiro adicional para o frontend se ainda nao estiver disponivel.
- Modify: `src-tauri/src/commands/career.rs`
  Serializar a leitura financeira detalhada da equipe do jogador.
- Optional Create: `src/pages/tabs/myTeamFinanceMeta.js`
  Helpers de labels, tons e formatacao se `MyTeamTab.jsx` crescer demais.

## Chunk 1: Reestruturar a Hierarquia da Aba

### Task 1: Redesenhar o layout base de `MyTeamTab`

**Files:**
- Modify: `src/pages/tabs/MyTeamTab.jsx`
- Test: `src/pages/tabs/MyTeamTab.test.jsx`

- [ ] **Step 1: Write the failing test**

Adicionar teste cobrindo que a aba:

- renderiza um bloco financeiro principal;
- mostra o ranking de equipes apenas no fim da tela;
- mantem `dupla de pilotos` e `operacao tecnica` como secoes menores.

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/pages/tabs/MyTeamTab.test.jsx`
Expected: FAIL porque a hierarquia atual ainda divide a tela em dois blocos mais equilibrados.

- [ ] **Step 3: Write minimal implementation**

Em `MyTeamTab.jsx`:

- reduzir a presenca visual de `DriverPanel` e do bloco de infraestrutura;
- mover o ranking para o fim da aba;
- introduzir um bloco central de `dossie financeiro`.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/pages/tabs/MyTeamTab.test.jsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/pages/tabs/MyTeamTab.jsx src/pages/tabs/MyTeamTab.test.jsx
git commit -m "feat: restructure my team tab around management dossier"
```

## Chunk 2: Expor o Dossie Financeiro

### Task 2: Mostrar rodada atual e acumulado na aba

**Files:**
- Modify: `src-tauri/src/commands/career_types.rs`
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src/pages/tabs/MyTeamTab.jsx`
- Test: `src/pages/tabs/MyTeamTab.test.jsx`

- [ ] **Step 1: Write the failing test**

Adicionar teste cobrindo que a aba mostra:

- caixa atual;
- resultado da rodada;
- secoes separadas de entradas e saidas da rodada;
- linha do tempo do caixa acumulado.

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/pages/tabs/MyTeamTab.test.jsx`
Expected: FAIL porque a aba atual nao possui granularidade suficiente.

- [ ] **Step 3: Write minimal implementation**

No backend, expor um payload detalhado para a equipe do jogador contendo:

- resumo da rodada atual;
- categorias de receita;
- categorias de despesa;
- historico resumido por rodada para timeline.

No frontend, renderizar:

- cards de resumo;
- extrato por categorias;
- timeline principal do caixa acumulado.

- [ ] **Step 4: Run focused tests**

Run: `npx vitest run src/pages/tabs/MyTeamTab.test.jsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/career_types.rs src-tauri/src/commands/career.rs src/pages/tabs/MyTeamTab.jsx src/pages/tabs/MyTeamTab.test.jsx
git commit -m "feat: show detailed finance dossier in my team tab"
```

## Chunk 3: Graficos e Ranking Final

### Task 3: Ampliar timeline e fechar a tela com ranking comparativo

**Files:**
- Modify: `src/pages/tabs/MyTeamTab.jsx`
- Test: `src/pages/tabs/MyTeamTab.test.jsx`
- Reference: `src/pages/tabs/StandingsTab.jsx`

- [ ] **Step 1: Write the failing test**

Adicionar teste cobrindo que:

- a timeline recebe mais destaque do que os blocos laterais;
- o ranking da categoria aparece no final;
- a linha da equipe do jogador fica destacada.

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/pages/tabs/MyTeamTab.test.jsx`
Expected: FAIL enquanto o ranking ainda nao estiver integrado ao novo fluxo.

- [ ] **Step 3: Write minimal implementation**

Em `MyTeamTab.jsx`:

- ampliar a area da timeline;
- adicionar grafico de composicao de custos;
- renderizar tabela final da categoria com destaque da equipe do jogador.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/pages/tabs/MyTeamTab.test.jsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/pages/tabs/MyTeamTab.jsx src/pages/tabs/MyTeamTab.test.jsx
git commit -m "feat: add finance charts and final team ranking to my team tab"
```

## Chunk 4: Verificacao Final

### Task 4: Validar a leitura da aba

**Files:**
- Verify only: `src/pages/tabs/MyTeamTab.jsx`
- Verify only: `src/pages/tabs/MyTeamTab.test.jsx`
- Verify only: `src-tauri/src/commands/career.rs`
- Verify only: `src-tauri/src/commands/career_types.rs`

- [ ] **Step 1: Run frontend tests**

Run: `npx vitest run src/pages/tabs/MyTeamTab.test.jsx`
Expected: PASS

- [ ] **Step 2: Smoke-check the UI manually**

Verificar manualmente que:

- o financeiro domina a tela;
- a timeline ficou maior que os blocos laterais;
- o ranking fecha a pagina;
- a leitura continua boa em larguras menores.

- [ ] **Step 3: Commit**

```bash
git add src/pages/tabs/MyTeamTab.jsx src/pages/tabs/MyTeamTab.test.jsx src-tauri/src/commands/career.rs src-tauri/src/commands/career_types.rs
git commit -m "feat: finalize my team management dossier layout"
```
