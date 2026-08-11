# Rivalidades nascidas em pista — a camada de PERCEPÇÃO (SDK iRacing)

**Data:** 2026-07-18
**Status:** ⚠️ **Retrato histórico, conferido em 11/08/2026.** Dizia "NÃO implementado", e a
percepção está no ar: `iracing_sdk/rivalry_perception.rs` mais o comando
`iracing_perceive_rivalries`, citados por `commands/iracing/resultado.rs` e
`corridas_salvas.rs`. Leia como a intenção original, e não como estado do app. O estado de hoje
está no [DESIGN.md](../../DESIGN.md) §15.2 e §19.3.
**Escopo:** como o jogo PERCEBE, dos dados reais do SDK do iRacing, que uma
rivalidade nasceu/cresceu numa corrida que o JOGADOR disputou de verdade. É a camada
de ENTRADA que alimenta o motor de rivalidade que já existe.

**Relacionado:** `2026-07-18-player-rivalry-nemesis-design.md`, o Nemesis vivido mais a Pressão
de Duelo, que consomem o que esta camada produz. Esse arquivo foi removido na limpeza de
11/08/2026 e continua recuperável pelo histórico do git; o Nemesis está no ar, na tabela
`player_nemesis`. `project_iracing_results_import`
(onde roda). `project_iracing_ai_roster` (resolução de identidade). `project_export_behavior`
(o sinal `nemesis` primitivo que isto formaliza).

---

## 1. Motivação

O motor de rivalidade hoje só é alimentado pela SIMULAÇÃO OFFLINE
(`process_collisions_rivalry`, campeonato, hierarquia). Quando o jogador corre no
iRacing, a briga REAL — roda a roda, contra um oponente identificável — não vira
rivalidade. O sinal `nemesis` do export (`behavior.rs:670`) já faz uma heurística
primitiva ("cruzou a linha lado a lado ±1 com o mesmo rival em ≥2 corridas") mas NÃO
realimenta o motor. Esta camada formaliza, aprofunda e fecha o loop.

**Decisões do usuário (2026-07-18):**
- **Um eixo só de intensidade** (sem distinguir rancor vs respeito — tom fica pra fase 2).
- **Quatro sinais criam rivalidade:** contato atribuído, duelo prolongado, pêndulo de
  posições, ultrapassagem decisiva.
- **Só pilotos mapeados na carreira** (car_number → driver_id via roster). Oponente
  não-mapeado é ignorado (não persiste).
- Sem postura pré-corrida. A rivalidade **nasce da pista**, emergente.

---

## 2. Matéria-prima (o que `RaceHistory` + `Attempt` já gravam)

| Dado | Carrega | Ref |
|---|---|---|
| `player_track` (~1Hz) | à frente/atrás do jogador (`ahead_idx`/`behind_idx`) + `gap_ahead`/`gap_behind`, corrida toda | `race_monitor.rs:428` |
| `laps` (LapSnapshot) | posição + gap ao líder de TODOS por volta E a cada troca de posição | :387 |
| `collided_with_car_number` | quem bateu no jogador (culpado no mesmo ponto) | :305 |
| `crashes` / `peak_crash_score` / `PlayerIncidentMark` | severidade + pontos (contato=4/rodada=2/saída=1) + volta.fração | :258/:415 |
| `car_laps` / `cars_meta` | ritmo por carro + `car_number` (ponte pro driver_id) + grid | :455/:464 |
| `pit_stops` / `yellow_laps` / `weather` | máscaras de ruído | :532 |
| `player_car_idx` | índice do jogador | :506 |

---

## 3. O livro-razão de interações (por oponente)

Pós-corrida, para cada `car_idx` que resolve para um `driver_id` da carreira, monta-se
um ledger a partir do `RaceHistory` + `Attempt`:

- contatos (contagem, pior severidade, consequência)
- tempo-de-duelo (segundos roda a roda) e voltas-de-duelo (voltas distintas)
- trocas de posição (pêndulo)
- ultrapassagens (a favor/contra, com volta e posição em jogo)
- posição-alvo da briga (relevância)

Cada linha vira delta pros dois eixos (`historical`/`recent`) via `apply_rivalry_event`
+ um capítulo em `rivalry_episodes`.

---

## 4. Gates de ruído (o coração do "como é percebido")

Inferir emoção de telemetria exige separar briga de artefato. TODOS os sinais passam por:

1. **Same-fight = adjacência de POSIÇÃO** (não só proximidade de pista). Dois carros só
   "brigam" se estão adjacentes na ordem de corrida. Isso mata retardatário/quem está
   uma volta à frente aparecendo com gap pequeno no `player_track` — o gate decisivo.
2. **Verde só** — descarta frames em `yellow_laps` (safety car/amarela).
3. **Fora do box** — descarta janelas de `pit_stops` (de qualquer um dos dois ± buffer);
   gap despenca no pit, não é duelo.
4. **Relevância por posição** — briga por vitória/pódio/pontos pesa mais que por P18.
   Multiplicador: pódio (≈×1.6), zona de pontos (≈×1.1), resto (≈×0.6).
5. **Mérito (sustentação OU repetição)** — um blip não é rivalidade. Exige duração
   mínima no duelo OU o par voltar a brigar na corrida seguinte.
6. **Cap por corrida** — uma corrida sozinha não pode criar rivalidade "intensa"
   (mérito ao longo do tempo). Cap agregado por oponente/corrida (ex.: h≤10, r≤22).

---

## 5. Detecção por sinal (fórmulas — magnitudes a CALIBRAR)

### 5.1 Contato atribuído  → `RivalryType::Colisao`
- `Attempt.collided_with_car_number` → car_idx → driver_id.
- Severidade de `peak_crash_score` / `impact_severity` + consequência (DNF? posições
  perdidas em volta do lap do contato, pelo trace).
- Deltas espelham os tiers de `process_collisions_rivalry` (real, não simulado):
  crítico h7/r18 · DNF h5/r14 · major ou ≥3 pos perdidas h3/r10 · leve h2/r8.
- Gate: só em verde e contato real (pontos=4 / score acima do limiar).

### 5.2 Duelo prolongado  → `RivalryType::Pista`
- Amostras consecutivas do `player_track` com o MESMO oponente à frente/atrás,
  **posições adjacentes**, `|gap| < GAP_CLOSE (~1.0s)`, verde, fora de box.
- Acumula `battle_secs` (soma dos intervalos) e `battle_laps` (voltas distintas).
- Conta como duelo se `battle_secs ≥ MIN_DUEL_SECS (~45)` OU `battle_laps ≥ 3`.
- Delta cresce com a duração (recent-heavy: um duelo é mais "calor" que "memória"):
  base h2/r8 → até h5/r14, × multiplicador de relevância.

### 5.3 Pêndulo de posições  → `RivalryType::Pista`
- Contagem de trocas de ordem entre o par ao longo da corrida (do trace de `laps`,
  que já emite a cada troca de posição). Troca = o par inverteu a ordem.
- ≥2 trocas = duelo; cada troca soma (retornos decrescentes): ≈ r+3/h+1 por troca, cap.
- Reforça o MESMO par que 5.2 (ok — ambos somam na mesma rivalidade; o cap por corrida
  segura o total).

### 5.4 Ultrapassagem decisiva  → `RivalryType::Pista`
- Ultrapassagens por posição reconstruídas (lógica do `overtake_feed`, `ai_news.rs:411`).
- Peso por **atraso** (fração da corrida decorrida — voltas finais ×2) e por **posição
  em jogo** (entrar no pódio/vitória ≫ P18).
- Ser passado tarde por um pódio = calor alto (semente de revanche). Delta ≈ h3/r10 no
  caso decisivo; passe comum no meio do grid, pequeno.

### 5.5 Agregação
- Soma os 4 sinais por oponente → um `(h_delta, r_delta)` com cap por corrida.
- `tipo` da rivalidade (se nova): contato presente → Colisao; senão Pista.
- `apply_rivalry_event(...)` + capítulo `rivalry_episodes` com summary gerado
  (ex.: "brigaram 9 voltas por P3 em Interlagos; ele passou na última volta").

---

## 6. Identidade

- `car_number` (`cars_meta`) → `driver_id` pela ponte do roster (`project_iracing_ai_roster`).
- Só pilotos mapeados. Não-mapeado (humano/desconhecido) → ignorado nesta leva.
- Rivalidade é sempre jogador (`is_jogador`) ↔ driver da carreira.

---

## 7. Onde roda + flag

- Módulo novo `iracing_sdk/rivalry_perception.rs`: função quase pura
  `perceive_rivalries(&RaceHistory, &Attempt, &roster_map) -> Vec<PerceivedRivalry>`.
- Chamado no pipeline de IMPORT (`project_iracing_results_import`), depois de montar o
  `RaceResult` — NÃO ao vivo (o monitor ao vivo é pro overlay/bandeiras; criar
  rivalidade exige a corrida inteira e limpa).
- Flag `IRACER_TRACK_RIVALRY`.

---

## 8. Validação (não dá pra Monte Carlo — é dado real)

Como a percepção é INFERÊNCIA sobre corridas reais, a validação é por inspeção:
- **Explicador "por que essa rivalidade?"**: uma view de debug que mostra, por corrida,
  o livro-razão por oponente e os deltas gerados — pra o usuário conferir "o sistema
  percebeu a corrida como eu senti?".
- Ajuste dos limiares (`GAP_CLOSE`, `MIN_DUEL_SECS`, multiplicadores) por corridas
  reais gravadas, não por MC.

## 8b. Modo de calibração por IA-sonda (probe) — testar sem dirigir 20 min

Para calibrar sem o jogador ter que correr, a percepção é generalizada de "o jogador"
para um **`probe_car_idx` qualquer**; o jogador é só o probe PADRÃO. Roda-se uma
corrida de IA (jogador espectando ou andando de leve), salva-se, e roda-se o
explicador apontando para uma IA.

**Refino de arquitetura (adotar):** a fonte primária passa a ser o **trace de campo**
(`laps`, universal — funciona pra qualquer probe), e o `player_track` (~1Hz + contato)
vira **camada de enriquecimento** exclusiva do probe-jogador. Assim o MESMO código que
roda de verdade pro jogador é o que se calibra na IA; o jogador só ganha fidelidade
extra.

```
trace de campo (laps)  →  batalhas de QUALQUER probe (pêndulo, duelo, ultrapassagem)
   + (só se probe == jogador) player_track  →  duelo em segundos + contato atribuído
```

**Disponibilidade por sinal numa IA-sonda:**
- Pêndulo de posições ✅ (trocas de ordem do par, do trace)
- Ultrapassagem decisiva ✅ (mudança de ordem + volta/progresso)
- Duelo prolongado ✅ com ressalva — gap entre o par = diferença dos gaps-ao-líder, mas
  na resolução do trace (~1 amostra/volta), não ~1Hz. Mede-se duelo em **voltas
  adjacentes-e-coladas**, não segundos. Suficiente pra calibrar; paridade fina exigiria
  um snapshot periódico de campo (~1Hz) no sampler — mudança pequena, adiável.
- Contato atribuído ❌ só o jogador (`collided_with_car_number` = "quem bateu em mim").
  A IA-sonda calibra 3 dos 4 sinais; contato calibra-se dirigindo de verdade. Proxy
  possível (perda súbita de gap + adjacência + `ai_stopped`/`ai_offtrack` do
  `RaceEvent`) mas menos confiável — deixar pra depois.

**Fluxo:** monta corrida de IA (20 min) com monitor rodando → salva (`RaceHistory` com
trace de campo) → explicador com seletor de carro mostra o livro-razão da IA escolhida
contra o grid + os deltas que gerariam → ajusta limiares sem aplicar rivalidade.

Nota de escopo: isto é HARNESS de teste/calibração. O sistema shippado continua
centrado no jogador. (A generalização do probe abre a porta pra rivalidade IA-vs-IA no
mundo no futuro, mas é fora de escopo aqui.)

## 9. Riscos / atenções

- **`player_track` só enxerga o vizinho imediato do JOGADOR** — perfeito pra rivalidade
  centrada no jogador (que é o escopo), mas não vê brigas IA-vs-IA (não é o objetivo).
- **Adjacência de posição depende do trace estar limpo** — `CarIdxPosition` pisca 0
  (já tratado no monitor com "última posição válida"); a percepção deve usar a posição
  saneada, não a crua.
- **Cap por corrida é o freio anti-inflação** — sem ele, uma corrida caótica criaria
  uma rivalidade "intensa" na hora, matando o senso de mérito.

## 10. A ponte de aplicação (design — 2026-07-18)

Como os deltas percebidos deixam de ser inertes e viram estado real no motor de
rivalidade. **Descoberta-chave: metade da ponte já existe** no caminho de import.

### Já existe (reusar, não reconstruir)
- **Identidade `car_number → driver_id`**: o `by_number` já é montado no setup do
  import (`build_session_race_result`, `commands/iracing.rs:749`); e o `RaceResult`
  importado já vem com `pilot_id = driver_id` da carreira (matching sessão→carreira
  do roster).
- **Idempotência**: `persist_race_result_tx` (`commands/race.rs:995`) roda numa
  transação com guard (`status != Pendente` → aborta) — nunca persiste a mesma corrida
  duas vezes.
- **Championship rivalry**: `process_championship_rivalry` já roda no persist (standings,
  category-wide) — complementar à percepção (é briga de PONTOS, não de pista). MANTER.
- **Episódios**: `record_rivalry_episodes` (`commands/race.rs:3833`) já grava os
  capítulos lendo o estado do motor + posições finais. MANTER.

### O que falta (pequeno)
1. **Onde**: só no caminho de IMPORT (corrida real do iRacing), atrás de
   `IRACER_TRACK_RIVALRY`. Corridas simuladas offline seguem só com os processadores
   atuais (têm incidents da sim; não têm SDK).
2. **Probe = jogador**: `perceive_rivalries(history, history.player_car_idx, contact, params)`.
3. **Resolver identidade**: cada oponente do livro-razão → `car_number` (de `cars_meta`)
   → `driver_id` via o `by_number` que o import já tem. Não-mapeado → pula.
4. **Aplicar**: um `RivalryEvent` por oponente com os deltas AGREGADOS já capados
   (`historical_delta`, `recent_delta`) e o `tipo` (Colisao se houve contato, senão
   Pista) → `apply_rivalry_event(player_driver_id, opponent_driver_id, …)`.
5. **Idempotência**: aplicar DENTRO da transação guardada do persist (passar os deltas
   já resolvidos a `driver_id` para dentro de `persist_race_result_tx`), OU num passo
   guardado por `subsession_id`. Nunca duas vezes na mesma corrida.

### Reconciliação com o que já roda
- **`process_collisions_rivalry` no import → DESLIGAR** (recomendado): ele lê
  `flat_incidents` da SIM, e o `RaceResult` importado traz POSIÇÕES, não pares de
  colisão bilaterais — então é praticamente inerte no import, e mantê-lo junto só
  arrisca duplo-contagem do contato. A percepção (dado REAL do SDK) é a fonte de pista
  no import. No offline sim ele CONTINUA (é a fonte lá).
- **Episódios**: melhoria pequena — alimentar `record_rivalry_episodes` com o conjunto
  de contatos vindo da percepção (`collided_with_car_number`), pra os capítulos de
  contato rotularem "colisao" (hoje o `collided` dele vem de `flat_incidents`, vazio no
  import → cairia em "duelo"/"campeonato").

### Fronteira
A **seleção do Nemesis** (trocar o proxy posicional de `build_primary_rival_summary`)
lê o estado ACUMULADO do motor — é downstream desta ponte, na spec
`2026-07-18-player-rivalry-nemesis-design.md` (removido na limpeza de 11/08/2026, recuperável pelo histórico do git; o Nemesis está no ar na tabela `player_nemesis`). Esta ponte é o pré-requisito dela.
