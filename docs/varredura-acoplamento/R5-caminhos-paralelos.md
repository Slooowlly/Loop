# R5 — Quatro caminhos de código vivos apenas pelos próprios testes

**Área:** Rust · **Risco:** baixo para investigar, médio para agir · **Conflita com:** nada

---

## Situação em 11/08/2026 — RESOLVIDO

Os quatro casos foram reconferidos por busca de definição (`fn X`) e de chamador no crate
inteiro.

| Função legada | Situação hoje |
|---|---|
| `simulation::engine::run_full_race` | **Sumiu.** Não existe `fn run_full_race` no crate. Produção e testes usam `run_full_race_with_breakdowns`, inclusive a arena de calibração, que documenta no cabeçalho por que mede o caminho completo. |
| `simulation::race::motor::simulate_race` | **Sumiu.** Só existe `simulate_race_with_breakdowns`. |
| `simulation::incidents::segmento::process_segment_incidents` | **Sumiu.** Só existe `process_segment_incidents_cfg`. |
| `market::pipeline::run_market` | **Existe, e é deliberado.** Tem `#[cfg_attr(not(test), allow(dead_code))]` e doc explicando o contrato: resolve o mercado inteiro de uma vez, a pré-temporada interativa NÃO usa isto (usa `run_market_prepasses` + `run_market_movements` + a Janela ao vivo), e ele fica como cobertura do wiring completo. Atende exatamente ao critério de "harness/teste com contrato claro e `cfg` pontual". **Mantido.** |

A "falsa cobertura" que o briefing denunciava acabou junto: `test_full_race_integration` não
existe mais medindo um atalho, porque o atalho não existe.

### Item 6 — outros pares `fn X` / `fn X_variante` no crate

A busca foi refeita. Três candidatos, nenhum removido nesta passada:

- **`commands/race/financas.rs::calculate_team_round_finance_context_com`** — zero chamadores
  no crate, inclusive testes. O doc diz "só o harness de calibração passa algo diferente do
  `Default`", mas o harness chama `..._modelo` direto. É o padrão do R5, e é removível.
  **Deixado de fora de propósito:** o comparador de despesa está sendo mexido em outra
  frente, e `EtapaFisica::de_referencia` (hoje viva só por ele) cairia junto.
- **`market/car_maintenance/{decide_maintenance, decide_maintenance_with_ceiling, apply_plan}`**
  — dead code declarado, área de previsão de quebra. Fora do escopo por instrução.
- **`calendar/full_season/parcial.rs`** (subárvore inteira sem chamador, incluindo
  `generate_calendar_for_category_with_count`) — arquivo em edição por outra frente.

## O que foi encontrado

Quatro funções cujo único chamador, no crate inteiro, é a suíte de testes. Produção
entra por outra porta.

| Função legada | Chamadores reais | Produção usa |
|---|---|---|
| `simulation::engine::run_full_race` | `simulation/engine.rs:116,134` — ambos dentro de `#[test]` | `run_full_race_with_breakdowns` ([`commands/race.rs:47`](../../src-tauri/src/commands/race.rs)) |
| `simulation::race::motor::simulate_race` | `simulation/race/tests/mod.rs:97,120,144,165` | `simulate_race_with_breakdowns` |
| `simulation::incidents::segmento::process_segment_incidents` | `simulation/incidents/tests/mod.rs:72,104,124` | `process_segment_incidents_cfg` |
| `market::pipeline::run_market` | `market/pipeline/tests/mod.rs:64,700,718,827` | `market::preseason::initialize_preseason` (a confirmar) |

Nos três primeiros o padrão é claro: a variante `_with_breakdowns` / `_cfg` foi
adicionada, produção migrou, a original virou wrapper e os testes ficaram nela.
`process_segment_incidents_cfg` inclusive documenta a relação:
`simulation/incidents/segmento.rs:53` — "Igual a [`process_segment_incidents`], mas
com `catalog_mechanical`".

O caso do `run_market` é diferente e menos certo — preciso que seja confirmado.

## Por que importa

O efeito é o oposto do que a suíte sugere: `test_full_race_integration` tem nome de
teste de integração e exercita um caminho que o app nunca percorre. Regressão na
interação entre corrida e quebras de carro — justamente o que a variante nova
adiciona — passa despercebida.

Não é código morto (os testes o mantêm compilando e verde). É pior: é **falsa
cobertura**.

## Armadilhas conhecidas

1. **Não é só apagar.** Se `run_full_race` é um wrapper que passa
   `IncidentCatalog::empty()`, apontar os testes para a variante nova pode mudar o
   resultado esperado — e aí é preciso decidir se o valor novo está certo.
2. **Determinismo por seed.** Os testes usam `StdRng::seed_from_u64(41/42)`. Trocar
   a função chamada muda a sequência de consumo do RNG e quebra asserções que
   dependem de resultado exato. Espere ter que recalibrar.
3. **`run_market` pode não ser legado.** Pode ser o motor que `initialize_preseason`
   chama por dentro, e minha varredura ter perdido a indireção. **Confirme antes de
   tratar como legado.**
4. **`cargo test` exige `npm run build` antes.** `tauri::generate_context!` embute
   `dist/` em tempo de compilação.

## O que eu quero da segunda análise

1. **Confirme os quatro casos, um a um.** Especialmente `run_market`: rastreie
   `market::preseason::initialize_preseason` por dentro e diga se ele chama
   `run_market` ou reimplementa. Se chamar, risque este caso da lista.
2. **Qual é a diferença real entre cada par?** Para os três primeiros, mostre o diff
   de comportamento entre a variante legada e a nova. A legada é um wrapper com
   parâmetro fixo, ou tem lógica própria que divergiu?
3. **Os testes perdem cobertura ao migrar?** Se `run_full_race` testa o caminho "sem
   catálogo de incidentes" e esse caminho ainda é alcançável em produção por outra
   via, a migração precisa preservar o caso. Verifique.
4. **Plano de migração por caso**, com atenção ao ponto 2 das armadilhas: quais
   asserções vão precisar de recalibração de seed, e como distinguir "recalibrei
   porque o RNG mudou" de "recalibrei porque escondi uma regressão". Este é o ponto
   mais delicado do briefing.
5. **Depois de migrar, a legada some ou fica?** Se ficar como API pública sem
   chamador, ela volta a apodrecer. Recomende.
6. **Existem outros pares assim que eu não peguei?** Minha varredura foi por
   contagem de referências e tem falso positivo/negativo conhecido. Procure o padrão
   `fn X` + `fn X_with_algo` / `fn X_cfg` no crate inteiro.

Não aplique nada ainda — quero ler a análise antes.
