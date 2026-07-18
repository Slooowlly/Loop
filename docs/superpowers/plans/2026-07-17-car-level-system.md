# Sistema de Nível do Carro — Plano de Implementação

> **Para workers agênticos:** REQUIRED: use superpowers:subagent-driven-development (se houver subagents) ou superpowers:executing-plans. Passos usam checkbox (`- [ ]`).
>
> Design de referência: `docs/superpowers/specs/2026-07-17-car-level-system-design.md`.

**Goal:** O carro nasce de 11 peças, tem Nível 1–10 (única coisa visível ao jogador), evolui a cada corrida sob decisão do cérebro do time, e o spread de níveis (por categoria e por orçamento) faz o carro decidir corridas. Export pro iRacing = fora de escopo (fase futura).

**Architecture:** Um módulo puro `car/` modela peças → PHA → Nível → custo (§Chunks 1–2). O cérebro do time (`market/car_build_strategy.rs`) decide trocar/esticar/degradar por corrida, olhando calendário + orçamento (Chunk 3). O estado persiste por time no save (Chunk 4). A simulação lê magnitude→`car_performance` e shape→vetor PHA, aposentando o `CarBuildProfile` discreto (Chunk 5). Finanças ganham a depreciação real (Chunk 6). A My Team mostra só o Nível e perde o seletor de perfil (Chunk 7).

**Tech Stack:** Rust, Tauri, JavaScript, Vitest, Vite. Compilar com `CARGO_TARGET_DIR` fora do OneDrive.

**Princípios inegociáveis (do design):**
- Jogador NÃO investe, NÃO vê shape. Só o Nível 1–10.
- Nível domina; shape só fura fila em pista mono-tema (regra de dominância).
- Rookie = spec puro (tudo nível 1, sem gestão de peças).
- Os números "assumidos" (§12 do design) entram como constantes calibráveis, validadas no Chunk 8.

---

## Chunk 1: Modelo do carro (peças, PHA, Nível, custo, tetos)

### Task 1: Definir as 11 peças e derivar PHA + Nível

**Files:**
- Create: `src-tauri/src/car/mod.rs`
- Create: `src-tauri/src/car/parts.rs`
- Modify: `src-tauri/src/lib.rs` (registrar `mod car`)
- Test: `src-tauri/src/car/parts.rs`

- [x] Escrever testes: `Car` com 11 `CarPart { part_type, level, wear }` produz `pha() -> (P,H,A)` somando o viés de cada peça escalado pelo nível; `display_level()` = round(média dos níveis) clampado em 1–10; carro rookie spec (tudo nível 1) dá `display_level()==1` e shape neutro.
- [x] Rodar os testes e confirmar falha pelas funções ausentes.
- [x] Definir a tabela de peças (durabilidade, viés PHA por nível, custo-base relativo) como dado; implementar `pha()`, `magnitude()` (total PHA contínuo) e `display_level()`.
- [x] Rodar novamente os testes focados e confirmar sucesso.

### Task 2: Curva de custo por categoria com teto suave

**Files:**
- Create: `src-tauri/src/car/cost.rs`
- Modify: `src-tauri/src/car/mod.rs`
- Test: `src-tauri/src/car/cost.rs`

- [x] Escrever testes: `part_cost(cat, part, level) = base(cat)·1,2385^(level-1)·parede(level, teto_cat)`; abaixo do teto o incremento é +23,85%; 1 nível acima ≈ +59%, 2 acima ≈ +94% (parede = +23,85% + 35%·níveis_acima); `nível 5` custa muito mais no amador (teto 2) que no gt3 (teto 7).
- [x] Rodar os testes e confirmar falha.
- [x] Implementar a tabela de tetos (rookie 1, amador 2, production 4, bmw 3, gt4 6, gt3 7, lmp2/endurance 8) e a curva; `base(cat)` como constante calibrável ancorada no orçamento da categoria.
- [x] Rodar novamente os testes focados e confirmar sucesso.

## Chunk 2: Desgaste e ciclo de vida da peça

### Task 3: Acúmulo de desgaste e as 3 saídas (trocar/esticar/degradar)

**Files:**
- Create: `src-tauri/src/car/wear.rs`
- Modify: `src-tauri/src/car/mod.rs`
- Test: `src-tauri/src/car/wear.rs`

- [x] Escrever testes: desgaste sobe ~`100/durabilidade`%/corrida; **esticar** só habilita com wear ≤95%, custa ~40% de peça nova, concede +1 corrida e depois marca morte obrigatória; **degradar** acima de 100% derruba 1 nível/corrida (PHA e base de custo caem junto); **trocar** zera o wear.
- [x] Rodar os testes e confirmar falha.
- [x] Implementar `advance_race(&mut Car, decisions)`, `can_stretch(part) -> bool`, `stretch_cost/replace_cost`, e a queda de nível por degradação — como transformações puras (sem DB).
- [x] Rodar novamente os testes focados e confirmar sucesso.

## Chunk 3: Cérebro de estratégia do time

### Task 4: Horizonte de planejamento (traço por time, varia por temporada)

**Files:**
- Modify: `src-tauri/src/market/car_build_strategy.rs`
- Test: `src-tauri/src/market/car_build_strategy.rs`

- [x] Escrever testes: sorteio de horizonte segue 20% temporada / 30% 5-corridas / 30% 3-corridas / 20% míope (1 pista); o horizonte é determinístico por (team_id, season) e re-rola por temporada; distribuição estável sobre N times.
- [x] Rodar os testes e confirmar falha.
- [x] Implementar `planning_horizon(team_id, season) -> Horizon` (determinístico via hash, sem `Math::random`) com a distribuição.
- [x] Rodar novamente os testes focados e confirmar sucesso.

> **Nota de implementação:** o cérebro foi criado num módulo irmão novo, `market/car_maintenance.rs` (não em `car_build_strategy.rs`), pra separar o motor novo do sistema legado de perfil discreto (`CarBuildProfile`) que o Chunk 5 vai aposentar. `#![allow(dead_code)]` próprio.

### Task 5: Decisão orçada olhando o calendário à frente

**Files:**
- Modify: `src-tauri/src/market/car_build_strategy.rs`
- Test: `src-tauri/src/market/car_build_strategy.rs`

- [x] Escrever testes: dado orçamento + próximas N pistas (N = horizonte) com suas demandas PHA, o brain prioriza as peças do atributo que a próxima onda exige; time rico atinge/mantém o teto da categoria, time pobre trava abaixo; escolhe **esticar** quando sem caixa mas a próxima pista exige aquele atributo; **degrada** peça irrelevante pra próxima pista.
- [x] Rodar os testes e confirmar falha.
- [x] Implementar `decide_car_maintenance(team, car, budget, upcoming_tracks) -> Decisions` reaproveitando os pesos PHA de pista existentes (`track_profile`/`car_build::dot_match_score`).
- [x] Rodar novamente os testes focados e confirmar sucesso.

## Chunk 4: Persistência e ciclo de vida no save

### Task 6: Migration e queries do estado do carro por time

**Files:**
- Modify: `src-tauri/src/db/migrations.rs`
- Create: `src-tauri/src/db/queries/team_car.rs`
- Modify: `src-tauri/src/db/queries/mod.rs`
- Test: `src-tauri/src/db/queries/team_car.rs`

- [x] Escrever testes: gravar/ler `team_car` (team_id, part_type, level, wear) round-trip; idempotência por (career/save, team, part).
- [x] Rodar os testes e confirmar falha.
- [x] Adicionar migration (nova tabela `team_car`) e `get_team_car`/`upsert_team_car`.
- [x] Rodar novamente os testes focados e confirmar sucesso.

> **Feito:** migration **v48** (`team_car`, uma linha por peça, PK `(team_id, part_type)`) + `db/queries/team_car.rs` (`get_team_car`/`upsert_team_car`, com `ensure_table` idempotente para testes in-memory). 4 testes verdes. `PartType::as_str`/`from_str` para persistência.

### Task 7: Seed inicial e transição de temporada

**Files:**
- Modify: `src-tauri/src/generators/world.rs` (seed do carro por time)
- Modify: `src-tauri/src/evolution/season_transition.rs` (roll de temporada + re-roll de horizonte)
- Test: nos respectivos arquivos

- [x] Escrever testes: no seed, o nível inicial do carro correlaciona com orçamento/prestígio do time e respeita o teto da categoria; rookie recebe carro spec (tudo nível 1); na virada de temporada o desgaste/nível persistem e o horizonte re-rola.
- [x] Rodar os testes e confirmar falha.
- [x] Implementar seed correlacionado e o passo de transição de temporada.
- [x] Rodar novamente os testes focados e confirmar sucesso.

> **Feito:** `car/seed.rs` (`seed_car(cat, quality)` puro, piso = 40% do teto, rookie=spec) + `car_maintenance::seed_and_persist_team_cars` (qualidade = percentil de `car_performance` na categoria). Ligado em `career.rs:159` após `insert_teams`. 5+1 testes verdes. Transição de temporada: o estado do carro persiste no DB (nada a fazer) e o horizonte re-rola sozinho (é função de `season`).

### Task 8: Aplicar a manutenção do carro a cada corrida (pipeline)

> **ADIADA para o Chunk 5** (decisão do usuário, jul/2026). Ligar o tick agora evoluiria
> um estado que nada consome até a simulação lê-lo (Chunk 5), e o comportamento vivo só é
> verificável rodando o app. Fazer os dois juntos deixa o tick observável na hora de testar.
> Hook candidato: `race.rs:991 persist_race_result_tx` (por categoria, já com guard de
> idempotência) — mas confirmar onde as corridas de IA (calendário 9D) são finalizadas.

**Files:**
- Modify: `src-tauri/src/evolution/pipeline.rs` (ou o ponto onde a corrida é finalizada por time)
- Test: no mesmo arquivo

- [x] Escrever teste: após uma rodada, cada time teve `advance_race` aplicado com as decisões do brain e o `team_car` foi persistido; guard de idempotência (não avança duas vezes na mesma corrida).
- [x] Rodar o teste e confirmar falha.
- [x] Ligar brain (Chunk 3) + wear (Chunk 2) + persistência (Task 6) no tick pós-corrida.
- [x] Rodar novamente o teste focado e confirmar sucesso.

> **FEITO:** `car_maintenance::run_car_maintenance_for_round` (carrega/decide/aplica/persiste por time, janela cortada pelo horizonte) ligado no **passo 8 do `persist_race_result_tx`** (race.rs) — dentro da tx guardada por idempotência, uma vez por rodada de cada categoria. As próximas pistas vêm do calendário da categoria após a rodada atual. Teste verde.

## Chunk 5: Integração na simulação

### Task 9: Magnitude → car_performance e shape → vetor PHA

**Files:**
- Modify: `src-tauri/src/simulation/math.rs` (`category_car_performance` a partir da magnitude)
- Modify: `src-tauri/src/simulation/race.rs` (usar o vetor PHA no lugar do perfil)
- Modify: `src-tauri/src/simulation/car_build.rs` (aposentar `CarBuildProfile`; expor casamento por vetor contínuo)
- Test: `src-tauri/src/simulation/race.rs`, `src-tauri/src/simulation/car_build.rs`

- [x] Escrever testes: magnitude maior → `car_performance` maior; o shape entra como `CarAttributeWeights` contínuo; `car_weight_scale` por categoria inalterado; rookie continua spec.
- [x] Rodar os testes e confirmar falha.
- [x] Substituir o enum `CarBuildProfile`/`weights_for_profile` pelo vetor emergente das peças; remover o caminho do perfil discreto.
- [x] Rodar novamente os testes focados e confirmar sucesso.

> **FEITO (rewire completo, verde).** `car/sim_bridge.rs` (`car_performance_from`, `car_shape_weights`); `car_build.rs` reescrito só com o caminho de shape contínuo (`track_delta_from_shape`/`effective_car_performance_from_shape` + peakiness/dominância). `CarBuildProfile` APOSENTADO: `Team` ganhou `car: Option<Car>` e perdeu `car_build_profile`; `teams.rs` (parser/insert/update) para de ler/gravar a coluna e `attach_cars` anexa o Car nos loaders; `context.rs` usa `team.car` (fallback ao escalar); `preseason.rs` não escolhe mais perfil; `car_build_strategy.rs` DELETADO; DTOs (career_types/career) sem o campo; testes de perfil de race/qualifying aposentados. ⚠️ conflito de edição paralela resolvido (usuário parou o editor que restaurava o campo). 0 erros, 0 warnings novos, simulation:: 137 verdes.

### Task 10: Regra de dominância (clamp por peakiness da pista)

**Files:**
- Modify: `src-tauri/src/simulation/car_build.rs` (`track_delta` com teto função da peakiness)
- Test: `src-tauri/src/simulation/car_build.rs`, `src-tauri/src/simulation/race.rs`

- [x] Escrever testes: em pista equilibrada, nível 8 **sempre** bate nível 6 (bônus de shape ≈ 0); em pista mono-tema (ex.: 90/5/5), um nível-6 alinhado **pode** bater um nível-8 generalista; o clamp em pista normal fica < metade do gap de nível adjacente.
- [x] Rodar os testes e confirmar falha.
- [x] Trocar o clamp fixo ±6 por `clamp = f(peakiness)`; peakiness = distância dos pesos da pista ao balanceado.
- [x] Rodar novamente os testes focados e confirmar sucesso.

> **Feito:** `track_peakiness` + `DOMINANCE_MAX_DELTA=8`; `track_delta` clampa por peakiness. 10 testes verdes (dominância em pista equilibrada + virada em ponto único). ⚠️ mexe no balanço vivo (context.rs:202) — verificar no app.

### Task 11: Reconciliar `car_weight_scale` da production

**Files:**
- Modify: `src-tauri/src/simulation/math.rs`
- Test: `src-tauri/src/simulation/math.rs`

- [x] Decidir e testar o novo peso da production (hoje 1.40, alto demais pra categoria de baixo prestígio/escada 3); ajustar coerente com o teto 4.
- [x] Rodar os testes focados e confirmar sucesso.

> **FEITO:** `production_challenger` `car_weight_scale` **1.40 → 0.72** (entre amador 0.65 e bmw 0.80). Teste de math atualizado. Calibrável no chunk 8.

## Chunk 6: Integração financeira (depreciação real)

### Task 12: Custo de peças na fatura de manutenção

**Files:**
- Modify: `src-tauri/src/finance/` (o módulo da fatura/manutenção)
- Modify: `src-tauri/src/commands/` (report da fatura, se aplicável)
- Test: nos módulos afetados

- [x] Escrever testes: as decisões de carro (trocar/esticar) viram despesa real por corrida na fatura do time; time sem caixa não consegue trocar (o brain degrada/estica); consistência com o dossiê financeiro real.
- [x] Rodar os testes e confirmar falha.
- [x] Ligar o custo de peças à fatura de manutenção (hoje informativa → depreciação real).
- [x] Rodar novamente os testes focados e confirmar sucesso.

> **FEITO (verde).** `car_maintenance::maintain_team_car` (por time: decide→aplica→persiste, devolve o custo) chamado no loop de finanças do `apply_race_result_to_database`; o custo entra em `technical_investment_cost` (substituindo a proxy antiga por `car_performance`). `category_cost_scale` RE-ANCORADO na economia de cada categoria (`operating_cost_midpoint × ~0,00065`, 1ª passada — mantém as proporções relativas; chunk 8 refina). **+ REGRA DO SOBREUSO** (pedido do usuário): repor peça esticada (`spent`) cai um nível (`wear::replacement_level`); `replace_cost` cobra o nível abaixo; o passe de upgrade ignora peças spent. commands::race 27 + car 38 + cashflow 14 verdes.

## Chunk 7: UI My Team

### Task 13: Mostrar Nível do Carro e remover o seletor de perfil

**Files:**
- Modify: componente da aba My Team (localizar via grep `car_build_profile`/perfil no `src/`)
- Modify: comando que serve os dados da My Team (expor `car_level`)
- Test: `src/**/*.test.jsx` do componente

- [x] Escrever teste de UI: a aba mostra "Nível do Carro 1–10" do time do jogador e **não** exibe balanced/aceleração/handling nem seletor de perfil.
- [x] Rodar `npm run test:ui -- <arquivo>` e confirmar falha.
- [x] Remover o seletor de perfil; exibir o Nível vindo do backend.
- [x] Rodar novamente o teste focado e confirmar sucesso.

> **FEITO (verde).** Backend: `car_level: u8` adicionado a `TeamSummary` E `TeamStanding` (populado de `team.car.display_level()`). Frontend `MyTeamTab.jsx`: coluna "Nível do carro" usa o `car_level` REAL; coluna "Tipo do carro" REMOVIDA; dossiê técnico troca as linhas de shape (Foco do projeto/Equilíbrio) por "Pacote do carro" (Nível X/10) + "Desempenho na pista" + "Confiabilidade"; funções mortas de perfil (`buildMeta`/`BUILD_META`/`profile*`) deletadas. MyTeamTab 23/23 verde, `npm run build` ✓, backend 0 erros/warnings.

## Chunk 8: Verificação integrada e calibração

### Task 14: Monte Carlo — spread, tetos e dominância

**Files:**
- Create/Modify: harness de MC existente (grep `monte_carlo`/`mc_` no `src-tauri/`)
- Test: cenário de MC dedicado

- [x] Escrever cenário MC: sobre N temporadas, os níveis médios de carro por categoria convergem perto dos tetos (amador ~2, bmw ~3, production ~4, gt4 ~6, gt3 ~7, lmp2 ~8); há spread intra-categoria por orçamento; carro nível alto quase nunca perde pro baixo, exceto em pistas mono-tema.
- [x] Rodar o MC e ajustar as constantes assumidas (base por categoria, 35% do orçamento, 40% do esticar, +35% da parede) até bater as metas.
- [x] Registrar os números finais calibrados no design doc (§12).

> **FEITO.** `sim_stats.rs` ganhou coletor + relatório "NÍVEL DO CARRO por categoria" (média/min/max vs teto). MC (2×8) validou: **spread emerge do orçamento**, **tetos respeitados** (max ≤ teto após o fix), **economia saudável** (caixa médio 17,6M, ~8 colapsos em milhares de team-temporadas — o custo real do carro NÃO fale ninguém), **rookie = spec exato (1.0)**. Resultados: rookie 1.0, amador 1.9/2, bmw 3.0/3, gt4 5.7/6, gt3 6.6/7, endurance 6.5/8, production 2.7/4 (as 2 "difíceis" ficam ~1.3 abaixo do teto — aceitável/realista, spread saudável). **BUG CORRIGIDO:** time rebaixado carregava carro acima do teto da nova categoria pra sempre → clamp ao teto no `maintain_team_car` E re-ancoragem instantânea em `apply_team_category_change` (promoção/rebaixamento). `category_cost_scale` da 1ª passada (Chunk 6) já se mostrou bem calibrado — não precisou mexer.

### Task 15: Regressão e compilação

**Files:**
- Verify only

- [x] `cargo test --lib` (novos módulos + regressões da simulação passam): car 38, car_maintenance 12, simulation 137, promotion 48, commands::race 27, cashflow 14, teams 21, math 8 — verdes.
- [x] `cargo check` (sucesso, sem dead_code novo).
- [ ] `npm run test:ui` e `npm run build` (My Team) — pertence ao Chunk 7 (UI), ainda não feito.
- [x] Revisar o diff e confirmar que `CarBuildProfile` foi aposentado sem sobras órfãs.

---

## Fora de escopo (fase futura)

- **Export pro iRacing** (inversão carro→dificuldade da IA): `driverSkill_IA = base + track + adaptativo + carro_IA − carro_jogador`. É pra isso que a margem até 125 foi reservada no redesign de skill. Plano próprio quando chegar a hora.
- Risco de falha mecânica por desgaste extremo (hoje o degradar só derruba nível).
- Horizonte de planejamento como atributo de staff (hoje é traço direto por time).
