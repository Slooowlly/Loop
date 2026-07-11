# Reputação viva das equipes — design

**Data:** 2026-07-11
**Escopo:** ideia 1 do redesign "Sistema de equipes vivo" (analisado a partir do mapa do
sistema atual). As ideias 2 (acumular títulos no registro do time) e 3 (moral viva)
ficam para depois.

## Problema

`reputacao` (0–100) é semeada na criação da equipe (`reputacao_base ± 3`) e só muda em
degraus discretos: promoção (+3..8), rebaixamento (−25..−15) e venda do time. **Nunca por
resultado.** No entanto ela já é lida por quatro sistemas:

| Consumidor | Efeito | Local |
|---|---|---|
| Patrocínio (receita) | termo `reputacao × base × 0.004` | `commands/race.rs:91` |
| Teto salarial | `0.85 + reputacao/260` (swing ~45%) | `finance/salary.rs:10` |
| Saúde financeira → estratégia | `support_score = (budget+reputacao)/2`, peso 0.15 | `finance/state.rs:25` |
| Elite/dinastia (Pilar D) | `elite_score = reputacao + titulos×2 + marca?1000` | `finance/strategy.rs:30` |

O encanamento já existe e é caro; falta só a reputação **se mover**.

## Modelo

Atualização **anual, determinística** (sem RNG → reproduzível no Monte Carlo), com duas forças.

**1. Alvo por desempenho.** Do `team_season_archive` (posição no campeonato + tamanho do grid
do grupo categoria+classe):

```
força = 1 − 2·(posição−1)/(grid−1)        → 1º = +1, último = −1, meio = 0
alvo  = max(BASE + força·AMPLITUDE, TARGET_FLOOR)
```

**2. Inércia + reversão à média.** A reputação caminha até o alvo, não pula:

```
reputação_nova = reputação + (alvo − reputação)·ADJUST_RATE
                 + (campeão? TITLE_KICKER : 0)
                 clamp na banda [FLOOR, CEIL]
```

### Constantes (travadas com o usuário)

| Constante | Valor | Papel |
|---|---|---|
| `BASE` | 48 | reputação de uma equipe mediana em regime (âncora da reversão) |
| `AMPLITUDE` | 45 | meia-amplitude do alvo (campeão mira ~93, lanterna ~3 antes do piso) |
| `ADJUST_RATE` | 0.20 | inércia **Média**: sobe/desce em 3-4 temporadas |
| `TITLE_KICKER` | 2 | empurrão de legado do título |
| `TARGET_FLOOR` | 22 | piso do alvo (convergência limpa junto ao piso da banda) |
| `FLOOR` / `CEIL` | 25 / 98 | banda; piso **Protegido** evita espiral de morte do fundo |

Equilíbrios resultantes: campeão perpétuo ~98, pódio consistente ~70, meio ~48, lanterna
crônica ~25.

## Onde liga

`evolution/pipeline.rs`, no offseason, **logo após `archive_team_season`** (posições
conhecidas) e **antes de `run_promotion_relegation`** (que soma o próprio degrau por cima).
Nova função pura em `finance/reputation.rs`:
`advance_team_reputation(current, position, grid_size) -> f64` + orquestrador
`update_team_reputations_from_season(conn, season)`.

## Interações

- **Dinastias (Pilar D):** a reputação viva faz o `elite_score` das equipes fictícias
  (Production e grids sem marca) rankear por mérito de verdade — a elite passa a girar
  conforme quem vem ganhando. Marcas reais seguem fixas (+1000). Verificar que a
  concentração de títulos premium **não estoura** o teto calibrado (~56.8% da top).
- **Salário/patrocínio:** como a reputação reverte à média (~48), a *média* do grid fica
  estável; só a *dispersão* cresce (vencedor paga mais e pode; perdedor menos). Verificar
  massa salarial total.
- **Espiral de morte do fundo:** controlada pelo `FLOOR = 25` (piso Protegido).

## Riscos / calibração

Métrica nova em `sim_stats`: dispersão de reputação por tier (média/desv.pad/min/max).

**Resultado (MC 8 runs × 15 temporadas, `ADJUST_RATE=0.20`, `FLOOR=25`):**
- Dispersão de reputação cresceu de ~±3 (semente) para **desv.pad 12–19 por tier**;
  médias escalam por prestígio de categoria (rookie 33.8 → endurance 67.0), máximos 87–99,
  média global 50.6 (sem inflação).
- Dinastias: **4.73 vencedores únicos/classe premium, 58.3% da top** (era 4.53 / 56.8% antes
  da reputação viva) — preservadas, dentro da banda "dinastia com sustos". O top share segue
  ~3 pp acima do alvo de 55%, mas isso já vinha do Pilar D; a alavanca continua sendo o PISO
  de recursos das elites, não a reputação.
- Salários por tier batem com a calibração anterior (endurance ~159k) → sem inflação.
- Colapso 4.8% dos estados-temporada; 65% se recuperam; sem espiral de morte (piso Protegido
  segurou o fundo).

Conclusão: constantes travadas (Média / Protegido) produziram a separação topo/fundo desejada
sem quebrar dinastias, salários ou estabilidade do grid. Nenhum ajuste necessário.

## Fora de escopo (ideia 1)

Momentum por corrida (reputação de curto prazo); moral viva (ideia 3).

---

# Ideia 2 — Histórico de carreira vivo (títulos acumulados)

**Data:** 2026-07-11.

## Problema

Os campos `historico_*` do registro do time (`historico_vitorias/podios/poles/pontos`,
`historico_titulos_pilotos`, `carreira_titulos` = `historico_titulos_construtores`) **nunca
eram gravados** — ficavam em 0. O `team_season_archive` sabe os totais por temporada, mas nada
faz o roll-up. Consequência: o termo `titulos_construtores*2` do `elite_score`
([`finance/strategy.rs:30`]) é **código morto** — a elite fictícia (Production) é ordenada só
por reputação, e o dossiê da equipe (que já expõe esses campos em `commands/transfer_market.rs`)
mostra 0.

## Modelo

`world/team_archive.rs::roll_up_team_career_history(conn)` — **recompute-from-archive**
(idempotente, faz backfill de saves antigos):
- Construtores: `SUM(vitorias/podios/poles/pontos/titulos_construtores)` sobre
  `team_season_archive` agrupado por `team_id` (uma varredura).
- Títulos de piloto: `COUNT` sobre `driver_season_archive` filtrando `titulos=1` e agrupando
  pelo `team_id` do snapshot JSON (`json_extract`) — atribui o título à equipe daquela temporada.
- Grava só as 6 colunas de histórico por time; equipes sem arquivo mantêm 0.

**Gancho:** offseason em `evolution/pipeline.rs`, logo após `update_team_reputations_from_season`
(archive já gravado).

## Tensão de design

Ideia 1 (reputação) **reverte à média** — sucesso passado desvanece. Ideia 2 (títulos)
**acumula** — sucesso passado é memória permanente. Juntas no `elite_score`: dinastia
reforçada por forma (reputação) + legado (títulos). Risco = a elite **congelar** (top-3 fixos
para sempre via títulos acumulados), matando o "4º desafiante orgânico". Alavanca de calibração
= o peso `*2` em `elite_score` (reduzir para `*1..1.5` se travar).

## Calibração (MC 8×15)

Com os títulos alimentando o `elite_score`, as dinastias **não congelaram** — afrouxaram
levemente para dentro do alvo:

| Métrica | Só ideia 1 | Com ideia 2 |
|---|---|---|
| Vencedores únicos/classe premium | 4.73 | **4.80** |
| Fatia da equipe top | 58.3% | **56.4%** (no alvo ~55%) |

Motivo: a reputação (magnitude ~25–98) segue o sinal dominante do `elite_score`; os títulos
acumulados (`×2` = +2..+10) adicionam legado sem travar. Como ~74 equipes ganham ≥1 título ao
longo do save, o termo se espalha e diversifica a elite em vez de congelar top-3. Reputação
(desv.pad 12–19), salários e taxa de colapso inalterados. **Peso `*2` mantido — nenhum ajuste.**
1683 testes verdes.

## Fora de escopo (ideia 2)

DNA/identidade e rivalidade entre times (ideias 4-5).

---

# Ideia 3 — Moral viva

**Data:** 2026-07-11. **Requisito do usuário:** a moral tem que afetar **tanto o jogador quanto
as IAs** (mecanismo simétrico, não flavor de um lado só).

## Problema

`morale` (0.5–1.5, neutro 1.0) já era LIDA em dois lugares — eficiência de dev do carro no
offseason (`cashflow.rs:234`) e o sinal de comportamento da IA no export do iRacing
(`behavior.rs:512`) — mas `update_team_morale` nunca era chamada → congelada em ~1.0 e invisível
(não entrava na simulação de corrida).

## Modelo (`finance/morale.rs`)

**A) Movimento (sazonal, volátil — moral é humor).** `advance_team_morale(moral, força, tensão)`:
```
strife = max(0, (tensão − 50)/50)                 # só treta ACIMA de 50 pune; garagem calma = neutra
alvo   = 1.0 + força·RESULT_SPAN − strife·STRIFE_SPAN
moral_nova = moral + (alvo − moral)·ADJUST_RATE    → clamp [0.5, 1.5]
```
força = posição vs grid (−1..+1, do archive); tensão = `hierarquia_tensao` (clima N1/N2).
Constantes (travadas): `ADJUST_RATE=0.35` (**Volátil**), `RESULT_SPAN=0.4`, `STRIFE_SPAN=0.35`.
Equilíbrios: campeão calmo ~1.4, meio ~1.0, lanterna ~0.6, meio-em-crise ~0.65.

**B) Efeito na simulação (sutil, SIMÉTRICO).** `morale_pace_delta` / `morale_reliability_delta`
entram em `simulation/context.rs` para TODO `SimDriver` (corridas simuladas do jogador + IA por
igual): `car_performance += (moral−1)·PACE_SPAN`, `car_reliability += (moral−1)·RELIABILITY_SPAN`.
Constantes (**Sutil**): `PACE_SPAN=1.0` (±0.5 nos extremos), `RELIABILITY_SPAN=6.0` (±3).
Cuidado: `from_driver_team_and_track` reescreve car_performance via `effective_car_performance`
→ o delta de moral é re-aplicado lá (a confiabilidade vem do construtor base, não é reescrita).

**Como o jogador sente:** time em alta (supera + harmonia) → carro um tico mais rápido/confiável
nas corridas simuladas + evolui mais rápido no offseason; time em crise → carro pior e frágil.
Simétrico com a IA. Na pista real, a moral segue moldando o comportamento dos adversários (export).

**Gancho:** offseason em `evolution/pipeline.rs`, após o roll-up de histórico. Promoção/rebaixamento
aplicam os multiplicadores de moral (×1.15 / ×0.60) depois.

## Calibração (MC 8×15)

- **Moral viva:** média 0.984, **desv.pad 0.226**, banda cheia (min 0.50 / max 1.50) — era travada
  em 1.0. Média levemente < 1.0 porque a treta interna só pune (assimétrico), realista.
- **Sutil não desestabilizou:** dinastias 4.91 únicos / 57.1% top (vs 4.80 / 56.4% na ideia 2 —
  variação marginal, dentro da banda "dinastia com sustos"). `car_performance` por tier IDÊNTICO
  (a moral afeta o resultado da corrida, não a evolução do carro). Confiabilidade média 75.4
  (inalterada). 1691 testes verdes. **Nenhum ajuste — PACE_SPAN/ADJUST mantidos.**

## Fora de escopo (ideia 3)

Momentum de moral DENTRO da temporada (hoje sazonal); DNA/identidade e rivalidade entre times
(ideias 4-5).
