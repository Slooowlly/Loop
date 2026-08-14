# Sistema de Quebra do Carro (falha mecânica em corrida)

> ⚠️ **Retrato histórico, conferido em 11/08/2026.** Este cabeçalho dizia "produção não
> construída", e a quebra está no ar: `car/breakdown.rs`, a tabela `race_breakdowns` e os
> comandos `get_breakdown_forecast` e `get_grid_breakdown_risk` registrados. O arquivo continua
> aqui porque `car/breakdown.rs` e `car/wear.rs` o citam como referência de design.
>
> **Leia como a intenção original, e não como estado do app.** O estado de hoje está no
> [DESIGN.md](../../DESIGN.md) §10.3.

> Modelo travado e calibrado por Monte Carlo (harness em `src-tauri/src/car/breakdown_sim.rs`).
> Construído EM CIMA do [Sistema de Nível do Carro](2026-07-17-car-level-system-design.md)
> e do [comando de chat de texto livre](../../../src-tauri/src/iracing_sdk/mod.rs)
> (`send_chat_text` → `!black`/`!dq` no iRacing ao vivo).

## 1. Objetivo e princípios

1. Uma peça mal-cuidada pode **falhar durante a corrida** — desde uma penalidade
   curta até um DNF. A falha é disparada no iRacing REAL via comando de admin.
2. A quebra é **consequência emergente da economia**, não um dado jogado por cima.
   O cérebro de manutenção já força os times pobres a `Stretch`/`Degrade` quando
   falta caixa (`car_maintenance.rs:264-278`). Esses carros entram na corrida com
   peças no limite → **são eles que quebram**. Time rico troca tudo e quase nunca
   quebra. Espelha o automobilismo de verdade.
3. **A sorte manda.** O modelo NÃO é determinístico: entrar na zona de perigo é
   uma roleta que se joga volta a volta. ~69% das quebras são por azar (não pela
   parede — ver §11).
4. **Cada quebra tem culpado** (motor, câmbio, etc.) → combustível narrativo para
   revista/debrief/"do mundo do grid". Vale nas duas corridas: a dirigida no iRacing e a
   simulada (§14 fase 7) gravam o mesmo desfecho em `race_breakdowns`.
5. O jogador **não gerencia manutenção** (o cérebro do time faz por ele). Logo a
   quebra do jogador é **telegrafada** no pré-corrida (§10) — contrato de justiça.

## 2. O sinal: o desgaste das peças JÁ é a confiabilidade

Não inventamos um "sistema de confiabilidade". Lemos o estado que o
[Sistema de Nível do Carro](2026-07-17-car-level-system-design.md) já mantém por peça:

| Sinal (já existe) | Significado | Leitura de quebra |
|---|---|---|
| `wear` 0.0→1.0+ | 0 = nova, 1.0 = fim da vida nominal, >1.0 = sobreuso | risco sobe perto/depois de 100% |
| `spent` | peça esticada, rodando no bônus emprestado | entra a corrida frágil |
| `durability` | motor/câmbio 3 corridas; eletrônica 6 | fragilidade intrínseca |
| decisão `Degrade` | time não trocou (sem caixa) | carrega peça >100% pra corrida |

## 3. Modelo de risco — por peça, volta a volta

No **verde**, para cada carro e cada peça, calcula-se o risco a partir do desgaste
que a peça **carrega ao largar**; durante a corrida o desgaste sobe por volta e o
risco é reavaliado.

### 3.1 A janela de perigo

> **Superado pelo redesign de 22/07/2026 §4.3.** O código em
> `src-tauri/src/car/breakdown.rs` roda hoje com `RISK_OPEN = 0.90` e
> `HARD_WALL = 1.20`, em **dois regimes**: em serviço `[0.90, 1.00)` com risco baixo
> e linear, sobreuso `[1.00, 1.20)` com risco quadrático. Os números desta seção e
> da §3.2 são o estado de 18/07/2026. A fonte de verdade são as constantes do
> módulo.

- **Abre a 95%** de desgaste. Abaixo disso a peça é **confiável** (risco = 0). Sem
  falhas "freak" com peça sadia — foi decisão explícita do design.
- **Parede a 105%.** Ao atingir/passar 105% a peça **acabou** (falha forçada,
  certa). Entre 100% e 105% o carro **aguenta** — passar de 100% não quebra sozinho.

### 3.2 A curva de risco por volta

```
se wear < 0.95:            risco_volta = 0
senão:
  t = (wear - 0.95) / (1.05 - 0.95)                    # 0 em 95%, 1 em 105%
  base = HAZARD_OPEN + (HAZARD_WALL - HAZARD_OPEN) * t # sobe rumo à parede
  risco_volta = base * fragilidade(peça) * GLOBAL      # (jogador não tem desconto — ver §8)
se wear >= 1.05:           falha FORÇADA nesta volta   # a parede
```

- `fragilidade(peça) = clamp(3 / durability, 0.5, 1.0)` — peça de vida curta falha
  mais. Pelas durabilidades de hoje: motor, câmbio, freios, suspensão e asa
  **dianteira** (3) = 1.0; asa **traseira** e laterais (4) = 0.75; chassi, assoalho e
  arrefecimento (5) = 0.6; eletrônica (6) = 0.5. As duas asas deixaram de ser iguais
  no redesign de 22/07/2026 §4.7: a dianteira ficou em 3, a traseira em 4.
- `GLOBAL` = botão único de calibração da taxa do grid (análogo ao
  `IRACER_SALARY_SHARE`).

### 3.3 Ruído de sorte no desgaste

Cada volta, o incremento de desgaste tem ruído **±30%** (volta "puxada" — zebra,
calor, tráfego — gasta mais). Isso põe sorte no **quando** a peça cruza os 95%.

### 3.4 Influência da PISTA (qual peça sofre) — IMPLEMENTADO

A pista inclina **qual** peça se desgasta mais, sem inflar a taxa total. O
`track_profile` dá a demanda P/H/A da pista; cada peça tem sua direção PHA
(`pha_per_level`). Uma peça **alinhada** com o que a pista cobra sofre mais:

```
mult_pista(peça) = clamp(1 + K · (align(peça) − média_align_das_11), 0.6, 1.5)   # K = 1.4
desgaste_volta(peça) *= mult_pista(peça)
```

- `align` = produto das direções PHA (peça × pista) centradas em 1/3.
- Subtrair a **média das 11 peças** centra os multiplicadores em ~1.0 → **redistribui
  sem inflar** (a peça de potência sobe *e* a de handling desce na mesma pista).
- Pista de potência → **motor/arrefecimento/câmbio**; técnica → **freios/suspensão/aero**.
- Validação (harness, frota realista): taxa total invariante (<2% entre neutro/power/
  handling); redistribuição forte (motor +48% e arrefecimento +58% na potência; freios
  +70% na técnica). ⚠️ Com **todas** as peças no fio (setup artificial) infla — validar
  sempre com frota de desgaste variado.
- `roll_race_breakdowns` recebe `track_pha`; o wiring passa `get_track_simulation_data(track_id)`.

### 3.5 Influência do CLIMA (chuva/calor) — IMPLEMENTADO

`weather_wear_mult(peça, wetness, temperatura)`, composto com o da pista no desgaste/volta:

- **🌧️ Chuva** (`wetness` 0–1, de Dry/Wet/HeavyRain ou do `TrackWetness` ao vivo):
  eletrônica `×(1 + w·0.60)` (curto/sensores); motor/arrefecimento `×(1 − w·0.25)`
  (a chuva **esfria**) → **~neutro no total** (redistribui pra eletrônica).
- **🔥 Calor** (`temperatura`, base 25°C → forno 42°C): motor/arrefecimento
  `×(1 + heat·0.40)`. **Escolha A: INFLA o total** — dia quente quebra mais motores,
  nada é aliviado. É o único tempero que muda a taxa total do grid.
- Validação (harness): chuva → eletrônica dobra (557→1123), motor cai, total ~igual;
  forno → motor +60% (1761→2812), total infla.
- `roll_race_breakdowns` recebe `weather = (wetness, temperatura)`; o wiring passa do
  `SimulationContext` (`.weather` → wetness, `.temperature`).
- **Upgrade futuro:** `wetness` **por volta** do timeline de chuva (arco dry→wet→dry) —
  o pré-roll já é volta-a-volta, só falta o threading.

## 4. Severidade → comando (tempo POR PEÇA)

Ao falhar, sorteia-se a severidade (distribuição **por peça** — §11). Falha na
**parede (105%)** sobe um degrau (leve→grave→DNF; a peça foi ao limite).

| Severidade | Comando no iRacing |
|---|---|
| **Leve** | `!black #N t` · `t` = faixa **da peça** (leve) |
| **Grave** | `!black #N t` · `t` = faixa **da peça** (grave) |
| **DNF** | `!dq #N` — **encerra a corrida do carro** |

O tempo `t` **depende da peça E da severidade** (não é genérico): um câmbio custa
mais tempo que uma asa; grave custa mais que leve. Faixas-base (segundos), antes do
pit crew:

| Peça | leve (s) | grave (s) |
|---|---|---|
| Câmbio | 6–9 | 14–20 |
| Motor | 6–9 | 13–19 |
| Chassi | 5–8 | 11–17 |
| Suspensão | 5–8 | 10–15 |
| Arrefecimento | 4–7 | 9–13 |
| Freios | 3–6 | 7–11 |
| Asa traseira | 3–5 | 6–9 |
| Asa dianteira | 2–5 | 5–8 |
| Assoalho | 2–4 | 5–8 |
| Sidepods | 2–4 | 4–6 |
| Eletrônica | 2–3 | 3–5 |

- **Pit crew** (`team.pit_crew_quality`, 0–100, de `pit_strategy.rs`) escala o tempo:
  fator `1.20×` (q0) → `1.00×` (q50) → `0.80×` (q100). Equipe boa perde menos.
- Retirada = **`!dq`** (NÃO `!remove`: o carro sumiria do nada; com `!dq` ele fica
  visível, marcado como desqualificado).
- **Nomenclatura por carro** (`PartType::display_name(category_id)`): Mazda MX-5 não
  tem asa → "Parachoque dianteiro/traseiro"; GT/Toyota/BMW → "Asa traseira". As
  demais peças têm nome fixo (Motor, Câmbio, Freios…). Usado na narrativa da quebra.

## 5. Reconciliação da escala de desgaste (Rota B) — a mudança na produção

**Problema:** hoje `wear.rs::advance_race` soma desgaste **por corrida em bloco**
(`+1/durability`, sem noção de volta nem de tamanho). O modelo de quebra precisa de
granularidade **por volta**. Havia duas rotas; adotamos a **B**.

### 5.1 A decisão

- **Rota A (DESCARTADA):** desacelerar o desgaste (peça dura ~2× mais corridas).
  Dá dominância de sorte, mas os times trocam peça na metade da frequência →
  `technical_investment_cost` cai → **exigiria re-tune de finança**. E o custo de
  peça JÁ é ligado à fatura (`maintain_team_car` em `commands/race.rs:2165`).
- **Rota B (ESCOLHIDA):** manter a cadência de troca de hoje; a sorte vem de um
  **risco por volta intenso** numa janela curta. **Finança intacta.**

### 5.2 A escala unificada

Uma **única** grandeza de desgaste alimenta economia E quebra:

```
desgaste_por_volta(peça) = 1 / (durability(peça) × REF_RACE_LAPS)      # REF_RACE_LAPS = 18
desgaste_da_corrida       = desgaste_por_volta × voltas_da_corrida
```

- `REF_RACE_LAPS = 18` ≈ tamanho típico de um sprint. Uma corrida de ~18 voltas
  soma exatamente `1/durability`. **`wear_per_lap = wear_per_race / REF_RACE_LAPS`**.

**⚠️ Onde o "tamanho importa" vale — descoberta da Fase 1:** a economia da IA é
abstrata (corridas simuladas, sem execução no iRacing). Ligar length-awareness NELA
faria os enduros gastarem mais peça → **quebraria a promessa "finança intacta"** da
Rota B (categorias de endurance passariam a custar mais). Como a carreira tem
`CalendarEntry.voltas`, isso é *possível*, mas é um **opt-in separado** (não Fase 1).

Resolução: **a economia da IA continua PLANA** (bloco por corrida, `wear_per_race`
inalterado → finança byte-idêntica), e o **"tamanho da corrida importa" é entregue na
corrida AO VIVO do jogador** — onde o passe por volta (§3) usa as **voltas reais da
sessão do iRacing**. Isso satisfaz os dois desejos: finança intacta E tamanho
importando onde o jogador de fato vê.

### 5.3 Mudanças concretas no código

1. **[FASE 1 — FEITO]** `car/wear.rs`: `REF_RACE_LAPS = 18` + `wear_per_lap(pt) =
   wear_per_race(pt) / REF_RACE_LAPS`. `wear_per_race` **inalterado** (`1/durability`,
   byte-idêntico). Testes de regressão: `wear_per_race_permanece_um_sobre_durabilidade`
   + os 18 testes de `car_maintenance` verdes → economia não muda.
2. **[Fase 2]** O passe de **risco por volta** (§3) é uma passada SEPARADA, só na
   corrida AO VIVO do jogador, que soma `wear_per_lap × voltas_reais_da_sessão` a
   partir do desgaste que cada carro carrega da economia. **NÃO** altera `advance_race`
   nem `needs_decision` (a economia fica intacta).
3. **[Opt-in futuro, NÃO Fase 1]** Se um dia se quiser que a economia da IA reflita o
   tamanho da corrida (enduro gasta mais peça pra atrito de IA em sim puro — §12), aí
   sim `advance_race`/`needs_decision` passariam a receber `voltas` (via
   `race_entry.voltas`, já disponível no persist). Custa re-validar a finança de
   endurance.

## 6. Enduro e a manutenção em box (gap 2)

Com desgaste ciente do tamanho, o enduro castiga muito mais (sprint ~7% DNF →
enduro até 45% sem parada). É **realista na direção**, mas precisa da manutenção em
box que os times fazem de verdade:

- Corrida longa (≥ ~30 voltas) faz **paradas em intervalos** que zeram as peças
  gastas (troca no pit). A quebra do enduro é **inteiramente governada** por essa
  política — varredura no harness demonstrou o intervalo **0% ↔ 45%**.
- Os parâmetros da parada (intervalo, piso de troca) e a **estratégia de pit** (rica
  troca mais, pobre economiza) se cravam no **wiring**, junto das regras reais de
  pit-stop da sessão. O mecanismo está provado; o número final é um dial.

## 7. Determinismo controlado

A rolagem usa RNG de verdade (sorte), mas **semeada por corrida** para ser
**reproduzível e testável** — re-simular a mesma corrida dá o mesmo desfecho, e não
re-rola sozinho a cada leitura. A semente combina `(time/piloto, race_id, peça,
volta)`. No jogo, a sorte não é previsível pelo jogador (não há como "ler a seed").

## 8. Quem quebra: tiers e o jogador

Cada carro tem uma **diligência de manutenção** = probabilidade de trocar, antes da
corrida, uma peça que entraria na zona de perigo. É a raiz de por que o pobre quebra
mais.

| Tier | Diligência | DNF/corrida (sprint) |
|---|---|---|
| Rico | 0.96 | ~2.6% |
| Médio | 0.88 | ~7.3% |
| Pobre | 0.70 | ~17.3% |
| Jogador em time pobre | 0.90 | ~6.3% |

**Regra do jogador:** ele **herda o tier do time**. Em time rico/médio, as chances
são **idênticas às da IA** daquele tier. Só quando está num **time pobre** ele é
protegido — e a proteção vem de **manutenção melhor** (diligência mais alta, o
engenheiro é mais cuidadoso), **não** de desconto mágico no risco. Isso reduz TODAS
as quebras dele (inclusive as da parede), diferente de um multiplicador que só
mexeria na sorte-na-janela.

## 9. Timing e wiring ao vivo

1. **Verde:** para o grid da corrida do jogador, roda o sorteio (§3) por carro/peça
   → decide quem quebra, qual peça, em que **volta/tempo** (mais desgaste → tende a
   falhar mais cedo) e a severidade.
2. **Durante a corrida:** o `race_monitor` (`iracing_sdk/race_monitor.rs`) já
   acompanha a sessão ao vivo. Ao atingir a volta/tempo-alvo de um evento, dispara
   o comando via `iracing_sdk::send_chat_text` (`!black #N t` ou `!dq #N`).
3. **Mapa carro→nº:** o `roster_gen` já associa nosso piloto/time ao `#N` da sessão
   do iRacing — é como sabemos qual `#N` desqualificar.
4. **Foco:** `send_chat_text` foca a janela do sim antes de digitar (fullscreen
   exclusivo pode bloquear — testar borderless).

## 10. Aviso pré-corrida do jogador

Como o sorteio é determinístico-por-corrida (§7), computa-se no **pré-corrida (Sala
de Estratégia)**. Se o carro do jogador vai quebrar, o engenheiro avisa a **peça
específica**: *"motor no limite, risco real de falha hoje"*. É o mesmo sorteio que
dispara ao vivo — o contrato de justiça do "com aviso".

## 11. Parâmetros calibrados e dials

Valores que bateram os alvos no MC (Rota B). Todos calibráveis.

| Parâmetro | Valor | Papel |
|---|---|---|
| `REF_RACE_LAPS` | 18 | ancora desgaste/volta à cadência da economia |
| `RISK_OPEN` | 0.95 (hoje **0.90**) | abre a janela de perigo |
| `HARD_WALL` | 1.05 (hoje **1.20**) | parede (falha forçada) |
| `HAZARD_OPEN` | 0.05 (hoje `HAZARD_SERVICE_LO` **0.006**) | risco/volta na abertura da janela |
| `HAZARD_WALL` | 0.28 (hoje **0.45**) | risco/volta junto à parede |
| — | hoje `HAZARD_SERVICE_HI` **0.030** | risco/volta no fim da vida nominal (100%) |
| `WEAR_NOISE` | ±0.30 | ruído de sorte no desgaste/volta |
| `GLOBAL` | 1.0 | botão único da taxa do grid |
| Fragilidade | `clamp(3/durability, 0.5, 1.0)` | peça de vida curta falha mais |
| Diligência | Rico .96 / Médio .88 / Pobre .70 / Jogador-pobre .90 | quem quebra mais |
| Severidade (leve, grave) por peça | ver harness `severity_weights` | resto = DNF |
| Enduro pit-service | intervalo + piso (dial) | TBD no wiring c/ pit real |

**Severidade por peça** (prob. leve, grave; resto DNF), suavizada:

| Peça | leve | grave | DNF |
|---|---|---|---|
| Motor | .20 | .42 | .38 |
| Câmbio | .22 | .44 | .34 |
| Suspensão | .28 | .47 | .25 |
| Chassi | .25 | .45 | .30 |
| Freios | .45 | .45 | .10 |
| Cooling | .50 | .42 | .08 |
| Asa diant./tras. | .60 | .35 | .05 |
| Underbody | .72 | .25 | .03 |
| Sidepods | .78 | .20 | .02 |
| Eletrônica | .72 | .25 | .03 |

## 12. Resultados do MC (validação)

Rota B, sprint, 48 mil corridas/tier (harness `breakdown_sim::report`):

| Tier | DNF/corr | Alvo | Bate? |
|---|---|---|---|
| Rico | 2.6% | 2-3% | ✅ |
| Médio | 7.3% | 5-7% | ✅ |
| Pobre | 17.3% | 15-30% | ✅ |
| Jogador (time pobre) | 6.3% | 5-7% | ✅ |

- **Sorte 69% / parede 31%** — a sorte manda.
- **11 peças participam** (frágeis ~12% cada, duráveis ~5%).
- Desgaste na falha concentrado em **97–105%** (nada abaixo de 95%).

## 13. O harness (como re-calibrar)

`src-tauri/src/car/breakdown_sim.rs` (`#[cfg(test)] mod breakdown_sim`, throwaway,
fora do build de produção). Rodar:

```
cd src-tauri
CARGO_TARGET_DIR=<fora do OneDrive> cargo test -p loop breakdown_sim -- --nocapture
```

Ajustar os `Params`/diligência e rodar de novo. Reporta taxa por tier, chance por
peça, condição na falha, severidade, rastros de "porquê", varredura do `GLOBAL` e o
efeito do tamanho da corrida.

## 14. Fases de implementação

1. **Escala unificada** (§5.3): `advance_race` por volta e ciente do tamanho +
   `needs_decision` por voltas. Testes de regressão da economia/finança (garantir
   cadência ≈ hoje).
2. **Cérebro da quebra** (puro, testável): risco por peça/volta, severidade,
   sorteio de volta-alvo, semeadura por corrida (§3, §7). Portar os `Params`
   calibrados do harness.
3. **Wiring ao vivo** (§9): verde → sorteio → `race_monitor` dispara
   `send_chat_text`; mapa carro→`#N` via `roster_gen`.
4. **Aviso pré-corrida** (§10): engenheiro avisa a peça na Sala de Estratégia.
5. **Enduro pit-service** (§6): política de parada + estratégia de pit por tier.
6. **Narrativa** (fase futura): quebra alimenta debrief/revista/"do mundo do grid".
7. **[FEITO] Quebra na corrida SIMULADA** — o que era "atrito da IA, opcional" virou a
   fonte de quebra de TODA corrida não-dirigida (todas as categorias, não só as outras).
   `commands::race::preroll_simulated_breakdowns` pré-rola o grid inteiro com os MESMOS
   inputs do disparo ao vivo (desgaste real do `team_car`, pit crew, pista, clima da
   etapa via `iracing::race_breakdown_weather` — agora fonte única dos 3 consumidores).
   Os desfechos entram em `simulate_race_with_breakdowns` ANTES de `build_race_results`,
   então posição/gap/pontos já saem coerentes: `repair_secs_to_score` é o inverso exato de
   `RACE_SCORE_TO_LAP_MS`, e N segundos no box viram N segundos de corrida (medidos contra
   o líder). DNF marca `dnf_reason` com a frase da peça + `dnf_catalog_id` do catálogo
   `Mechanical`. O resultado persiste em `race_breakdowns` (tela/notícia) e volta pra
   economia via `team_breakdowns` (§4.6) — mesmo laço da corrida ao vivo.
   - Carro já fora por batida antes da volta da quebra NÃO registra (`applied_mechanicals`).
   - Rascunho histórico fica de fora (grid sintético, sem `team_car` pra ler).
   - **FONTE ÚNICA de pane mecânica.** Onde a quebra roda, a pane genérica do catálogo de
     incidentes (`roll_mechanical`, `MECHANICAL_BASE_CHANCE = 0.015` → ~1,3% DNF/carro/
     corrida) é DESLIGADA. Ela sorteava sobre a `confiabilidade` abstrata da equipe sem saber
     que peças o carro tem: não nomeava culpado, não danificava nada e não conversava com a
     economia. Manter as duas dobraria o abandono mecânico sobre as taxas calibradas do §12 e
     deixaria carro de peça nova fundindo motor **sem** o aviso pré-corrida do §10.
     - O chaveamento é o `Option` de `simulate_race_with_breakdowns`: `Some(..)` (mesmo vazio)
       = quebra no comando, catálogo fora; `None` = quebra não rodou, catálogo é a fonte. O
       pré-roll devolve `None` só quando NENHUM carro do grid pôde ser lido — save antigo
       nunca fica sem nenhuma fonte de falha mecânica.
     - `roll_mechanical` continua sendo CHAMADO e o resultado descartado: ele consome do RNG,
       e pular a chamada deslocaria o fluxo, mudando todos os erros de piloto e batidas
       seguintes. Assim a única coisa que sai da corrida é a pane.
     - As frases `Mechanical` do catálogo seguem em uso na corrida IMPORTADA do iRacing (a
       ponte não passa por aqui) e no rascunho histórico.

## 15. Assumidos e questões em aberto

- ⚠️ `REF_RACE_LAPS = 18` assume sprint típico de ~18 voltas. Se o calendário real
  divergir muito, re-ancorar (afeta a cadência de troca vs finança).
- ⚠️ Sintaxe exata de `!dq` a confirmar na pista (como fizemos com `!black`).
- ⚠️ Estratégia de pit do enduro (§6) é o maior dial em aberto — depende das regras
  reais de pit-stop da sessão do iRacing.
- ⚠️ A volta-alvo do evento vs tempo-alvo: o `race_monitor` acompanha ambos; decidir
  qual usar por robustez (voltas variam com bandeiras).
