# Varredura de bugs — julho/2026

**Para quem vai investigar:** este documento lista 6 achados de uma varredura feita
sobre o diff de trabalho da branch `main-menu-redesign` (60 arquivos, ~1.900 linhas
novas, ainda não commitado no momento da varredura). Cada achado foi levantado por
LEITURA de código, e só o primeiro foi reproduzido empiricamente.

**Sua tarefa:** confirmar ou refutar cada um, com evidência. Não assuma que estão
certos — três deles são hipóteses baseadas em leitura estática e podem cair diante de
um invariante que a varredura não enxergou. Para cada achado há uma seção
**"O que refuta"**: se você encontrar aquilo, o achado morre e deve ser marcado como
falso positivo, não "amenizado".

Prioridade de investigação: **#1 e #2** primeiro (são os que têm consequência de
verdade), depois #3–#6.

## Contexto mínimo do projeto

Loop é um jogo de carreira no automobilismo em Tauri v2 (Rust) + React 18, SQLite
local. Código, comentários e UI em português. Três suítes de teste independentes:

```bash
npm run test:ui            # vitest (src/**/*.test.{js,jsx})
npm run test:structure     # node --test scripts/tests/*.test.mjs
cargo test --manifest-path src-tauri/Cargo.toml
```

**Atenção:** `cargo test` e `cargo build` exigem `npm run build` antes —
`tauri::generate_context!` embute os assets de `dist/` em tempo de compilação.

Estado das suítes no momento da varredura: vitest 465/465 ✅, estrutural 28/28 ✅,
cargo **1886 ✅ / 1 ❌** (o ❌ é o achado #1).

---

## #1 — Teste Rust intermitente (REPRODUZIDO, ~25% de falha)

**Arquivos:** [`src-tauri/src/commands/career/vacancies.rs`](../src-tauri/src/commands/career/vacancies.rs),
[`src-tauri/src/commands/career/tests/mod.rs`](../src-tauri/src/commands/career/tests/mod.rs)

**Confiança da varredura:** alta na reprodução, média no diagnóstico.

### O sintoma

`commands::career::tests::test_accept_proposal_to_full_team_replaces_incumbent_instead_of_creating_third_driver`
falha de forma intermitente.

Reprodução (8 execuções isoladas → 2 falhas, 6 passes):

```bash
cd src-tauri && for i in 1 2 3 4 5 6 7 8; do cargo test --lib test_accept_proposal_to_full_team_replaces_incumbent 2>/dev/null | grep "test result"; done
```

A asserção que quebra é a última do teste (~linha 3711):

> `incumbent displaced from the accepted role should no longer hold an active regular contract for the target team`

### O diagnóstico proposto

Em `normalize_regular_contracts_for_team` (vacancies.rs):

- Linhas **27–41**: os contratos regulares ativos do time são ordenados com o jogador
  primeiro, depois por `temporada_inicio` desc, `created_at` desc, `id` desc.
- Linhas **49–83**: o laço distribui os slots **pelo campo `papel` primeiro**. O
  primeiro contrato com `TeamRole::Numero1` fica com `keep_n1`; o primeiro com
  `Numero2` fica com `keep_n2`. Só quando o slot do próprio papel já está ocupado é
  que o contrato tenta o outro slot, e o terceiro excedente é rescindido (linha 70).

A hipótese: a prioridade do jogador na ordenação **só desempata dentro do mesmo
papel**. Com 3 contratos (jogador + 2 incumbents), quem é rescindido depende do
`papel` que a proposta aceita gravou no contrato do jogador:

- jogador entra como `Numero1` → o incumbent N1 perde a vaga (o teste passa);
- jogador entra como `Numero2` → o incumbent N2 perde a vaga (o teste falha, porque
  ele guardou `target_team.piloto_1_id` em `displaced_driver_id`, ~linha 3665).

Se essa hipótese estiver certa, **é o teste que está superespecificado**, não a
produção: os invariantes reais (`target_contracts.len() == 2` e o jogador presente no
lineup) continuam valendo nas duas ramificações, e ambos são asseverados
separadamente logo antes.

### O que confirmar

1. Instrumente o teste (ou rode-o sob `--nocapture` com um `dbg!`) e imprima o
   `papel` do contrato do jogador após `respond_to_proposal_in_base_dir`. Correlacione
   com passe/falha ao longo de ~20 execuções.
2. Descubra **de onde vem** esse `papel`: rastreie `seed_player_proposal` e
   `respond_to_proposal_in_base_dir` até o ponto que decide `TeamRole`. Se houver
   `rand::thread_rng()` no caminho, a nondeterminância está explicada.
3. Verifique se `create_test_career_dir("accept_proposal_replaces_full_team_incumbent")`
   isola de fato o diretório, e se `advance_season_in_base_dir` (chamado no início do
   teste) usa RNG não semeado — ele roda o mercado/evolução inteiros e é a outra fonte
   plausível de variação.

### O que refuta

- Se o `papel` do jogador for **constante** entre as execuções que passam e as que
  falham, o diagnóstico está errado e a causa é outra (procure então interferência
  entre testes: locale global do `rust-i18n`, diretório temporário compartilhado,
  ou estado global em `AppConfig`).
- Se existir, em algum ponto do fluxo de aceite, uma regra explícita de que o
  incumbent **N1** é sempre o deslocado, então a produção é que está violando a regra
  e a gravidade sobe: passa a ser bug de produção, não de teste.

### Se confirmado

A correção provável é no teste: trocar a asserção por "algum dos dois incumbents
perdeu o contrato regular ativo neste time", ou fixar o `papel` da proposta no seed
para tornar o cenário determinístico. **Não** mexa em `normalize_regular_contracts_for_team`
sem antes provar que a produção viola um invariante — ela parece correta.

---

## #2 — A fatura do fim de semana ancora numa linha de histórico não verificada

**Arquivo:** [`src-tauri/src/commands/race/manutencao.rs:83-110`](../src-tauri/src/commands/race/manutencao.rs)

**Confiança da varredura:** média. É um risco estrutural claro; falta provar que ele
se materializa em algum caminho real.

### O código

`compute_maintenance_breakdown` monta as linhas da fatura de operação e depois as
normaliza para que somem exatamente o valor que saiu do caixa:

```rust
let lines = crate::finance::operacao::compute_operation_lines(&inputs);
let bruto: f64 = lines.iter().map(|l| l.cost).sum();                    // linha 100

let debitado = team_queries::get_team_finance_history_recent(conn, &team.id, 1)  // 104
    .ok()
    .and_then(|h| h.first().map(|e| e.event_operations_cost))
    .filter(|v| *v > 0.0)
    .unwrap_or(bruto);
let ajuste = if bruto > 0.0 { debitado / bruto } else { 0.0 };          // linha 109
```

A query em [`db/queries/teams/financas.rs:136-152`](../src-tauri/src/db/queries/teams/financas.rs) é:

```sql
SELECT season_number, round, category, ... , event_operations_cost, ...
FROM team_finance_history
WHERE team_id = ?1
ORDER BY season_number DESC, round DESC
LIMIT ?2
```

### O problema proposto

Não existe nenhuma checagem de que a linha devolvida é **desta** rodada. A função
confia inteiramente em "a mais recente é a certa". Se ela não for:

- o `ajuste` reescala **a fatura inteira** pelo custo de operação de outra rodada;
- o resultado é silencioso: o tooltip mostra números plausíveis, com um total que não
  corresponde ao dinheiro efetivamente debitado — que é exatamente o problema que a
  reescrita queria resolver (o docstring da função diz "para não depender de os dois
  cálculos coincidirem").

A linha do histórico **tem** `season_number`, `round` e `category`, e os dois call
sites têm `race_entry` em mãos, então o casamento seria barato.

### O que confirmar

Os dois call sites são:

- [`src-tauri/src/commands/race.rs:313-330`](../src-tauri/src/commands/race.rs) (corrida simulada)
- [`src-tauri/src/commands/race/importacao.rs:436-452`](../src-tauri/src/commands/race/importacao.rs) (corrida importada do iRacing)

Para cada um, responda:

1. **Ordem de escrita.** A linha de `team_finance_history` desta rodada já foi
   gravada quando `compute_maintenance_breakdown` roda? Rastreie
   `apply_race_result_to_database` → `apply_round_cashflow` → o INSERT em
   `team_finance_history`, e confirme que ele acontece **antes**. (A varredura
   acredita que sim nos dois caminhos, mas não provou.)
2. **Unicidade por (season, round).** Um time pode ter mais de uma linha na mesma
   `(season_number, round)` — por exemplo correndo em duas categorias na mesma
   rodada, ou uma equipe do jogador com carro em categoria especial/endurance? Se
   sim, `ORDER BY season_number DESC, round DESC LIMIT 1` escolhe uma delas
   arbitrariamente (não há tie-break) e pode pegar a categoria errada.
3. **Fronteira de temporada.** Existe alguma escrita de `team_finance_history` com
   `round = 0` ou de pré-temporada que possa ficar "mais recente" que a corrida?
4. **Save antigo / fluxo fora do normal.** O `.unwrap_or(bruto)` cobre o caso de
   histórico ausente. Mas o caso perigoso não é ausência — é presença de uma linha
   **errada**, que passa pelo `.filter(|v| *v > 0.0)` sem levantar suspeita.

Um teste que materializaria o bug: gravar duas linhas de histórico para o mesmo time
(rodada 4 e rodada 5, com `event_operations_cost` bem diferentes), chamar
`compute_maintenance_breakdown` no contexto da rodada 4, e verificar se o total sai
ancorado na 5.

### O que refuta

- Se existir uma garantia estrutural de que a última linha é sempre a da rodada
  corrente **em todos os caminhos** (incluindo import do iRacing, corrida de
  categoria especial, e reabertura de tela antiga), o achado vira "código frágil mas
  correto" — e aí a recomendação é só adicionar a checagem defensiva, sem urgência.
- Note que a reabertura de tela antiga (`save_race_screen` / `openRaceScreen`) lê a
  fatura **persistida**, não recalcula — então esse caminho provavelmente está a
  salvo. Confirme.

---

## #3 — `dismissResult` não limpa `lastRaceMaintenance`

**Arquivo:** [`src/stores/career/raceSlice.js:121-130`](../src/stores/career/raceSlice.js)

**Confiança da varredura:** alta no fato, baixa no impacto.

### O fato

```js
dismissResult: async () => {
  const { careerId, loadCareer } = get();
  set({
    showResult: false,
    iracingRepair: null,
    lastRaceEvaluation: null,
    lastRaceTelemetry: null,
    lastRaceRepercussion: null,   // adicionado neste diff
    resultIsFresh: false,
  });
  ...
```

`lastRaceMaintenance` é o **único** campo de "última corrida" que não é zerado. Os
três irmãos (`lastRaceEvaluation`, `lastRaceTelemetry`, `lastRaceRepercussion`) são.

### O que confirmar

1. Existe algum caminho em que a tela de resultado é montada com
   `lastRaceMaintenance` de uma corrida **anterior**? Os três setters em
   `raceSlice.js` (simulada ~linha 31, importada ~linha 66, reabertura ~linha 99)
   sempre escrevem `... ?? null`, o que significa que uma abertura nova sempre
   sobrescreve. Se for assim em 100% dos caminhos, o impacto real é zero e isto é
   só inconsistência de código.
2. Verifique `state.js` (`initialState`) e `clearCareer` — se `clearCareer` reseta
   tudo, o vazamento não atravessa troca de carreira.
3. Caso de borda a testar: abrir resultado de corrida A (com fatura), fechar, e abrir
   a tela de uma corrida B **salva antes do campo existir** (`screen.maintenance`
   ausente → `null`). Se o `?? null` cobre, tudo bem.

### O que refuta

Se todo caminho que exibe `RaceResultViewV2` passa obrigatoriamente por um dos
setters, isto é cosmético. Documente como tal e não gaste tempo.

---

## #4 — Mudança silenciosa de semântica em "batida" (`is_crash`)

**Arquivos:** [`src-tauri/src/race_signals.rs:58-68`](../src-tauri/src/race_signals.rs) (novo),
[`src-tauri/src/narrative/incidentes.rs`](../src-tauri/src/narrative/incidentes.rs) (função removida),
[`src-tauri/src/narrative/beats.rs:213`](../src-tauri/src/narrative/beats.rs),
[`src-tauri/src/narrative/tests/mod.rs`](../src-tauri/src/narrative/tests/mod.rs)

**Confiança da varredura:** alta no fato, média em ser um bug (pode ser decisão
consciente mal documentada).

### O fato

A função removida de `narrative/incidentes.rs` era:

```rust
pub(crate) fn is_crash(inc: &IncidentResult) -> bool {
    match inc.incident_type {
        IncidentType::Collision => true,
        IncidentType::DriverError => inc.severity != IncidentSeverity::Minor,  // <-- olha aqui
        IncidentType::Mechanical => false,
    }
}
```

A substituta, em `race_signals.rs`:

```rust
pub fn dnf_kind(inc: Option<&IncidentResult>, mech_break: bool, reason: Option<&str>) -> DnfKind {
    if let Some(i) = inc {
        return match i.incident_type {
            IncidentType::Collision => DnfKind::Contato,
            IncidentType::DriverError => DnfKind::Erro,     // severidade NÃO entra mais
            IncidentType::Mechanical => DnfKind::Mecanico,
        };
    }
    ...
}

impl DnfKind {
    pub fn is_crash(self) -> bool { matches!(self, DnfKind::Contato | DnfKind::Erro) }
}
```

A `severity` sumiu da regra: `DriverError` + `Minor` era "não é batida" e agora é.
O caso de teste que cobria isso (`erro_leve`, com
`IncidentType::DriverError` + `IncidentSeverity::Minor` + `is_dnf: false`) foi
**apagado** do teste `pane_mecanica_nao_conta_como_batida`, em vez de preservado.

### O que confirmar

1. O único consumidor de `is_crash` hoje é `beats.rs:213`, num contexto de **DNF**
   (`+10.0` no peso do beat de abandono). Quantifique: quantos DNFs são gerados com
   `IncidentType::DriverError` **e** `IncidentSeverity::Minor`? Procure no motor de
   incidentes (`simulation/incidents.rs`) se essa combinação sequer é possível para
   um incidente que causa DNF. Se `is_dnf` nunca coexiste com `Minor`, a mudança é
   inócua na prática.
2. Existe outro consumidor de `DnfKind::is_crash()` fora de `beats.rs`? Confirme com
   um grep no crate inteiro.
3. Confira se a remoção do caso `erro_leve` do teste está justificada em algum
   comentário, briefing ou mensagem de commit. Se não estiver, é regra perdida.

### O que refuta

Se `IncidentSeverity::Minor` for estruturalmente incompatível com `is_dnf: true`, o
achado é teórico e deve ser fechado como tal (com uma nota, no máximo).

---

## #5 — Duas recalibrações embutidas na unificação de limiares

**Arquivos:** [`src-tauri/src/event_interest/calculator.rs:210-230`](../src-tauri/src/event_interest/calculator.rs),
[`src-tauri/src/commands/ai_news/tese.rs:67`](../src-tauri/src/commands/ai_news/tese.rs),
[`src-tauri/src/race_signals.rs:24`](../src-tauri/src/race_signals.rs)

**Confiança da varredura:** alta no fato, indefinida na intenção. **Isto pode ser
100% intencional** — o objetivo aqui é obrigar uma decisão consciente, não apontar
erro.

### 5a — `positional_bonus` ficou mais fraco no miolo

Antes (faixas):

```rust
let positional_bonus = if positions_gained >= 5 { 4.0 }
    else if positions_gained >= 2 { 2.0 }
    else if positions_gained <= -5 { -3.0 }
    else { 0.0 };
```

Depois (contínuo, linha 230):

```rust
let positional_bonus = (positions_gained as f32 * 0.4).clamp(-3.0, 4.0);
```

O comentário no código afirma que os limites "preservam o teto/piso das faixas
antigas". Isso é verdade **nos extremos**, mas o joelho da curva se mexeu:

| posições ganhas | bônus antigo | bônus novo |
|---|---|---|
| +2 | 2.0 | 0.8 |
| +5 | **4.0** | **2.0** |
| +10 | 4.0 | 4.0 |
| −5 | −3.0 | −2.0 |
| −8 | −3.0 | −3.0 |

Uma remontada de 5 posições vale metade do que valia. O teto antigo só é reatingido
com 10 posições.

**Confirmar:** existe teste ou documento de calibração que fixe o valor esperado de
repercussão para um cenário de referência? Se sim, ele ainda passa? Se não, vale
criar. Verifique também se o `final_score` alimenta algo com faixas fixas a jusante
(`news_importance_bias`, `headline_strength`) onde essa queda mude o tier resultante.

### 5b — o limiar de "remontada" do debrief caiu de 5 para 4

`ai_news/tese.rs:67` usava `s.positions_gained >= 5` e agora chama
`race_signals::remontada(...)`, que é `>= REMONTADA_MIN` com `REMONTADA_MIN = 4`.

O docstring de `race_signals.rs` reconhece explicitamente que existiam quatro
limiares (`>0`, `4`, `5`, `8`) e que a unificação escolheu um. Ou seja: a mudança foi
deliberada. O que falta é saber se o impacto no **debrief do jogador** foi avaliado —
a tese "remontada" agora dispara com 4 posições ganhas, onde antes exigia 5, e ela
compete na ordem de eleição com "colapso" e as teses de over/underperformance.

**Confirmar:** rode os testes de `ai_news` e veja se algum cenário de referência mudou
de tese. Procure também o beat de "Recuperação" em `beats.rs:131`, que ANTES usava
`> 0` (qualquer ganho) e agora usa `>= 4` — essa é a mudança na direção oposta, e o
comentário no código a justifica ("um ganho de 1 já passava do limiar, gastando uma
vaga do boletim com nada"). Confirme que as duas mudanças foram pesadas juntas.

### O que refuta

Um briefing, comentário ou nota de design que registre a nova calibração como
escolhida. Se existir, feche os dois itens.

---

## #6 — O overlay descarta a estimativa de voltas do próprio iRacing em prova por tempo

**Arquivo:** [`src-tauri/src/commands/overlay/torre.rs:625-655`](../src-tauri/src/commands/overlay/torre.rs)

**Confiança da varredura:** média. Depende de um fato sobre o SDK do iRacing que a
varredura não pôde verificar (é preciso o sim rodando).

### O código

```rust
let timed = tele.session_time_total > 0.0 && tele.session_time_total < 86_400.0;   // 625

// melhor volta de TODO o grid
let ref_lap = tele.cars.iter().map(|c| c.best_lap_time)
    .filter(|t| *t > 0.0).fold(f64::INFINITY, f64::min);                          // 632

let total_laps = if !timed
    && tele.session_laps_remain_ex > 0
    && tele.session_laps_remain_ex < 10_000 {
    max_completed + tele.session_laps_remain_ex                                    // 647
} else if timed && kind == "R" && ref_lap.is_finite() {
    max_completed + (tele.session_time_remain.max(0.0) / ref_lap).ceil() as i32    // 649
} else {
    0
};
```

### O problema proposto

Duas coisas:

1. **O ramo do `session_laps_remain_ex` passou a ser gated por `!timed`.** O
   comentário que o diff REMOVEU dizia literalmente que aquele valor *"vale em corrida
   por tempo"*. Ou seja: a versão anterior usava o `SessionLapsRemainEx` justamente no
   caso que agora o exclui. Se o dado do sim era bom ali, trocá-lo por estimativa
   caseira é regressão.
2. **O ritmo de referência é a volta mais RÁPIDA de todo o campo.** O ritmo médio de
   corrida é sempre mais lento que a melhor volta (tráfego, combustível, pneu,
   bandeiras). Dividir o tempo restante pela melhor volta **subestima
   sistematicamente** quantas voltas ainda cabem. O `.ceil()` compensa menos de uma
   volta; o viés não desaparece.

Efeito visível: o `/total` no header da torre fica baixo demais no início da prova e
vai subindo conforme o tempo passa — o total "cresce" durante a corrida, o que é
justamente o que o `total_laps.max(lead_lap)` logo abaixo tenta remendar.

### O que confirmar

1. **O fato sobre o SDK:** em sessão de corrida por TEMPO, o iRacing preenche
   `SessionLapsRemainEx` com uma estimativa útil, ou com o sentinela de "ilimitado"
   (32767)? Verifique em `iracing_sdk/` como o campo é lido, procure documentação do
   irsdk, e se possível teste com o sim rodando. **Este é o ponto que decide o
   achado.** Se o campo vem sentinelado em prova por tempo, o gate `!timed` está certo
   e o item 1 cai.
2. **O viés do ritmo:** compare `ref_lap` (melhor volta do campo) com uma média das
   voltas do líder num replay real. Quantifique o erro em voltas para uma prova de 30
   e de 60 minutos.
3. Verifique o histórico do arquivo (`git log -p src-tauri/src/commands/overlay/torre.rs`)
   para achar por que o comentário "vale em corrida por tempo" foi escrito
   originalmente — pode haver um caso concreto atrás dele.

### O que refuta

Confirmação de que `SessionLapsRemainEx` é inútil (sentinelado ou errado) em prova
por tempo. Nesse caso o item 1 morre e sobra só o item 2, que vira sugestão de
afinação (usar mediana das últimas voltas do líder em vez do melhor volta absoluta),
não bug.

---

## O que a varredura NÃO encontrou problema

Registrado para você não regastar tempo — mas confira por amostragem se discordar:

- **Parsers de sessão do YAML** (`race_monitor/sessao.rs`): o `strip_prefix("- ")`
  corrige um bug real (o `Sessions:` do iRacing é lista, então a primeira chave de
  cada item vem com traço) e veio com teste de regressão usando indentação real.
- **Gate `in_race_session`** no snapshot de grid: treino livre e quali também chegam a
  `SessionState = Racing`, e o gate novo impede que a primeira sessão do fim de semana
  consuma a captura. Correto e testado.
- **Animador da torre** (`src/overlay/towerAnimation.js`): o `startedAt: null` em vez
  de `0` é a correção certa (um `now` real de 0 colidia com o sentinela). Ciclo de
  vida das chaves e limpeza no `sync` conferidos.
- **`player_repercussion`** em `commands/race/financas.rs`: é calculado ANTES do
  `or_else` que move o `player_realized` — a ordem está certa e o comentário explica.
- **`iracing_process_race_result` chamado de dentro do import**: escreve num arquivo
  JSON de perfil, não no SQLite. Não há risco de conflito de conexão com o `db` aberto
  no escopo externo. (Foi uma suspeita levantada e descartada.)
- **Serialização `elapsed_s`/`duration_s`** (`OverlaySession` tem
  `#[serde(rename_all = "camelCase")]`): vira `elapsedS`/`durationS`, que é
  exatamente o que `towerCanvas.js` e `overlayMockData.js` leem. Confere.
- **`estimateAudience`** trocou de `tier_label` (texto traduzido) para o enum `tier`:
  o único call site (`nextRaceContext.js:97`) foi atualizado, e o enum `InterestTier`
  serializa como `"Baixo"`…`"EventoPrincipal"` sem `rename_all`. Confere.
- **`iracing_restore_yellow_macro`** foi removido do `invoke_handler` e não sobrou
  nenhum chamador no frontend. `old_state_path` continua em uso em outros três pontos
  de `race_control.rs` — não virou dead code.

## Formato da resposta esperada

Para cada achado de #1 a #6, devolva:

- **Veredito:** CONFIRMADO / REFUTADO / PARCIAL.
- **Evidência:** o trecho de código, saída de teste ou comando que sustenta o
  veredito. Não aceite raciocínio sem âncora no repositório.
- **Se CONFIRMADO:** o menor patch que resolve, e qual suíte cobre a regressão.
- **Se REFUTADO:** qual invariante do código a varredura não enxergou.

Se, no caminho, você encontrar um bug que não está nesta lista, adicione — mas
mantenha o mesmo padrão de evidência.
