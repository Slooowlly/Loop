# My Team — "cada número real e com fonte" (design)

Data: 2026-07-11
Escopo aprovado: **corpo da aba + endurecer o drawer** · Persistência: **histórico por rodada**

## 1. Problema

A aba **My Team** (`src/pages/tabs/MyTeamTab.jsx`) exibe blocos que *parecem* dados
financeiros reais, mas são números fabricados no front. A causa raiz é única:

- O backend, em `calculate_team_round_finance_context` (`src-tauri/src/commands/race.rs:80`),
  calcula **a divisão real** de receita/despesa por rodada — 9 linhas
  (`TeamRoundFinanceContext`, `src-tauri/src/finance/cashflow.rs:24`):
  `sponsorship_income`, `result_bonus`, `partial_prize_income`, `aid_income`,
  `salary_expense`, `event_operations_cost`, `structural_maintenance_cost`,
  `technical_investment_cost`, `debt_service_cost`.
- `apply_round_cashflow` (`cashflow.rs:79`) **descarta** essas 9 linhas e grava só os 3
  totais (`last_round_income/expenses/net`) via `update_team_finance_snapshot`
  (`db/queries/teams.rs:559`).
- **Não existe** nenhuma tabela de histórico por rodada.

Com a divisão real jogada fora e sem histórico, o front recria tudo com frações
chumbadas.

### O que é real hoje (NÃO mexer)
`TeamSummary` (`commands/career_types.rs:173`) — que vira `playerTeam` na store — já
carrega reais: `cash_balance`, `debt_balance`, `salary_ceiling`, `spending_power`,
`last_round_income/expenses/net`, `season_strategy`, `parachute_payment_remaining`,
`financial_state`, `car_performance`, atributos técnicos. Portanto o **topo do dossiê**
(KPIs Caixa/Resultado/Dívida/Teto/Poder de gasto), os **totais** dos ledgers, o
`FinancialRiskPanel` e a `ExecutiveReading` são derivações reais. `founded_year` e
`stats_vitorias/pontos` também já vão no payload de `get_teams_standings`.

### Inventário do que é falso

| Widget | Local | Fabricação |
|---|---|---|
| **CostChart** (rosca "custos acumulados") | `MyTeamTab.jsx:467` | 4 linhas 100% hardcoded `42/24/20/14` |
| **Ledgers entradas/saídas** | `incomeRows`/`expenseRows` `:1638` | total real × frações fixas inventadas (`splitAmount`) |
| **Gráfico de caixa R1–R10** | `cashTimeline` `:1663` | retro-projeção `cash - net*(9-i)` + zig-zag cosmético `(i%3)` |
| Secundários (pico/pior trecho) | `:370` | derivam do `cashTimeline` falso → falsos |
| Salário dos pilotos (fallback) | `estimateSalary` `:1609` | teto × share × skill quando o real falta |
| Drawer: ano de fundação | `KNOWN_TEAM_FOUNDING_YEARS` `:35`, `resolveTeamFoundedYear` `:1392` | tabelas literais + interpolação por ranking |
| Drawer: vitórias/pódios/títulos/dívida | `teamWins/teamPodiums/teamTitles/estimateHistoricDebt` `:1446+` | `pontos/24`, `pontos/8`, etc. |
| Drawer: gestão (pico caixa, maior investimento, temporadas saudáveis, eficiência) | `buildTeamHistoryDossier` `:1265` | fórmulas/strings inventadas |

O drawer **já** carrega dados reais via `get_team_history_dossier`
(`commands/career.rs:3213`); as fabricações acima são **fallback** de loading/erro. O
alvo de "endurecer" é substituir esse fallback por estado honesto e usar os campos reais
já disponíveis.

## 2. Arquitetura da solução

### 2.1 Backend — persistir a divisão real por rodada

**Nova tabela `team_finance_history`** (DB por-carreira; ver memória `project_save_location`).

```
team_finance_history (
  id INTEGER PK AUTOINCREMENT,
  team_id TEXT NOT NULL,
  season_number INTEGER NOT NULL,
  round INTEGER NOT NULL,            -- rodada dentro da temporada
  category TEXT NOT NULL,            -- categoria na hora (equipe pode subir/descer)
  -- receita (4)
  sponsorship_income REAL NOT NULL,
  result_bonus REAL NOT NULL,
  partial_prize_income REAL NOT NULL,
  aid_income REAL NOT NULL,
  -- despesa (5)
  salary_expense REAL NOT NULL,
  event_operations_cost REAL NOT NULL,
  structural_maintenance_cost REAL NOT NULL,
  technical_investment_cost REAL NOT NULL,
  debt_service_cost REAL NOT NULL,
  -- resultantes
  income_total REAL NOT NULL,
  expenses_total REAL NOT NULL,
  net REAL NOT NULL,
  cash_balance REAL NOT NULL,        -- caixa APÓS a rodada (para o gráfico)
  debt_balance REAL NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(team_id, season_number, round)
)
INDEX (team_id, season_number, round)
```

- Migração idempotente no estilo das existentes (`db/migrations.rs`). Saves antigos
  nascem sem histórico — aceitável e honesto (ver §3).
- **Gravação:** no laço de pós-corrida (`race.rs:1918-1922`), logo após
  `apply_round_cashflow`, inserir uma linha com o `finance_context` (as 9 linhas) + o
  `cash_balance`/`debt_balance` resultantes. Nova query
  `insert_team_finance_history(conn, &team, &context, season_number, round)`.
  - `season_number` já disponível no escopo (`active_season_number`); `round` vem da
    entrada de calendário da corrida — threadar até o laço.
  - Escrita dentro da mesma transação do snapshot (respeitar `BEGIN IMMEDIATE`, ver
    memória `project_db_locked_simulation`).

**Novo comando `get_team_finance_report(career_id, team_id) -> TeamFinanceReport`**
(registrar em `generate_handler!` de `lib.rs`). Retorna:
- `latest_round`: as 9 linhas + totais da última rodada (para os **ledgers**).
- `season_breakdown`: soma das 9 linhas na temporada atual, agrupada em
  Receitas {Patrocínios, Bônus, Prêmio, Auxílios} e Custos {Salários, Operação,
  Manutenção, Investimento, Serviço da dívida} (para a **rosca** — agora de fato
  "acumulados").
- `cash_timeline`: `[{season_number, round, cash_balance, net}]` das últimas N rodadas
  (para o **gráfico de caixa**, agora real).
- `rounds_recorded`: contagem — o front decide entre estado cheio e estado vazio.

**Endurecer o drawer (dados já existentes):**
- Adicionar `historico_vitorias`, `historico_podios`, `historico_titulos_construtores` a
  `TeamStanding` (`career_types.rs:1147`) e preencher em
  `get_teams_standings_in_base_dir` (`career.rs:3040`). Isso mata os estimadores
  `teamWins/teamPodiums/teamTitles` no ranking e no fallback do drawer.
- `founded_year` já é real no payload; garantir que o `playerTeam` (TeamSummary) também
  o exponha para o caso do próprio time.

### 2.2 Frontend — trocar fabricação por fonte real

`MyTeamTab.jsx` passa a buscar `get_team_finance_report` no mesmo `useEffect` do load
(junto de drivers/teams), com estado de loading/erro.

- **CostChart** → recebe `season_breakdown.custos`. Remove `rows` hardcoded. Sem rodadas
  na temporada → estado vazio ("Sem custos registrados nesta temporada ainda").
- **Ledgers** → `incomeRows`/`expenseRows` passam a mapear `latest_round` (linhas reais).
  Remover `splitAmount` e as frações. Sem última rodada → esconder/estado vazio.
- **cashTimeline** → substituído por `cash_timeline` real. Rótulos = rodada real
  (ex.: `T3·R5`). `< 2` pontos → mensagem em vez de barra fabricada. Pico/pior trecho
  passam a sair do histórico real.
- **Salário dos pilotos** → preferir o valor de contrato real; se ausente, mostrar "—"
  em vez de `estimateSalary`. (Verificar na implementação se `TeamSummary`/contratos já
  trazem `salario_anual`; se sim, remover o estimador.)
- **Drawer** → remover `KNOWN_TEAM_FOUNDING_YEARS`, `CATEGORY_FOUNDING_BASE_YEARS`,
  `resolveTeamFoundedYear` (usar `founded_year` do payload) e os estimadores
  `teamWins/teamPodiums/teamTitles/estimateHistoricDebt/healthySeasonEstimate/
  managementEfficiency`. Onde o dossiê real não tiver o campo, exibir estado neutro
  ("indisponível"/"—"), nunca um número inventado. O `biggestInvestment`/`peakCash`
  back-projetados viram campos reais do dossiê ou estado neutro.

## 3. Decisões e limites

- **Saves antigos** não têm histórico: rosca e ledgers começam vazios e se enchem a cada
  rodada corrida pós-update. **Seed retroativo IMPLEMENTADO**: a migração v43 semeia 1 linha
  `round = 0` ("Início") por equipe que já correu, com os TOTAIS reais do snapshot
  (`last_round_*` + caixa/dívida). As 9 linhas ficam em 0 — o detalhamento da rodada pré-v43
  foi genuinamente descartado e NÃO é reconstruído (seria fabricar). Efeito: o gráfico de
  caixa já tem 1 ponto no 1º acesso; ledgers/rosca (que dependem das 9 linhas) seguem no
  estado honesto até a próxima corrida real. Idempotente por `UNIQUE(team_id, season, round)`.
- **Retenção:** manter todo o histórico da carreira; `cash_timeline` corta para as
  últimas N (ex.: 12) na leitura, não no armazenamento.
- **Granularidade:** 1 linha por equipe por corrida (o `apply_round_cashflow` já roda por
  equipe por evento). Vale para todas as equipes da categoria, não só a do jogador — assim
  o histórico serve também ao drawer de rivais no futuro.
- **Consistência:** `income_total/expenses_total/net` gravados como derivados das linhas
  (fonte única), evitando divergência com `last_round_*` do snapshot.

## 4. Plano de implementação (ordem)

1. Migração `team_finance_history` + testes de coluna (`db/migrations.rs`).
2. `insert_team_finance_history` (`db/queries/teams.rs`) + thre.ar `round`/`season` no
   laço de `race.rs` e gravar junto do snapshot.
3. `TeamFinanceReport` + `get_team_finance_report` + registro no handler.
4. Estender `TeamStanding` com históricos e preencher.
5. Front: buscar o report; reescrever CostChart, Ledgers, cashTimeline.
6. Front: endurecer o drawer (remover estimadores, usar campos reais + estados neutros).
7. Verificação: rodar a carreira algumas rodadas e conferir que rosca/ledgers/gráfico
   batem com o cashflow real; estados vazios em save novo.

## 5. Arquivos afetados

Backend: `db/migrations.rs`, `db/queries/teams.rs`, `commands/race.rs`,
`commands/career.rs`, `commands/career_types.rs`, `commands/career_commands.rs`
(comando), `lib.rs` (handler).
Frontend: `src/pages/tabs/MyTeamTab.jsx` (único arquivo de UI).
