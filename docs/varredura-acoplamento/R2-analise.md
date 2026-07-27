# R2 — Segunda análise: três motores de tese

**Veredito curto:** a duplicação das TESES é aceitável e deve ficar. A duplicação
dos SINAIS não é — e não é teórica: há um **defeito real** de escala de unidade
que já produz o debrief e o boletim classificando o mesmo piloto na mesma corrida
de formas contraditórias. O item de maior valor aqui não é refactor, é conserto.

---

## 1. Tabela de sinais

Onde cada conceito é derivado, por motor. Onde o limiar diverge, está marcado.

### 1.1 DNF mecânico vs. erro/contato

| Motor | Local | Como decide |
|---|---|---|
| Debrief | [`ai_news/fatos.rs:621`](../../src-tauri/src/commands/ai_news/fatos.rs#L621) | `player_mech_break` (peça grave quebrada) **OU** `dnf_reason` casando uma lista de palavras-chave textuais |
| Boletim | [`narrative/beats.rs:193`](../../src-tauri/src/narrative/beats.rs#L193) | `is_crash(dnf_inc)` sobre o `IncidentResult` (+10 de peso). Nunca produz o rótulo "mecânico"; só usa `dnf_reason_of(d)` como texto livre |
| Prévia | [`nextRaceThesis.js:71`](../../src/pages/tabs/nextRaceThesis.js#L71) | só `last.is_dnf` — não distingue |

**Divergência:** três fontes de verdade distintas para a mesma pergunta —
peças + regex de texto (debrief), tipo do incidente (boletim), nada (prévia).
O boletim pode chamar de batida (`is_crash`) um DNF que o debrief classifica como
mecânico, porque um lê `IncidentType` e o outro lê a string do motivo.

### 1.2 Remontada

| Motor | Local | Limiar |
|---|---|---|
| Debrief (tese) | [`ai_news/tese.rs:67`](../../src-tauri/src/commands/ai_news/tese.rs#L67) | `gained >= 5 && !underperf` |
| Boletim (tese) | [`narrative/tese.rs:119`](../../src-tauri/src/narrative/tese.rs#L119) | `gained >= 8 && finish <= 6` e não ser o vencedor |
| Boletim (beat) | [`narrative/beats.rs:116`](../../src-tauri/src/narrative/beats.rs#L116) | `gained > 0`, peso `30 + 2·gained` |
| `race_eval` (headline) | [`race_eval.rs:224`](../../src-tauri/src/race_eval.rs#L224) | `gained >= 4` |

**Quatro limiares** para "remontada": 4, 5, 8 e >0.

### 1.3 Colapso

| Motor | Local | Limiar |
|---|---|---|
| Debrief | [`ai_news/tese.rs:80`](../../src-tauri/src/commands/ai_news/tese.rs#L80) | `gained <= -4` **OU** (`underperf` e `grid <= target_low`) |
| Boletim | [`narrative/tese.rs:41`](../../src-tauri/src/narrative/tese.rs#L41) | só existe para o **pole**: `!dnf && finish >= 5` |
| Prévia | [`nextRaceThesis.js:46,72`](../../src/pages/tabs/nextRaceThesis.js#L46) | `position >= averageFinish + 8` (`DISMAL_MARGIN`) |

**Assimetria estrutural:** o boletim só enxerga colapso de quem largou na pole.
Um piloto qualquer que perde 12 posições não é candidato a eixo — só aparece
como fato de contexto, e só se estiver em `featured`.

### 1.4 Over/under performance vs. expectativa

Os dois motores Rust chamam **a mesma** `race_eval::evaluate`. O que diverge é a
construção do campo de mérito — e é aqui que mora o defeito.

| | Debrief ([`race/importacao.rs:38`](../../src-tauri/src/commands/race/importacao.rs#L38)) | Boletim ([`race/fatos.rs:336`](../../src-tauri/src/commands/race/fatos.rs#L336)) |
|---|---|---|
| `car_norm` | `Team::car_strength()` → **0–100** | `SELECT car_performance FROM teams` → **−5..16, cru** |
| Fonte do carro | modelo de peças (`car`), com fallback pra coluna reescalada | coluna legada, que **o sistema de peças nunca atualiza** |
| Forma recente | `recent_avg_finish(...)` → mistura 15% | `None` → não mistura |
| Campo | `filter_map` — piloto sem linha no DB **some do grid** e encolhe `field_size` | grid inteiro, com defaults 50.0 |

`compute_merit` faz `0.5·skill + 0.5·car_norm` esperando ambos em 0–100
([`race_eval.rs:37`](../../src-tauri/src/race_eval.rs#L37)). Alimentado com o valor
cru (domínio −5..16, confirmado em
[`vacancies.rs:510`](../../src-tauri/src/commands/career/vacancies.rs#L510) e
[`car/sim_bridge.rs:14`](../../src-tauri/src/car/sim_bridge.rs#L14)), o componente
"carro" some: no caminho do boletim o mérito é **essencialmente só skill**.

Consequência: o mesmo piloto tem duas posições-potencial diferentes, logo duas
faixas de meta diferentes, logo dois `Assessment` diferentes — na mesma corrida.

### 1.5 Vitória / pódio improvável

| Motor | Local | Regra |
|---|---|---|
| Debrief | [`ai_news/tese.rs:55`](../../src-tauri/src/commands/ai_news/tese.rs#L55) | `finish == 1` (sem noção de improbabilidade) |
| Boletim (tese) | [`narrative/tese.rs:90`](../../src-tauri/src/narrative/tese.rs#L90) | `winner_grid >= 6` |
| Boletim (beat) | [`narrative/beats.rs:69`](../../src-tauri/src/narrative/beats.rs#L69) | `grid >= 6`, bônus `min(grid−5, 15)` |

Único conceito onde os dois lados do boletim já concordam no limiar (6). Sem
conflito com o debrief, que é player-cêntrico e não precisa do conceito.

### 1.6 Caos

Só existe no boletim: `total_dnfs >= max(4, field_size/4)`
([`narrative/tese.rs:81`](../../src-tauri/src/narrative/tese.rs#L81)), com um
segundo limiar solto de `>= 2` para a linha de cabeçalho
([`narrative/contexto.rs:63`](../../src-tauri/src/narrative/contexto.rs#L63)).
O debrief não tem o conceito, e legitimamente não precisa.

---

## 2. A incoerência é observável? Sim.

Os dois textos são gerados para a **mesma corrida**, e o jogador está sempre em
`featured` ([`fatos_boletim.rs:43`](../../src-tauri/src/commands/race/noticias/fatos_boletim.rs#L43)),
então o boletim opina sobre o desempenho dele no mesmo `#[...]` de fatos que vai
pra IA.

### Cenário A — contradição direta (piloto bom, carro ruim)

Grid de 20. Jogador: skill 88, equipe com carro fraco (`car_strength` ≈ 25, coluna
crua ≈ 0,25). Os rivais de IA: skill ≈ 60 em carros fortes (`car_strength` ≈ 85,
coluna crua ≈ 13). Largou **P6**, terminou **P8**, 3 incidentes, sem DNF.

**Debrief** — mérito jogador = 0,5·88 + 0,5·25 ≈ 56; IA = 0,5·60 + 0,5·85 ≈ 72.
Jogador ranqueia ~P14 de potencial. `expected = 0,6·6 + 0,4·14 = 9,2` →
meta **P8–P11**. Chegou P8 → **Dentro**. `gained = −2` (não passa de −4), sem
over/under → cai na regra 9: *"dia de somar, P8"*.

**Boletim** — mérito jogador = 0,5·88 + 0,5·**0,25** ≈ 44; IA = 0,5·60 + 0,5·**13** ≈ 37.
O jogador vira o **melhor conjunto do grid**, potencial P1. `expected = 0,6·6 + 0,4·1 = 4,0`
→ meta **P3–P5**. Chegou P8, com agravante (3 incidentes) → **MuitoAbaixo** →
imprime `briefing.perf.much_below`: *"largou P6 e caiu para P8, muito abaixo do
esperado"* ([`fatos.rs:380`](../../src-tauri/src/commands/race/fatos.rs#L380)).

Mesma corrida, mesmo piloto, mesmo motor `evaluate`: **"dia de somar" vs. "muito
abaixo do esperado"**. E ambos os textos alimentam o enriquecimento por IA.

O cenário espelhado (piloto médio em carro forte) inverte: o debrief acusa
subdesempenho e o boletim não vê nada de errado.

### Cenário B — divergência de limiar, benigna

Jogador larga P14 e termina P8 (`gained = 6`), vencedor saiu da pole.
Debrief → **Remontada** (≥5). Boletim → o `biggest_recovery` exige ≥8 e `finish ≤ 6`,
não dispara; o eixo vira **Domínio** do vencedor, e a recuperação entra como beat
de destaque (peso 42, passa o limiar).

Isso **não** é incoerência: são as duas vozes fazendo o trabalho delas. A revista
conta a corrida, o debrief conta a sua corrida. Não mexa.

**Conclusão do item 2:** existe incoerência real, mas ela está toda concentrada no
sinal 1.4 (over/under), e a causa é um bug de unidade — não a duplicação de teses.

---

## 3. `race_eval` generaliza?

**Já generalizou.** `performance_context_facts` chama `evaluate` num laço sobre
`featured` passando cada `pilot_id` como `player_id`
([`fatos.rs:362`](../../src-tauri/src/commands/race/fatos.rs#L362)). Não falta
dado: grid de largada vem do `RaceResult`, força do carro vem de `teams`, skill
vem de `drivers`. Os dois call-sites já têm exatamente os mesmos insumos
disponíveis.

O custo da "generalização" é, portanto, **zero** — o que falta é o oposto:
**unificar a construção do campo**, hoje escrita duas vezes com semânticas
diferentes. Uma função, ~30 linhas, chamada pelos dois.

Ressalva legítima: `evaluate` embute uma decisão player-cêntrica em
`build_headline`/`build_team_read` (prosa em 2ª pessoa implícita). Para uso no
grid inteiro isso é ignorado — `performance_context_facts` só lê `assessment`.
Se a camada compartilhada for adiante, vale separar `evaluate` (números) de
`evaluate_display` (frases), senão gera-se prosa descartada para 20 pilotos.

---

## 4. A camada de sinais compartilhada

Duas metades, por causa da restrição de pureza do `narrative`:

### 4a. `race_eval::campo` — impuro (lê DB), o conserto de verdade

```rust
pub fn build_merit_field(conn: &Connection, result: &RaceResult) -> Vec<DriverMerit>
```

Uma única definição de: qual coluna de carro (`effective_car_performance()`,
nunca a crua), se entra forma recente, o que fazer com piloto ausente do DB
(manter com default, **nunca** encolher o grid). Substitui os dois blocos de hoje.

### 4b. `race_signals` — puro, sem `rusqlite`, consumível pelo `narrative`

Fatos por piloto, um registro por linha do grid:

```
grid, finish, gained, is_dnf, dnf_kind: {Mecanico|Contato|Erro|Desconhecido},
assessment, target_low, target_high, has_fastest_lap, is_player, is_winner, is_pole
```

Fatos da corrida: `field_size`, `total_dnfs`.

E — o ponto — os **predicados nomeados**, um limiar cada:
`remontada(d)`, `colapso(d)`, `overperf(d)`, `underperf(d)`, `caos(race)`,
`vitoria_improvavel(race)`.

O `narrative` recebe `&RaceSignals` pronto em vez de construir; continua puro e
testável, e o doc-comment de `RaceThesisSignals` continua honesto.

### O que fica **de fora** (é voz, não fato)

- **A ordem de eleição.** Cada motor mantém seu `select_*` e sua prioridade. É aqui
  que mora a decisão de design que o R2 alertou para não destruir.
- **Os `statement` e as chaves i18n.** Revista ≠ debrief.
- **As listas de promoção** (`Vec<BeatKind>` vs `Vec<&'static str>`) — vocabulários
  diferentes de propósito.
- **Os pesos dos beats** ([`beats.rs`](../../src-tauri/src/narrative/beats.rs)) —
  curadoria editorial, não fato.
- **`PostRaceDuel`** (nemesis/rival, h2h) — relacional ao jogador, não é fato da corrida.
- **`pole_flopped` e `biggest_recovery` como singletons** — "quem é o protagonista"
  é escolha de voz. A camada expõe as flags por piloto; cada motor elege o seu.

---

## 5. O frontend entra? **Não.**

`nextRaceThesis.js` é pré-corrida: seus sinais são classificação, histórico de
pista, clima, quebra e estado de campeonato — nenhum resultado existe ainda. É um
conjunto **estruturalmente diferente**, não uma terceira cópia do mesmo.

O único conceito que se sobrepõe é `dismal` (`position >= averageFinish + 8`), que
é uma leitura **retrospectiva** da corrida anterior — cujo `assessment` e `grade`
o backend já calculou e persistiu em `race_screens/{race_id}.json`. Então a
correção de coerência no frontend não é unificação: é **parar de recalcular
`dismal` em JS e ler o `assessment` persistido**. Item pequeno, isolado, opcional.
Todo o resto do arquivo fica onde está.

---

## 6. Recomendação

**Fazer parcialmente.** Em três fatias de valor decrescente:

| | O quê | Custo | Toca `narrative/`? |
|---|---|---|---|
| **P0 — ✅ feito** | `build_merit_field` em [`commands/race/merito.rs`](../../src-tauri/src/commands/race/merito.rs) vira a única construção do campo de mérito; `performance_context_facts` e `compute_race_evaluation` passam a chamá-la | ~1h | **não** — `commands/race/{merito,fatos,importacao}.rs` |
| **P1 — ✅ feito** | [`race_signals.rs`](../../src-tauri/src/race_signals.rs): `dnf_kind` + os predicados nomeados, um limiar por conceito. Consumido por `narrative/{tese,beats}.rs`, `commands/ai_news/{tese,fatos}.rs` e `race_eval.rs` | ~4–6h | **sim** — coordenar com o R1 |
| **P2 — opcional** | Frontend lê o `assessment` persistido em vez de recalcular `dismal` | ~1h | não |

**Não fazer:** um `RaceFacts` único e completo consumido pelos três seletores. Os
motores precisam de recortes genuinamente diferentes (o boletim quer o grid raso,
o debrief quer um piloto fundo, a prévia não quer resultado nenhum), e o custo de
manter uma struct que cada consumidor usa pela metade supera o ganho.

**Não fazer:** qualquer fusão das vozes. As três teses estão certas em existir.

Nota franca: sua suspeita do briefing estava meio certa e meio errada. A
duplicação de **teses** é aceitável, como você desconfiava. Mas a duplicação de
**sinais** escondeu um bug de unidade que já está em produção — e ele sozinho
justifica o P0 independentemente de qualquer refactor.
