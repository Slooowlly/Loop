# Rivalidade entre Equipes — Design

**Data:** 2026-07-19
**Status:** ⚠️ **Retrato histórico, conferido em 11/08/2026.** Dizia "não implementado", e a
rivalidade entre equipes está no ar: `rivalry/team.rs`, `models/team_rivalry.rs` e a tabela
`team_rivalries`. Leia como a intenção original, e não como estado do app.
**Relacionado:** `2026-07-18-track-rivalry-perception-design.md`. O
`2026-07-11-team-living-reputation-design.md` foi removido na limpeza de 11/08/2026 e continua
recuperável pelo histórico do git.
**Memórias:** `project_team_rivalry_backlog`, `project_team_living_system`, `project_market_live_engine`

---

## 1. Objetivo

Hoje "rivalidade" no código é **exclusivamente piloto↔piloto** (`rivalries.piloto1_id/piloto2_id`
FK→`drivers`; `RivalryType` = Colisao/Companheiros/Campeonato/Pista). Não existe nenhuma tabela,
campo ou score de rivalidade entre **equipes**. Isso trava a cadeia narrativa que o backlog quer:
*"piloto larga a equipe → vai pra rival → ganha o título lá → ex-time em crise"* (o **Elo 2** é o
gargalo — é feature ausente, não ajuste de peso).

Esta feature entrega o Elo 2: um mundo de equipes onde **clássicos nascem** (da tabela, do mercado,
da pista) e onde **o jogador vive** o clássico (sente na moral e na Sala de Estratégia).

### Decisões do usuário (2026-07-19)
- **Fontes:** as **4** (título, roubo de talento, guerra na pista, transbordamento de piloto).
- **Consequências:** **Tier 1 + Tier 2** (narrativa + jogador vive). **Sem Tier 3** (dentes de mercado) por ora.
- **Jogador:** **tem rivais** (narrativa + moral). A retaliação mecânica de mercado (Tier 3, futuro)
  nasce IA-vs-IA.

### Princípio nº1 — reusar o motor
O `rivalry/mod.rs` já é um **motor de dois eixos maduro e genérico**. Não reinventamos nada:
espelhamos a máquina para times. O núcleo puro em `models/rivalry.rs`
(`perceived_intensity`, `rivalry_lifecycle`, `normalize_pair`, thresholds) é **agnóstico de piloto**
(só opera f64/strings) → **reutilizado como está**. Só o que é de domínio (tabela, enum de tipo,
fontes, consequências) é novo.

---

## 2. Modelo de dados

### Tabela `team_rivalries` (nova migração)
Espelha `rivalries` trocando o par de piloto por par de time:

```sql
CREATE TABLE team_rivalries (
    id                   TEXT PRIMARY KEY,
    team1_id             TEXT NOT NULL REFERENCES teams(id),  -- sempre team1_id < team2_id
    team2_id             TEXT NOT NULL REFERENCES teams(id),
    historical_intensity REAL NOT NULL DEFAULT 0,             -- memória lenta (0–100)
    recent_activity      REAL NOT NULL DEFAULT 0,             -- calor volátil (0–100)
    tipo                 TEXT NOT NULL,                       -- TeamRivalryType
    criado_em            TEXT NOT NULL,
    ultima_atualizacao   TEXT NOT NULL,
    temporada_update     INTEGER NOT NULL,
    UNIQUE(team1_id, team2_id)
);
```
Mesma `UNIQUE(par)` do driver (que já rejeita duplicata — ver teste `rivalries_table_rejects_duplicate_pair`).
`perceived = 0.4·historical + 0.6·recent` (idêntico ao de piloto — reusa `perceived_intensity`).

### `TeamRivalryType` (novo enum, `models/team_rivalry.rs`)
Separado do `RivalryType` de piloto porque as origens são diferentes:

| Variante      | Origem                                             |
|---------------|----------------------------------------------------|
| `Campeonato`  | Fonte 1 — briga de construtores na tabela          |
| `Mercado`     | Fonte 2 — roubo de talento (o Elo 2)               |
| `Pista`       | Fonte 3 — carros dos dois times se batendo         |
| `Herdada`     | Fonte 4 — transbordamento de rivalidade de piloto  |

### Motor `rivalry/team.rs` (novo módulo)
Gêmeo enxuto do `rivalry/mod.rs`:
- `apply_team_rivalry_event(conn, &TeamRivalryEvent) -> TeamRivalryApplied` — upsert dois eixos, clamp
  [0,100], com o mesmo tratamento de corrida de constraint do original.
- `get_team_rivalries(conn, team_id) -> Vec<TeamRivalrySummary>` — leitura por time (resolve o "outro lado").
- `apply_season_end_team_rivalry_decay(conn, temporada)` — decaimento anual (§6).

Thresholds semânticos (`AtritoLeve`→`Intensa`) e `crossed_threshold` são reutilizados do módulo de
piloto (são funções puras sobre percebida) — sem duplicar.

---

## 3. As 4 fontes (onde a rivalidade NASCE)

Deltas espelham a escala do sistema de piloto (recente sobe rápido, histórico é memória). **Todo
delta grande tem gate anti-ruído e há cap agregado por temporada** (freio anti-inflação, igual ao
`cap_historical`/`cap_recent` da percepção de pista).

### Fonte 1 — Briga de construtores (`Campeonato`) — a espinha dorsal
**Onde:** fim de temporada, em `evolution/pipeline.rs`, junto do roll-up de reputação/moral, lendo
`team_season_archive` (posição + pontos de construtores, mesma fonte que reputação/moral já usam).
*(Timing de season-end, não ao vivo, para reusar a leitura de archive já existente e evitar precisar
de standings de time ao vivo. Trade-off: a manchete cai no recap do offseason. Aceito; revisável.)*

**Gate:** dentro da mesma categoria/classe, pega os **top-4**; para cada par, rivalidade só se
**ambos no top-3 OU gap de pontos ≤ 15% do total em disputa**. Reforça a cada temporada que a briga
se repete (é assim que um clássico "eterno" cresce no eixo histórico).

**Delta base:** `h=4, r=10` (percebida ≈7.6). **+50%** se decidiu o título (1º vs 2º).

### Fonte 2 — Roubo de talento (`Mercado`) — o Elo 2, a peça-chave
**Onde:** todo site do mercado onde o `equipe_id` de um piloto muda de B→A:
`market/pipeline.rs::run_poaching_pass` (assédio mid-contrato, tem `transfer_between_teams`),
a escada paginada (`fill_vacancies_paced`/`fill_remaining_vacancies_with_rookies`) e o move por
renovação-negada. **Um helper único** `seed_team_rivalry_from_transfer(conn, from_team, to_team,
driver, kind, temporada)` chamado em cada site.

**Delta escalado** (o rancor é proporcional ao que se perdeu e ao descaramento):

| Situação                                             | h  | r  |
|------------------------------------------------------|----|----|
| Astro/N1 assediado mid-contrato (poaching)           | 8  | 16 |
| Astro sai de graça pro rival direto (fim de contrato)| 6  | 12 |
| Titular comum trocando de casa                       | 3  | 8  |
| Reserva / peça menor                                 | 1  | 4  |

- **×0.5** se os dois times NÃO estão na mesma categoria (rivalidade entre divisões pesa menos).
- "Astro/N1" = skill alta **ou** `midia` alta **ou** era o `hierarquia_n1_id` do time de origem.
- **Bidirecional** (o par é normalizado; não importa quem "começou").
- É isto que dá **memória duradoura** ao "piloto largou e foi pro rival" — hoje o destino do piloto é
  só prestígio+carro+dinheiro (`driver_offer_score`), sem deixar marca no mundo.

### Fonte 3 — Guerra na pista (`Pista`) — add-on barato
**Onde:** piggyback em `commands/race.rs` (logo após `process_collisions_rivalry`, linha ~1148),
reusando o mesmo `flat_incidents`. Novo `process_team_collisions_rivalry(conn, incidents, categoria,
rodada, temporada)`: resolve o time de cada piloto envolvido em colisão, agrega **por par de times**,
pega a **severidade máxima** entre os dois no evento.

**Delta (por corrida, capado):** `h=2, r=6`. Só conta colisão entre times **diferentes** (bater no
próprio companheiro não é rivalidade de time). Cap de `r` por temporada por par para dois times
azarados não inflarem sozinhos.

### Fonte 4 — Transbordamento de piloto (`Herdada`) — o "Verstappen×Hamilton → RBR×Merc"
**Onde:** ao fim da corrida em `commands/race.rs`, depois das rivalidades de piloto já aplicadas.
Novo `process_driver_rivalry_bleed(conn, categoria, temporada)`: varre rivalidades de piloto
**vivas e intensas** (percebida ≥ **60**, faixa `Forte`) cujos dois pilotos estão em **times
diferentes**; pinga um trickle na rivalidade dos times.

**Delta:** `h=1, r=3` por par intenso e por corrida (trickle deliberadamente pequeno — é eco, não
origem). Cap por temporada.

---

## 4. Consequências

### Tier 1 — Narrativa (reusa o pipeline de notícias)
Ao **cruzar um threshold** de percebida (mesma lógica `crossed_threshold` do piloto), gera manchete.
Voz jornalística em 3ª pessoa (consistente com o rodapé "Do mundo do Grid").

- Manchetes por origem: *"O clássico do grid esquenta: [A] x [B]"* (Campeonato),
  *"[A] fisga [Piloto] de [B] e acende a rivalidade"* (Mercado), etc.
- Aparece no **dossiê do time** (o dossiê já é exposto em `commands/transfer_market.rs`): "Maiores
  rivais: [B] (rivalidade forte)".
- **Wiring de schema:** o `NewsItem` hoje tem um único `team_id`. Precisa de um **segundo campo de
  time** (ex.: `team_id_secondary`) **ou** um `NewsType::RivalidadeEquipe` dedicado que reaproveite
  os slots. Toque pequeno de schema — decidir na implementação.

### Tier 2 — O jogador VIVE (a lacuna central do backlog)
**2a. Moral de derby (per-race, simétrico jogador+IA).** Novo `apply_derby_morale(conn, race_results,
categoria, temporada)`: para **todo par de times com rivalidade viva presente na corrida**, compara o
melhor carro de cada; **vencer o rival empurra a moral pra cima, perder empurra pra baixo**, dentro da
banda [0.5, 1.5] que a moral já respeita.
- Magnitude **sutil**, escalada pela intensidade: `±BASE · (0.5 + percebida/100)`, `BASE ≈ 0.015`.
- **É um movimento NOVO de moral no meio da temporada** — hoje `advance_team_morale` só roda no
  offseason. O derby dá à moral um pulso vivo por corrida. Precisa ser pequeno pra não engolir o
  modelo sazonal (calibrar via MC — §7).
- Simétrico: vale pro time do jogador **e** pras IAs (mesmo princípio da moral atual).
- A moral já é SENTIDA na pista (`morale_pace_delta`/`morale_reliability_delta` em
  `simulation/context.rs`) → derby vira ritmo/confiabilidade reais na corrida seguinte. **Loop fechado
  sem tocar em mercado.**

**2b. Enquadramento na Sala de Estratégia (briefing pré-corrida).** Injeta um fato de rivalidade no
builder de fatos do briefing (mesmo padrão do `build_recent_arc_facts` do arco narrativo): *"Hoje você
encara [rival] — seu maior rival desde [origem/contexto]."* Some:
- Um **badge determinístico** na Sala de Estratégia (aparece mesmo sem IA/proxy) marcando a corrida
  como derby.
- Fecha o loop pré→pós: o debrief pode reler ("bater o rival era o objetivo do dia").

---

## 5. Regras de inclusão do jogador

- **Fontes 1, 2, 4 e Tier 1/2:** o time do jogador **participa normalmente** — ganha rivais por
  título, por perder/roubar piloto, e vive na moral e no briefing.
- **Fonte 3 (colisões):** o contato do jogador vem do monitor ao vivo (já existe) → entra igual.
- **Tier 3 (dentes de mercado):** **fora do escopo** agora. Quando vier, nasce **IA-vs-IA**
  (o `run_poaching_pass` já exclui o jogador dos dois lados — `market/pipeline.rs:1069`), pra não punir
  o jogador de forma imprevisível antes de calibrar.

---

## 6. Decaimento e ciclo de vida

Idêntico ao de piloto (`apply_season_end_rivalry_decay`), novo
`apply_season_end_team_rivalry_decay(conn, temporada)` hookado em `evolution/pipeline.rs:187`
(ao lado do decay de piloto):
- **Ativa nesta temporada** (`temporada_update == atual`): `recent *= 0.5`, histórico intacto.
- **Inativa**: `recent *= 0.2`, `historical *= 0.85`.
- Ciclo `Extinta` (ambos eixos baixos) → **removida do banco** (reusa `rivalry_lifecycle`).

Isso garante que clássicos ativos **persistem e crescem** no histórico, enquanto rivalidades pontuais
(uma briga de pista isolada) **esfriam e somem** sozinhas — sem intervenção.

---

## 7. Calibração / Monte Carlo

Novas métricas em `sim_stats` (padrão das ideias 1–3 do sistema vivo):
- `team_rivalry_count` — nº de rivalidades vivas por temporada (alvo: existe, não explode).
- `team_rivalry_perceived` — distribuição da percebida (soma/soma²/min/max/n).
- `team_rivalry_by_source` — contagem por `TeamRivalryType` (as 4 fontes contribuem? alguma domina?).
- `derby_morale_swing` — amplitude do pulso de moral de derby (confirmar que ficou **sutil**).

**Alvos de calibração:**
1. Rivalidades vivas **existem e são poucas** (clássicos, não ruído) — provável 3–8 vivas/categoria.
2. A **moral de derby NÃO desestabiliza** o modelo sazonal (dinastias, car_performance por tier e
   colapso% ~inalterados vs baseline atual — mesmos invariantes que as ideias 1–3 protegeram).
3. As 4 fontes **todas contribuem** (nenhuma morta; título+mercado dominam por design).

---

## 8. O que deliberadamente NÃO se toca

Respeitando o backlog e a fragilidade conhecida (`project_market_fragility_hardening`,
`project_grid_skill_deflation`):
- **Espiral de crise (Elo 4)**, piso de reputação (`FLOOR=25`), venda/falência de time → **congelados**.
- Esta feature é **puramente aditiva**: um score + notícia + pulso de moral. **Não exige soltar nenhum
  freio** que segura a grade. Mantém a ordem acordada: **rivalidade primeiro, crise por último.**

---

## 9. Plano de fases (implementação incremental)

1. **Fundação** — migração `team_rivalries`, `models/team_rivalry.rs` (enum + model), `rivalry/team.rs`
   (upsert/leitura/decay) + testes de unidade. Nada plugado ainda. *(Espelha o passo 1 do sistema de piloto.)*
2. **Fonte 1 (Campeonato)** — hook no offseason + manchete. Primeira rivalidade emergente visível.
3. **Fonte 2 (Mercado)** — o helper de transferência nos 3 sites. **É aqui que o Elo 2 fecha.**
4. **Fontes 3 + 4** — colisões de time + transbordamento (add-ons baratos, dado já existe).
5. **Tier 1 completo** — schema de notícia (2º time), dossiê do time.
6. **Tier 2** — moral de derby (2a) + briefing/badge (2b).
7. **Calibração MC** — métricas §7, ajuste de deltas/caps, validar invariantes.

---

## 10. Riscos

- **Inflação de rivalidades** (tudo vira rival) → mitigado por gates + caps por temporada + decaimento
  agressivo do recente. Medir com `team_rivalry_count`.
- **Moral de derby engolindo o modelo sazonal** → magnitude sutil + MC comparando invariantes.
- **Schema de notícia** (2 times num `NewsItem` de 1 time) → decisão pequena mas real na Fase 5.
- **Fonte 1 no offseason** (manchete atrasada vs run-in ao vivo) → aceito; se incomodar, migrar pra
  standings de time ao vivo nas últimas rodadas (custo: query nova de pontos de construtor ao vivo).
