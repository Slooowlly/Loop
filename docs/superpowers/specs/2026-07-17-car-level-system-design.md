# Sistema de Nível do Carro (motor de peças, desgaste e economia)

> ⚠️ **Retrato histórico, conferido em 11/08/2026.** Este cabeçalho dizia "não implementado",
> e o sistema está no ar: `car/parts.rs`, `car/wear.rs`, `car/cost.rs`, `car/seed.rs`,
> `car/sim_bridge.rs` e a tabela `team_car`. O arquivo continua aqui porque **8 pontos do Rust
> em produção o citam como referência de design** (`car/mod.rs`, `car/parts.rs`, `car/cost.rs`,
> `car/seed.rs`, `car/sim_bridge.rs`, `car/wear.rs`, `db/queries/team_car.rs`,
> `market/car_maintenance.rs`, `simulation/car_build.rs`).
>
> **Leia como a intenção original, e não como estado do app.** O estado de hoje está no
> [DESIGN.md](../../DESIGN.md) §10.3.

> Inspirado no modelo de peças do GPRO (Grand Prix Racing Online), adaptado às categorias do
> Loop. Export pro iRacing ficou para uma fase futura (ver §11).

## 1. Objetivo e princípios

1. O carro passa a ser um **cidadão de primeira classe** também no lado econômico:
   nasce das peças, evolui **a cada corrida**, e é dirigido pela economia do time.
2. O **jogador não investe, não escolhe, não vê o shape**. Ele só enxerga o
   **Nível do Carro (1–10)**. Todo o resto é decisão do **cérebro do time**.
3. **Nível domina; shape é a cereja.** Um carro de nível alto **nunca** perde pra
   um de nível baixo — exceto em pista focada exclusivamente num único atributo.
4. O sistema **substitui** o dropdown "balanced / aceleração / potência / handling"
   da aba My Team. O conceito continua existindo por baixo (emergente das peças),
   mas some da cara do usuário.
5. Os soquetes já existem na simulação (§8); a maior parte do trabalho é alimentar
   esses soquetes a partir do novo motor de peças.

## 2. O carro é um vetor

- **Magnitude** = o quão bom o carro é no total → alimenta `car_performance`.
- **Direção (shape)** = como o total se divide entre **P** (Power), **H** (Handling)
  e **A** (Acceleration) → alimenta o delta de casamento com a pista.
- **Nível do Carro (1–10, VISÍVEL)** = a leitura resumida da magnitude (ver §4).

Consequência-chave: **dois carros "nível 7" podem ser feras diferentes** por baixo
(um empilhou motor→P, outro asas→H). O usuário não distingue; os times sim.

## 3. As 11 peças

Cada peça tem: um **nível 1–10**, uma **durabilidade** (em corridas), um **custo-base**
(relativo — ver §6) e um **viés PHA** (quanto contribui pra cada atributo no nível máximo).

| Peça | Durab. (corridas) | Custo-base rel.¹ | Viés P | Viés H | Viés A | Papel |
|---|---|---|---|---|---|---|
| Motor (Engine) | 3 | 7.29 | 52 | 5 | 19 | Power puro |
| Câmbio (Gearbox) | 3 | 6.82 | 29 | 6 | 37 | Aceleração + Power |
| Asa diant. (Front wing) | 3 | 3.41 | 2 | 22 | 11 | Handling |
| Asa tras. (Rear wing) | 3 | 3.31 | 2 | 22 | 11 | Handling |
| Chassis (Telaio) | 5 | 2.84 | 7 | 16 | 13 | Handling all-round |
| Suspensão (Sospensioni) | 3 | 2.60 | 0 | 14 | 11 | Handling / Accel |
| Eletrônica (Elettronica) | 6 | 2.06 | 13 | 0 | 13 | Power + Accel |
| Freios (Freni) | 3 | 1.53 | 0 | 18 | 0 | Handling puro |
| Underbody (Fondo piatto) | 5 | 1.12 | 2 | 11 | 5 | Handling-lean |
| Sidepods (Fiancate) | 4 | 1.01 | 3 | 6 | 0 | Handling leve |
| Cooling (Raffreddamento) | 5 | 1.00 | 11 | 0 | 2 | Power leve |

¹ Custo-base relativo (Cooling = 1.00), extraído das proporções do GPRO. Os
**valores absolutos** não são os oficiais; serão reescalados por categoria (§6).

**A tensão central:** as peças que dão P/A (Motor, Câmbio) são as **mais caras E de
vida mais curta** (3 corridas). As baratas e duráveis (Eletrônica 6, Chassis 5,
Cooling 5, Underbody 5) puxam mais Handling. Investir em Power/Accel custa caro e
recorre rápido → é a decisão estratégica que separa os times.

### Viés PHA por atributo (resumo)

- **P**: Motor >> Câmbio > Eletrônica > Cooling
- **H**: Asas > Freios > Chassis > Suspensão > Underbody
- **A**: Câmbio >> Motor > Eletrônica > Chassis

## 4. Nível do Carro (1–10)

**Definição (assumida):** `Nível do Carro = round(média aritmética dos níveis das 11 peças)`,
clampado em `[1, 10]`.

- É a **magnitude exibida**. Legível e estável.
- Por baixo, a simulação usa o **total de PHA contínuo** (não o inteiro 1–10) pra
  decidir a corrida, evitando degraus artificiais.
- Rookie é **spec puro**: todas as peças no nível 1 → Nível do Carro sempre 1, sem
  gestão de peças, sem shape (carro idêntico pra todos).

> ⚠️ Assumido. Alternativa considerada: `(Σ pontos PHA) / 11` bucketizado — descartada
> porque Motor/Câmbio dominariam o número (dão 3–5× mais pontos que as outras).

## 5. Desgaste e ciclo de vida da peça

Desgaste vai de **0% (nova)** a **100% (fim da vida nominal)**. A durabilidade define
quantas corridas até 100% (ex.: Motor gasta ~33%/corrida; Eletrônica ~17%/corrida).

No fim da vida, o time tem **3 saídas** por peça:

| Saída | Custo | Efeito |
|---|---|---|
| **Trocar** (peça nova) | Cheio (§6) | Peça zera o desgaste, mantém/sobe o nível |
| **Esticar** | Reduzido | Roda **+1 corrida**, depois **morre** (troca obrigatória) |
| **Degradar** | $0 | Peça **cai de nível** → Nível do Carro cai |

### Regra do "esticar" (assumida)

- Só é habilitada se, na hora da decisão, o desgaste da peça estiver **≤ 95%**.
  Passou disso → só trocar ou degradar.
- Custo = **~40% do preço de uma peça nova** do mesmo nível (barato, mas não grátis).
- Concede **exatamente +1 corrida**; ao fim dela a peça está morta e a troca é obrigatória.
- **PUNIÇÃO DO SOBREUSO (implementada):** ao repor uma peça esticada, o time só pode
  comprar uma peça **um nível ABAIXO** (peça nível 4 esticada → só dá pra comprar nível 3;
  fica impossibilitado de comprar 4 ou 5). Sem isso, esticar seria sempre grátis e todo
  time ficaria forçando peça pra sempre — a punição faz esticar ser um trade-off real
  (ganha 1 corrida no nível atual, mas perde um nível na reposição obrigatória).
- Uso típico: *"não tenho caixa OU a próxima pista exige muito desse atributo — então
  estico por uma corrida em vez de deixar o nível cair bem agora."*

### Regra do "degradar" (assumida)

- Cada corrida rodada **acima de 100%** sem trocar/esticar → a peça **cai 1 nível**
  (contribuição PHA e base de custo caem junto), até ser reposta.
- Rodar muito degradado aumenta risco de falha (fase futura — hoje só derruba o nível).

> ⚠️ Os números (95%, 40%, −1 nível/corrida) são pontos de partida a calibrar.

## 6. Curva de custo por categoria (o teto suave)

```
custo(cat, peça, nível) = base_peça(cat) · 1,2385^(nível−1) · parede(nível, teto_cat)
```

- **`base_peça(cat)`** — custo-base da peça reescalado **por categoria** (mantém as
  proporções relativas da tabela §3, mas os absolutos batem com a economia da
  categoria). É o que faz `nível 5 no mazda = X` e `nível 5 no gt3 = Y`.
- **`1,2385^(nível−1)`** — crescimento geométrico normal **abaixo** do teto (+23,85%/nível).
- **`parede(nível, teto)`** — = 1 até o teto. **Acima**, o incremento por nível é
  `23,85% + 35%·(níveis acima do teto)`. Ou seja: 1º acima = +59%, 2º = +94%,
  3º = +129%… compõe e vira um muro. **Não é cap rígido** — é dor crescente.

### Tetos suaves por categoria

| Categoria | Escada | Teto suave | `car_weight_scale` atual |
|---|---|---|---|
| rookie | 1 | **1** (spec) | 0.15 |
| amador | 2 | **2** | 0.65 |
| production_challenger | 3 | **4** | 1.40 ⚠️ |
| bmw_m2 | 4 | **3** | 0.80 |
| gt4 | 5 | **6** | 1.00 |
| gt3 | 6 | **7** | 1.30 |
| lmp2 / endurance | 7 | **8** | 1.60 |

Alcance global **1–10** — o teto do LMP2 (8) deixa 9–10 só pros ultra-ricos que
decidirem sangrar dinheiro.

### Exemplo (peça tipo "motor" no amador, teto 2, base ilustrativa $100k)

| Nível | Custo | Δ | Situação |
|---|---|---|---|
| 1 | $100k | — | |
| 2 | $124k | +24% | no teto |
| 3 | $197k | +59% | 1 acima → parede |
| 4 | $382k | +94% | |
| 5 | $874k | +129% | ~9× o nível-2, **a cada 3 corridas** → inviável |

No GT3 (teto 7), o mesmo nível 5 fica *abaixo* do teto → crescimento manso, apesar da
base absoluta ser maior. Valores **e sustentabilidades** completamente diferentes.

### Ancoragem no orçamento (calibração, não número mágico)

`base_peça(cat)` é setado pra que **sustentar o nível-teto custe ~35% do orçamento
típico da categoria** (recorrente, ao longo da temporada, considerando durabilidades).
Assim o teto **emerge do caixa** e fica acoplado ao sistema de finanças já calibrado.

**Os dois mecanismos agem juntos:** a **parede** define a *forma* da dor; o **orçamento**
define *onde cada time para*. Um GT3 rico crava 7; um GT3 pobre trava em 4–5 — e é esse
spread **dentro** da categoria que faz o carro decidir corridas.

## 7. Cérebro de estratégia do time

Um brain por time (evolução de [`market/car_build_strategy.rs`](../../../src-tauri/src/market/car_build_strategy.rs))
decide, a cada corrida, para cada peça: trocar / esticar / degradar — dentro do
orçamento e olhando o calendário à frente. **O jogador não participa; seu time roda
no mesmo brain, limitado pelo seu caixa.**

### Horizonte de planejamento (traço por time, varia por temporada)

| Horizonte | Comportamento |
|---|---|
| Temporada inteira | Planeja picos pras pistas importantes, empilha peças duráveis, suaviza gasto |
| 5 corridas | Antecipa a próxima "onda" de pistas |
| 3 corridas | Reage à janela curta |
| 1 pista | Míope: compra só pra próxima, é pego pelo calendário, desperdiça |

- **Distribuição base:** 20% temporada / 30% cinco-corridas / 30% três-corridas / 20% míope.
- **Varia por temporada:** o horizonte é re-rolado/deriva a cada temporada (um ano o time
  tem bom diretor de estratégia, no outro perde). Futuro: virar atributo de staff.
- **Interação com o calendário 9D escalonado:** um time de horizonte longo vê que as
  próximas 3 são pistas de Power e investe motor/câmbio; o míope chega com o shape errado
  numa pista peaked — exatamente quando o shape decide (§9).

> Opcional a decidir: tilt por prestígio (times grandes tendem a horizonte mais longo).

## 8. Integração com a simulação (soquetes existentes)

- **Magnitude (total PHA)** → `car_performance` — [`math.rs:63`](../../../src-tauri/src/simulation/math.rs)
  (`category_car_performance`) e `normalize_car_performance`.
- **Shape (vetor P,H,A)** → substitui o `CarBuildProfile` discreto pelo vetor contínuo
  que já é `CarAttributeWeights` — [`car_build.rs:5`](../../../src-tauri/src/simulation/car_build.rs).
  O enum `CarBuildProfile` e o dropdown **saem**.
- **Peso por categoria** (`car_weight_scale`: rookie 0.15 → endurance 1.60) **fica** —
  continua decidindo o quanto o carro importa em cada divisão.

## 9. Regra de dominância

```
desempenho = magnitude(nível) + bônus_de_shape(pista)
```

O **bônus de shape é clampado por quão "peaked" é a pista**:

- Pista equilibrada (~33/33/33): bônus ≈ 0 → **só o nível decide** → nível 8 sempre
  bate nível 6. ✅
- Pista de ponto único (ex.: 90/5/5): o clamp **abre**, e um nível-6 especializado no
  eixo certo *pode* furar a fila de um nível-8 generalista. ✅ (a única exceção permitida.)

Implementação: o teto do `track_delta` (hoje ±6 fixo — [`car_build.rs:127`](../../../src-tauri/src/simulation/car_build.rs))
passa a ser **função da peakiness da pista** (perto de 0 em pista equilibrada, largo em
pista mono-tema). Garantia numérica: em pista normal, o clamp fica **< metade do gap
típico de nível** entre carros adjacentes.

## 10. Jogador passivo + UI My Team

- Remover o seletor de perfil de carro (balanced/accel/…) da aba My Team.
- Exibir só o **Nível do Carro (1–10)** do time do jogador, subindo/descendo por corrida
  conforme caixa e calendário. (Opcional futuro: mini-barras P/H/A discretas.)
- Amarra com o dossiê financeiro real ([project_my_team_real_numbers]).

## 11. Fase futura: export pro iRacing

Confirmado o desenho (a implementar depois): como no iRacing todo mundo corre o **mesmo
carro spec**, o nível do carro do jogador **não** vira potência na pista — ele **modula a
dificuldade da IA** no export:

```
driverSkill_IA = tier_base + track_offset + adaptativo
               + contribuição_do_carro_da_IA
               − vantagem_do_carro_do_jogador
```

Carro bom → IA relativamente mais fácil; carro ruim → IA mais dura. É pra isso que a
margem até 125 foi reservada no redesign de skill.

## 12. Decisões pendentes / assumidas (a confirmar na revisão)

1. Nível exibido = média dos níveis das peças (§4). **Assumido.**
2. Esticar: ≤95%, custo ~40% de nova, +1 corrida, morte obrigatória (§5). **Assumido nos números.**
3. Degradar: −1 nível por corrida acima de 100% (§5). **Assumido.**
4. Ancoragem do orçamento em ~35% pra sustentar o teto (§6). **Assumido.**
5. Tilt de horizonte por prestígio (§7). **Em aberto.**

## 13. Reconciliações no código existente

- **`car_weight_scale` da production** está em **1.40** (mais alto que GT3), mas a
  production agora é categoria de baixo prestígio (escada 3, teto 4). Revisar se o peso
  do carro dela deve ser tão alto.
- **`CarBuildProfile`** (enum) e helpers `weights_for_profile` / `profile_*_cost` /
  `track_advantage` — migrar de perfil discreto pra vetor PHA contínuo emergente.
- **Fatura de manutenção** ([project_car_maintenance]) hoje é informativa; passa a
  carregar a **depreciação real** das peças.
- **`ROOKIE_SPEC_CAR_PERFORMANCE`** ([`math.rs:53`](../../../src-tauri/src/simulation/math.rs))
  continua válido (rookie spec puro).
