# Economia de Fama — Fase 3: Público / Imprensa (design travado)

> Status: **DESIGN TRAVADO, não implementado.** Fecha a feature "Nasce um astro"
> (ver `project_stardom_economy`). Fases 1, 2a e 2b já estão no ar.
> Data: 2026-07-19.

## Contexto — o que a Fase 3 encontra pronto

A fama (`midia`, 0–100) já move dinheiro por **patrocínio** (Chunk B) e move o
**mercado** (escada valoriza `fama × necessidade`, Fase 2a). O que falta é o
**público** virar consequência econômica direta e o **astro** virar tinta na revista.

Duas peças-chave já existem e são reusadas inteiras:

1. **`event_interest/calculator.rs`** — `calculate_expected_event_interest(ctx) -> score`.
   O `score` já soma categoria (rookie 18 → endurance 82) + fase + importância da
   rodada + decisão de título + **fama do jogador** (`player_media`). Já vira
   `display_value = score × 450` (número de "público" na tela) **e** a pressão de
   "casa cheia" (via `event_stakes_from_score` em `simulation/pressure.rs`).
2. **`calculate_team_round_finance_context`** (`commands/race.rs:76`) — a fatura da
   rodada por time. Já tem o termo de patrocínio por fama
   (`lineup_public_presence × round_operating_base × FAME_SPONSORSHIP_COEFF`), e o
   call site (`race.rs:~2200`) já calcula `lineup_public_presence` por time.

**O buraco:** o score do evento cria pressão psicológica de casa cheia, mas **nunca
vira dinheiro**. Não há linha de bilheteria. A Fase 3 fecha esse loop: *a mesma
arquibancada que pressiona o piloto agora também enche o caixa.*

## Decisões travadas (AskUserQuestion, 2026-07-19)

- **Divisão do bolo de bilheteria** = **COTA POR FAMA (competitivo)**: cada time leva
  fatia proporcional à fama do seu lineup vs. o grid. Astro num time pobre puxa
  bilheteria mesmo sem vencer; ídolo vale mais se os rivais forem anônimos.
- **Canal de receita** = **LINHA PRÓPRIA "Bilheteria"**, separada do patrocínio no
  dossiê financeiro. Duas texturas distintas (portão volátil/por-evento vs.
  patrocínio suave/de-temporada). Dois canais fama→dinheiro → calibrar pra fama não
  dominar.
- **Timing do portão** = **PRÉ-VENDIDA (interesse ESPERADO)**: dimensionada pelo
  `calculate_expected_event_interest` (público compra antes da corrida). Estável, não
  dobra com o `result_bonus` que já existe na fatura, mais fácil de calibrar.
- **Imprensa (3b)** = **OS DOIS**: nota de astro no rodapé "Do Mundo do Grid" +
  selo de público na edição (spread) da revista.

---

## 3a — Bilheteria (o núcleo econômico)

Nova linha de receita `gate_income` em `calculate_team_round_finance_context`.

### Insumos novos (baratos — já quase tudo no call site)

Pré-computados **1× por categoria/rodada**, antes do loop de times:

```rust
// soma da presença pública de todos os lineups do grid da categoria
let grid_total_presence: f64 = Σ derive_team_public_presence(lineup).raw_score;

// score do evento, PRÉ-VENDIDO (contexto de local, sem depender do resultado)
let event_prestige_score: f64 = calculate_expected_event_interest(&venue_ctx).score;
let n_teams: f64 = /* nº de times do grid da categoria */;
```

`derive_team_public_presence(lineup).raw_score` já é calculado por time no call site
(race.rs:~2200 como `lineup_public_presence`) — só falta somar o grid inteiro uma vez.

### Fórmula (dentro de `calculate_team_round_finance_context`, por time)

```rust
let prestige_factor = (event_prestige_score / 60.0).clamp(0.3, 2.2);   // 60 ≈ evento GT3 médio → 1.0
let gate_pot = round_operating_base * GATE_POT_COEFF * prestige_factor * income_modifier;

let fame_share = if grid_total_presence > 0.0 {
    team_presence / grid_total_presence
} else {
    1.0 / n_teams
};

// piso de público (metade vem pela corrida, dividida igual) + prêmio de estrela
let gate_income = gate_pot * (GATE_FLOOR_WEIGHT / n_teams
                              + (1.0 - GATE_FLOOR_WEIGHT) * fame_share);
```

Entra como nova linha no `TeamRoundFinanceContext` (10ª linha) e no
`insert_team_finance_history` (dossiê My Team, ver `project_my_team_real_numbers`).

### Três propriedades que o design trava

| Propriedade | Como emerge |
|---|---|
| Evento grande = portão grande | `prestige_factor` reusa o MESMO score da pressão de casa cheia. Endurance/final/decisão lotam; rookie de meio, não. Loop fechado: uma multidão, dois efeitos. |
| Astro puxa bilheteria sem vencer | `fame_share` competitivo — ídolo vale mais se os rivais forem anônimos. Fama = ativo econômico. |
| Piso + prêmio | `GATE_FLOOR_WEIGHT` (~0.5): metade da multidão vem pela corrida (split igual, ninguém zera), metade pelas estrelas (split por fama). Estrelato é prêmio sobre um piso. |

### Constantes (tunáveis; calibrar por Monte Carlo)

- `GATE_POT_COEFF ≈ 0.12` (fração de `round_operating_base` que é o bolo do evento médio).
- `GATE_FLOOR_WEIGHT ≈ 0.5`.
- Botão de ambiente `IRACER_GATE_SHARE` no molde do `IRACER_SALARY_SHARE`
  (ver `project_salary_budget_diagnosis`) pra escalar sem re-tunar a fórmula.
- **Alvo de calibração:** bilheteria média por rodada MENOR que o patrocínio — fama já
  ganha por 2 canais, não pode dominar a economia nem substituir mérito/skill.

### Superpotência realizada (visão do user, verbatim)

Um time meio-de-grid contratar um **astro em declínio** (fama alta, skill caindo)
vira jogada economicamente racional: ele lota o portão + capta patrocínio →
financia o carro → aí compra talento de verdade. Fama = investimento, não talento.

### Escopo / wiring

- O loop de finanças (`race.rs:~2212`) roda por time da categoria. Gate aplica ao grid
  todo da categoria. Incentivo da IA a valorizar fama já existe (escada 2a); a
  bilheteria só reforça implicitamente (times com astro ficam mais ricos).
- **A confirmar na implementação:** se o loop roda também para categorias só-IA. Se
  não, a bilheteria vale só a categoria do jogador no MVP (aceitável — mercado 2a já
  cuida do incentivo da IA no resto do mundo).

---

## 3b — Prominência na imprensa (OS DOIS)

### (i) Nota de astro no rodapé "Do Mundo do Grid"

Nova categoria de nota em `commands/world_footer.rs` (`collect_world_notes` /
`WorldNote`, ver `project_world_footer_news`). O **maior nome da categoria atual**
(maior `midia`, tier Estrela+/Ídolo) vira notinha jornalística em 3ª pessoa (a
revista é publicada — SEM "você"). `tag` temática (ex.: BASTIDORES/MERCADO).

- Encaixe na cascata: entra como fonte de fallback junto/antes dos recordes, ou como
  nota própria de topo quando há um Ídolo (`midia > 87`) na categoria.
- Texto determinístico primeiro (voz jornalística), IA depois via `/world-notes`
  (contrato em `docs/world-notes-endpoint.md`, deploy separado — mesmo padrão do
  rodapé atual).
- Gatilho: fama alta E marco (subiu de tier, recorde de fama, "lota onde corre").

### (ii) Selo de público na edição (spread)

A edição da corrida (spread em `NewsMagazineTab.jsx`) ganha um **selo de público /
lotação** ("Casa cheia — ~42 mil") puxado do `event_interest` daquela corrida
(`display_value` / `final_display_value`). Liga visualmente arquibancada → manchete.

- Dado: reusa o `RealizedEventInterest.final_display_value` já persistido pós-corrida
  (ou o `expected` da edição). Precisa de comando/JOIN pela rodada, no molde do
  `player_race_news_id`.
- UI: badge no cabeçalho do spread (CSS escopado `.newsmag`), tom por tier
  (Evento Principal = dourado). Toca layout do spread — trabalho de CSS.
- ⚠️ Ambiente: o user roda a própria instância `tauri dev` e edita
  `NewsMagazineTab.*` em paralelo — COMBINAR antes de tocar o front (ver
  `project_news_tab`).

---

## 3c — Polish da vitrine

- **Sala de Estratégia (pré-corrida):** linha "Público esperado: ~X mil — sua estrela
  puxa Y%" (do `event_interest` + `fame_share` do time do jogador). Fecha o loop
  legível: fama → público → dinheiro, antes mesmo de correr.
- **Dossiê My Team:** nova linha "Bilheteria" na fatura por rodada
  (`get_team_finance_report`, `project_my_team_real_numbers`).

---

## Ordem de ataque sugerida

1. **3a motor** — `gate_income` na fórmula + `grid_total_presence`/`event_prestige_score`
   no call site + linha no `TeamRoundFinanceContext` e no histórico. Testes: fama do
   lineup sobe a bilheteria; evento grande sobe o bolo; piso garante gate a time sem
   estrela. (Rust-only, `cargo check --lib` + testes; não relançar o app do user.)
2. **3c dossiê** — linha "Bilheteria" no relatório financeiro (backend já grava).
3. **3c Sala de Estratégia** — público esperado + cota do jogador (front, COMBINAR).
4. **3b (i)** — nota de astro no world_footer (determinístico).
5. **3b (ii)** — selo de público no spread (front, COMBINAR).

Calibração (MC do `GATE_POT_COEFF`/`GATE_FLOOR_WEIGHT`) fica pro fim, com save real.

## Pontos de integração conhecidos

- `commands/race.rs:76` `calculate_team_round_finance_context` + call site `~2212`.
- `event_interest/calculator.rs` `calculate_expected_event_interest`.
- `public_presence/team.rs` `derive_team_public_presence`.
- `db/queries/teams.rs` `insert_team_finance_history` / `get_team_lineup_medias`.
- `commands/world_footer.rs` `collect_world_notes` / `WorldNote`.
- `src/pages/tabs/NewsMagazineTab.jsx` (+ `.css`).
- Constantes no molde de `IRACER_SALARY_SHARE` (`project_salary_budget_diagnosis`).
