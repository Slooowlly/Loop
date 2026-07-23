# Redesign do ciclo de vida e da falha das peças

**Data:** 2026-07-22
**Status:** proposta de design (aguardando aprovação do usuário)
**Escopo:** o comportamento das 11 peças — individuação, desgaste, risco de falha, timing de troca e o acoplamento entre a quebra ao vivo e o estado persistido. **Fora de escopo:** calibração de custo/finança (economia). Decisão do usuário: "a economia foda-se, a gente pensa nela depois; primeiro resolver a questão das peças."

Este documento SUPERSEDE, no que toca ao modelo de falha, as decisões de `2026-07-21-breakdown-balance-decisions-design.md` (que não chegaram a ser implementadas) e a separação estrita "desgaste econômico × risco ao vivo" de `2026-07-18-car-breakdown-system.md`. A infraestrutura de disparo (director, monitor, forecast, overlay, narrativa) é PRESERVADA — muda o **modelo interno**, um campo persistido novo e um **feedback pós-corrida**.

---

## 1. Problemas medidos (esta sessão)

Três testes diagnóstico rodaram sobre o pipeline REAL e expuseram os defeitos:

- `breakdown::analise_profunda_quebras` — frequência + quebras seguidas na mesma corrida.
- `car_maintenance::analise_recorrencia_entre_corridas` — recorrência da mesma peça entre corridas.
- `car_maintenance::analise_desgaste_por_peca` — trajetória de desgaste peça a peça.

| # | Defeito | Evidência |
|---|---|---|
| **P1** | **Lockstep** — peças de mesma durabilidade desgastam idênticas e chegam ao fim JUNTAS. | Neutro: as 6 peças durab-3 marcham 33→67→troca em sincronia; até 11 peças cruzam a zona na MESMA corrida. |
| **P2** | **Asa dianteira ≡ traseira** — mesma durabilidade E mesmo PHA `(0.22,2.44,1.22)` → nunca divergem, quebram sempre juntas. | Colunas `AsD`/`AsT` bit a bit iguais em toda a tabela. |
| **P3** | **Piso estrutural de DNF mesmo no time rico (~27%/corrida).** | `needs_decision` só troca quando `wear+wear_per_race≥1.0`, então TODA peça roda sua corrida final entrando em ~limiar (durab-3 = 0.667) e limiar+1 corrida cai na zona [0.87,1.0]. |
| **P4** | **Penhasco, não rampa** — orçamento parcial não ajuda. | Caixas de 5e4 a 4e5 deram idênticos ao pobre (47% base, 58% DNF). |
| **P5** | **Runaway do pobre** — degradar nunca zera o wear → peça mora além da parede → falha FORÇADA toda corrida, pra sempre, na MESMA peça. | Pobre: 88,6% das quebras forçadas; recorrência 65% (razão 1,4×). |
| **P6** | **Multi-quebra na mesma corrida alta** (P(≥2\|≥1) 80-90%). | Consequência direta de P1. |
| **P7** | **Corrida > 18 voltas explode** — modelo por-volta escala com voltas, mas a troca assume a referência de 18. | 30 voltas: rico_saudavel 0%→46% de quebra. |
| **P8** | **Desacoplamento** — a peça que quebra na pista zera SÓ na simulação (descartado); o save só muda por `maintain_team_car`. A quebra não tem consequência física no estado. | "Recorrência" é 100% história de orçamento, não da quebra. |
| **P9** | **Ruído só ao vivo** — `WEAR_NOISE ±30%` existe na sim viva, mas o desgaste PERSISTIDO é determinístico. | Fonte de P1: nada separa peças iguais. |

A raiz comum: **as peças não têm individualidade.** Todas nascem iguais (`Car::uniform` → wear 0), gastam de forma determinística e são geridas por uma regra de troca cega ao tamanho da corrida e ao desfecho da quebra.

---

## 2. Princípios de design

1. **Cada peça é um indivíduo.** Duas peças do mesmo tipo, instaladas em momentos diferentes ou em times diferentes, têm vidas diferentes. Um câmbio pode ser "bom" e durar mais, outro pode ser um "limão".
2. **A quebra é consequência, não dado.** Time bom/rico → carro confiável (falha rara, "azar"). Time pobre → peça esticada/sobreusada → falha crescente e certa. Isto é a promessa original do sistema, hoje violada por P3.
3. **Dois regimes claros de falha.** *Em serviço* (dentro da vida): risco BAIXO de azar. *Em sobreuso* (além da vida): risco que escala rápido rumo à certeza. A fronteira é 100% da vida da peça.
4. **A quebra tem consequência física.** Um desfecho grave/abandono muda o estado da peça no save. O jogo nunca narra uma quebra que "não aconteceu de verdade".
5. **Tamanho da corrida é neutro para a gestão.** Uma corrida de qualquer distância não deve surpreender o cérebro de manutenção; ele planeja sabendo as voltas reais da próxima etapa.
6. **As 11 peças têm CARÁTER distinto** — não só taxa de desgaste, mas *como* falham (motor funde/abandona; sidepod só custa tempo) e *o que* estressa cada uma (pista/clima).
7. **Determinístico e testável.** Toda a individualidade vem de sementes; mesma entrada → mesmo desfecho (o disparo ao vivo e a previsão continuam coincidindo).

---

## 3. Espaço completo de variáveis

O modelo precisa acomodar, de forma coerente, TODAS estas variáveis:

**Intrínsecas da peça (tipo):** durabilidade base · fragilidade · distribuição de severidade (leve/grave/DNF) · faixas de tempo de conserto · perfil PHA (estresse por pista) · se é estrutural (pode abandonar) · modos de falha (narrativa).

**Identidade da unidade instalada (NOVO, persistido):** `unit_seed` → deriva a *escala de vida individual* (`life_scale`) e o *caráter da unidade*. Re-rolada a cada instalação.

**Estado dinâmico (persistido):** wear (fração da vida INDIVIDUAL) · nível · `spent` (esticada) · `unit_seed`.

**Contexto da corrida:** voltas/duração (sprint vs enduro) · demanda PHA da pista · clima (chuva, temperatura, umidade, vento) · rampa de fim (enduro) · paradas de serviço.

**Time:** confiabilidade de engenharia (do orçamento/qualidade + pit crew) · decisões de manutenção (trocar/esticar/degradar/upgrade) · horizonte de planejamento · DNA/foco · jogador vs IA · estilo de pilotagem (só jogador).

**Constantes do modelo:** `REF_LAPS` · `LIFE_VAR` · `RISK_OPEN` · `OVERUSE` · `WALL` · curva de hazard (2 regimes) · escalas de enduro · pesos de severidade.

---

## 4. O modelo unificado

### 4.1 Identidade da unidade (`unit_seed`, `life_scale`) — resolve P1, P2, P9

`CarPart` ganha `unit_seed: u32` (serde `#[serde(default)]` para saves antigos → 0 = fallback determinístico pelo tipo).

- Em **qualquer instalação** (seed inicial, `Replace`, fim do `Stretch`): `unit_seed = splitmix32(prev_seed ^ team_hash ^ part_tag ^ install_counter)`.
- Dela deriva a **escala de vida individual**:
  ```
  life_scale = 1 + LIFE_VAR * (2*hash01(unit_seed) - 1)     // ∈ [1-LIFE_VAR, 1+LIFE_VAR]
  vida_individual(peça) = durability(peça) * life_scale
  ```
- O **desgaste vira fração da vida INDIVIDUAL**: uma peça com `life_scale=0.85` gasta mais rápido (limão), uma com `1.15` dura mais.

Com `LIFE_VAR = 0.18`, uma peça durab-3 tem vida 2,46–3,54 corridas. Duas peças durab-3 instaladas no mesmo momento têm `unit_seed` distintos → vidas distintas → **atingem o fim em corridas diferentes**. O lockstep morre já na primeira volta de vida, sem depender de calendário variado (P1). Asa dianteira e traseira, tendo `unit_seed` próprios, deixam de ser clones (P2) — e ver §4.7 para diferenciá-las também no tipo. O ruído volta ao estado persistido de forma determinística (P9).

> **Por que não ruído por corrida?** Ruído por corrida re-randomiza o desgaste toda vez e some no agregado (não cria "esta peça é um limão"). A escala de vida por INSTALAÇÃO dá individualidade PERSISTENTE e coerente — a mesma peça se comporta consistente até ser trocada.

### 4.2 Confiabilidade do time — resolve o eixo "quebra = consequência"

Cada time tem uma **confiabilidade de engenharia** `rel ∈ [0,1]` derivada do que já existe: `rel = mix(quality_na_categoria, pit_crew_quality/100)`. Ela desloca a MÉDIA da `life_scale` na instalação:

```
life_scale = base_life_scale(unit_seed) * (1 + REL_LIFE_GAIN * (rel - 0.5) * 2)
```

Com `REL_LIFE_GAIN = 0.12`: time excelente (`rel=1`) → peças ~12% mais duráveis e mais confiáveis; time fraco (`rel=0`) → ~12% menos (mais limões). Isto substitui o ad-hoc `PLAYER_MAX_RELIEF`: a proteção do jogador em time fraco vira um caso particular (o `rel` do time dele) — coerente, sem exceção mágica, e a IA sente o mesmo eixo. **É aqui que a qualidade do time entra no risco**, não num desconto separado.

### 4.3 Curva de hazard em DOIS REGIMES — resolve P3

Seja `w` o desgaste normalizado pela vida individual. Hazard por **volta de referência** (depois escalado por volta real, §4.4):

```
w < RISK_OPEN            → 0                              (confiável)
RISK_OPEN ≤ w < OVERUSE  → lerp(H_SERVICE_LO, H_SERVICE_HI, t1)   t1 = (w-RISK_OPEN)/(OVERUSE-RISK_OPEN)
OVERUSE   ≤ w < WALL      → lerp(H_SERVICE_HI, H_WALL, t2^2)       t2 = (w-OVERUSE)/(WALL-OVERUSE)
w ≥ WALL                 → 1.0                            (falha forçada)
```

- **Regime EM SERVIÇO** `[RISK_OPEN, OVERUSE) = [0.90, 1.00)`: hazard BAIXO (`H_SERVICE_LO=0.006 → H_SERVICE_HI=0.030`). É o "azar" que atinge o carro rico: a peça que roda sua corrida final (entrando ~0.9) tem exposição pequena → falha rara, quase sempre LEVE/GRAVE. **Isto derruba o piso do rico de ~27% para a faixa-alvo de poucos %** (P3), sem precisar que o rico troque antes da hora.
- **Regime SOBREUSO** `[OVERUSE, WALL) = [1.00, 1.20)`: hazard sobe QUADRÁTICO até `H_WALL=0.45` na parede. É a consequência do pobre que estica/degrada além de 100%. Escala rápido → falha crescente e certa.
- **Parede** `WALL = 1.20`: falha forçada (subiu de 1.13 para dar mais corredor ao sobreuso).

`fragility(pt)` continua multiplicando o hazard (peça de vida curta falha mais dentro da janela). Como `w` já é fração da vida INDIVIDUAL, a curva é a mesma para todas as peças; a durabilidade e a `life_scale` entram pela velocidade com que `w` sobe.

### 4.4 Comprimento da corrida — resolve P7

O desgaste por volta permanece `vida_individual` ÷ voltas de referência, e uma corrida de `L` voltas consome `L/REF_LAPS` de vida. **A correção é tornar a TROCA ciente do tamanho** (§4.5): o cérebro planeja para as voltas reais da PRÓXIMA etapa. Assim uma corrida de 30 voltas NÃO pega o time de surpresa — ele troca peças que projetam ultrapassar `OVERUSE` naquela distância, e nenhuma peça entra numa corrida longa destinada a estourar a parede. O hazard em si não muda; só some a surpresa (P7).

### 4.5 Troca ciente do tamanho e do risco — resolve P3/P4/P7

`needs_decision` passa a olhar o desgaste PROJETADO ao fim da próxima corrida, com a distância e as condições reais:

```
wear_fim = wear + desgaste_esperado(peça, laps_proxima, condicoes)
precisa_trocar = spent || wear_fim ≥ REPLACE_CEIL
```

- `REPLACE_CEIL = 1.0` para todos (mantém a cadência de vida).
- **Times confiáveis (rico) trocam com folga:** um segundo limiar `PROACTIVE_CEIL = 0.90` — com caixa sobrando, o passe de upgrade/renovação também repõe peças que projetam cruzar `PROACTIVE_CEIL`, mantendo o carro rico majoritariamente ABAIXO da zona de risco. Isto transforma o penhasco (P4) numa RAMPA: quanto mais caixa, mais cedo troca, menos exposição — um gradiente real de confiabilidade por orçamento.
- Pobre sem caixa: não troca → `w` passa de 1.0 → entra no regime de sobreuso (falha crescente), como manda o princípio 2.

### 4.6 Acoplamento quebra → estado — resolve P5 e P8

Depois da corrida, os `BreakdownEvent` daquela peça (do director/live) são AUTORIDADE sobre a condição pós-corrida, ANTES do cérebro de manutenção:

| Severidade | Efeito físico na peça persistida |
|---|---|
| **Leve** | Continua com o wear que tinha (só perdeu rendimento na corrida). |
| **Grave** | Troca FORÇADA (mesmo sem caixa → a débito). Peça nova. |
| **DNF** | Troca FORÇADA (mesmo sem caixa → a débito). Peça nova (`unit_seed` re-rolado). |

> **Nota de implementação (2026-07-22):** foi escolhida a **variante simples** (Grave e DNF ambos forçam troca), não a graduada (Grave→`max(wear,OVERUSE)` deixando o pobre arrastar). Motivo MEDIDO no harness: a graduada deixava o runaway do pobre **intacto** (a maioria das quebras do pobre é Grave, não DNF; Grave→degrada→requebra a mesma peça). A variante simples derrubou a recorrência do pobre de razão 1,4× para 0,9× (mesma peça não mais pegajosa) e a % de falha forçada de 90% para 43%. O runaway vira DÍVIDA (o custo da troca forçada estoura o orçamento), não o mesmo defeito eterno.

Efeitos:
- **P8 resolvido:** a quebra tem consequência no save; o jogo não narra falha fantasma.
- **P5 resolvido:** um DNF destrói a peça → ela é NOVA na próxima → não repete a MESMA peça infinitamente. O pobre passa a ver peças DIFERENTES falhando conforme o carro inteiro degrada, e a espiral vira DÍVIDA (o `Replace` forçado a débito), que outros sistemas já tratam — não um loop idêntico e chato.
- A recorrência remanescente (peça Grave que o pobre não troca) é INTENCIONAL e física, não um artefato de parede.

> Fiação: os eventos já existem por carro (player = live; IA = pré-roll do director). `maintain_team_car` recebe `Vec<BreakdownEvent>` do time e aplica esta tabela antes de `decide_maintenance`. O `unit_seed` re-rola no `Replace`.

### 4.7 Caráter distinto por peça — resolve P2 e reforça o princípio 6

Além da `life_scale`, cada TIPO de peça tem caráter próprio já existente (severidade, tempo de conserto, PHA, fragilidade). Ajustes:

- **Asa dianteira × traseira:** hoje idênticas. Diferenciar o perfil PHA (a traseira pesa mais em estabilidade/retas, a dianteira em curva) e/ou a durabilidade em 1 — para que nunca sejam clones nem no tipo (P2). Valores finais na calibração.
- Revisar as 11 durabilidades para **espalhar** (hoje 6 peças em durab-3): p.ex. dar a algumas das durab-3 valores 3 vs 4, reduzindo o tamanho do maior grupo. Combinado com `life_scale`, elimina qualquer resquício de sincronia.
- Severidade passa a depender do REGIME onde falhou: falha EM SERVIÇO (azar) tende a Leve/Grave; falha em SOBREUSO/parede tende a Grave/DNF. Isto substitui o "parede sobe um degrau" por algo contínuo e coerente com os dois regimes.

---

### 4.8 Durabilidade por NÍVEL — curva-tenda (desempenho × confiabilidade)

O nível da peça deixa de ser "estritamente melhor". A durabilidade segue uma **tenda simétrica com pico no nível 5**: baixo nível é frágil por ser barato/mal-feito; alto nível é frágil por ser de ponta e altamente estressado (materiais exóticos, tolerâncias apertadas, roda mais quente — a peça de qualificação que é uma granada). O nível 5 é o ponto maduro, provado e confiável.

```
mult_nível(level):
  1→0.60  2→0.75  3→0.88  4→1.00  5→1.15  6→1.00  7→0.88  8→0.75  9→0.60  10→0.50
```

Entra como fator na vida individual:

```
vida_individual(peça) = durability_base(tipo) × mult_nível(level) × life_scale(unit_seed) × fator_confiabilidade_time
```

**Efeitos de jogo:**
- **Imposto de confiabilidade no topo:** o time rico que empurra pro nível 7-8 por RITMO paga em confiabilidade (peça chega à zona de risco mais cedo → mais trocas, mais exposição). Some com "rico domina em tudo" — equilibra a grade por dentro.
- **Nível 5 como jogada de valor:** ritmo suficiente + confiabilidade máxima + custo médio. Cria escolha real de nível, não corrida cega ao teto.
- **Narrativa do colapso:** numa categoria gerida (ex.: GT3), nível 1-2 só surge quando o time DEGRADOU até o fundo → "a sucata do time falido quebra" é coerente e desejável.

**Guarda obrigatória — categorias spec/entrada:** a curva NÃO se aplica quando o teto da categoria ≤ 2 (rookie/amador). Nessas, o nível baixo é a NORMA (spec, todos iguais, sem gestão), não sinal de sucata — usam `durability_base` puro. Sem isso, o carro do iniciante viveria de DNF (péssima primeira impressão). Onde há gestão (teto ≥ 3), a curva vale integral.

**Consequência para o cérebro de manutenção (trabalho relacionado, fase posterior):** hoje o cérebro empurra toda peça ao teto. Com a tenda, subir além do 5-6 troca confiabilidade por ritmo — o cérebro deve passar a PESAR isso (alguns times/DNA preferem o 5-6 confiável; outros aceitam o 8 frágil por pace). Não bloqueia o mecanismo da peça; é otimização de decisão para depois.

## 5. Como cada variável entra (mapa)

| Variável | Onde atua no modelo |
|---|---|
| Durabilidade base (tipo) | Velocidade de subida de `w` (vida em corridas). |
| Nível da peça | `mult_nível` — curva-tenda pico no 5 (§4.8); desempenho × confiabilidade. |
| `life_scale` (unit_seed) | Individualiza a vida; desloca `RISK_OPEN`/`OVERUSE` em corridas reais. |
| Confiabilidade do time | Média da `life_scale` na instalação (§4.2). |
| Fragilidade | Multiplica o hazard na janela. |
| Pista (PHA) × clima | `condition_mult` no desgaste por volta (inalterado). |
| Voltas/duração | Desgaste por corrida + troca ciente do tamanho (§4.5). |
| Enduro | Rampa de fim + severidade abrandada (mantidos como modificadores sobre a curva). |
| Orçamento/manutenção | `REPLACE_CEIL`/`PROACTIVE_CEIL` (rampa de confiabilidade). |
| Quebra (evento) | Feedback físico pós-corrida (§4.6). |
| Estilo do jogador | Multiplica o desgaste por peça (inalterado). |
| Crash (batida) | Adiciona wear → entra na mesma curva (inalterado). |

---

## 6. Parâmetros propostos (primeiro corte, calibrar no harness)

| Constante | Valor | Papel |
|---|---:|---|
| `REF_LAPS` | 18 | Ancora vida↔voltas (inalterado). |
| `LIFE_VAR` | 0.18 | Variância individual de vida (±18%). |
| `LEVEL_DURABILITY` | tenda 0.60→1.15→0.50 | Mult de vida por nível, pico no 5 (§4.8); só teto ≥ 3. |
| `REL_LIFE_GAIN` | 0.12 | Ganho de vida por confiabilidade do time. |
| `RISK_OPEN` | 0.90 | Abre o regime em-serviço. |
| `OVERUSE` | 1.00 | Fronteira serviço→sobreuso. |
| `WALL` | 1.20 | Falha forçada. |
| `H_SERVICE_LO` / `H_SERVICE_HI` | 0.006 / 0.030 | Hazard por volta-ref no regime em-serviço. |
| `H_WALL` | 0.45 | Hazard por volta-ref junto à parede. |
| `REPLACE_CEIL` | 1.00 | Limiar de troca obrigatória (projetado). |
| `PROACTIVE_CEIL` | 0.90 | Troca proativa do time com caixa. |

Enduro (`ENDURO_*`), fragilidade, severidade por peça e tempos de conserto: manter os atuais como ponto de partida, re-calibrar após os dials acima.

---

## 7. Persistência e migração

- `CarPart` ganha `unit_seed: u32` com `#[serde(default)]`. Saves antigos: `0` → o modelo usa um fallback determinístico por `(team, part_type)` no primeiro carregamento e re-rola na primeira instalação. **Sem migração de schema destrutiva** (o carro é blob serializado; o default cobre).
- Nenhuma coluna nova de DB se o carro é persistido como JSON em `team_car`; confirmar o formato no wiring.

---

## 8. Correção de cada erro (rastreabilidade)

| Erro | Resolvido por |
|---|---|
| P1 Lockstep | §4.1 `life_scale` individual + §4.7 espalhar durabilidades. |
| P2 Asa D≡T | §4.1 `unit_seed` distinto + §4.7 diferenciar tipo. |
| P3 Piso do rico | §4.3 regime em-serviço de hazard baixo + §4.5 troca proativa. |
| P4 Penhasco | §4.5 `PROACTIVE_CEIL` cria a rampa de confiabilidade. |
| P5 Runaway do pobre | §4.6 DNF destrói e re-instala (vira dívida, não loop). |
| P6 Multi-quebra | Some junto com P1 (peças dessincronizadas). |
| P7 Corrida longa | §4.4/§4.5 troca ciente do tamanho. |
| P8 Desacoplamento | §4.6 feedback físico da quebra. |
| P9 Ruído só ao vivo | §4.1 individualidade persistente por instalação. |

---

## 9. Critérios de aceitação

**Testes determinísticos:**
- `life_scale` varia por `unit_seed` e re-rola na instalação; time confiável → média maior.
- Curva de hazard: 0 abaixo de `RISK_OPEN`; baixa e crescente em serviço; quadrática em sobreuso; forçada na parede.
- Feedback: Leve mantém wear; Grave → `wear≥OVERUSE`; DNF → peça nova (`wear=0`, `unit_seed` novo, `Replace` a débito se sem caixa).
- Troca ciente do tamanho: peça que projeta cruzar `REPLACE_CEIL` em 30 voltas é trocada antes.
- Asa dianteira e traseira NÃO idênticas (tipo e unidade).
- `mult_nível`: pico no 5 (1.15), 4/6 = 1.00, cai simétrico até 1/9; categoria spec (teto ≤ 2) ignora a curva (usa `durability_base`).
- **Tradeoff mensurável:** peça nível 8 quebra mais que nível 5 no mesmo carro/uso (a mesma peça, só o nível muda).

**Harness Monte Carlo (frota REALISTA escalonada, não só perfis sintéticos):**
- **Rico, temporada neutra:** DNF/corrida ≤ 5% (era ~27%); recorrência da mesma peça razão ≤ 1,2×.
- **Rico, clima brutal:** quebra relevante 5–10%, DNF ≤ 3%.
- **Pobre, temporada neutra:** DNF/corrida 20–35%, peças DIFERENTES ao longo da temporada (razão de recorrência ≤ 1,3×; sem loop na mesma peça); dívida cresce.
- **Anti-lockstep:** em calendário neutro, o nº de peças de MESMA durabilidade cruzando a zona na mesma corrida ≤ 2 na esmagadora maioria das corridas (co-ocorrência quebrada).
- **Comprimento:** rico_saudavel em 30 voltas → DNF ≤ 2% (era 46%).
- **Multi-quebra:** P(≥2 quebras \| ≥1) cai para faixa branda (alvo < 40% na frota realista).

Os 3 testes diagnóstico desta sessão viram a base do harness de validação (já existem, `#[ignore]`).

---

## 10. Plano de implementação (fases, uma de cada vez)

1. **Identidade da unidade** — `unit_seed` em `CarPart`, `life_scale`, wear normalizado pela vida individual. Só isso já mata P1/P2/P9. Medir com `analise_desgaste_por_peca`.
2. **Curva de dois regimes** — reescrever `per_lap_hazard` + `RISK_OPEN/OVERUSE/WALL`. Medir P3 com `analise_profunda_quebras`.
3. **Confiabilidade do time** — `rel` na instalação; aposenta `PLAYER_MAX_RELIEF`.
4. **Troca ciente do tamanho + proativa** — `needs_decision` projetado; `PROACTIVE_CEIL`. Medir P4/P7.
5. **Feedback físico da quebra** — threading dos eventos para `maintain_team_car`; tabela §4.6. Medir P5/P8 com `analise_recorrencia_entre_corridas`.
6. **Caráter por peça** — diferenciar asa D/T, espalhar durabilidades, severidade por regime, **curva-tenda de durabilidade por nível (§4.8)** com a guarda de categoria spec.
7. **Calibração final** — varredura dos dials no harness até bater todos os critérios da §9.

Cada fase compila e passa a suíte antes da seguinte.

---

## 11. Fora de escopo (adiado, por decisão do usuário)

- Recalibração de custo/preço de peça e de orçamento (a "economia").
- Balanceamento fino de finança/dívida do time pobre (só garantir que o DNF force `Replace` a débito).
- Novas superfícies de UI/narrativa além das existentes (o feedback físico só alimenta as que já leem o evento).
- Estratégia automática nova de pit para a IA.
