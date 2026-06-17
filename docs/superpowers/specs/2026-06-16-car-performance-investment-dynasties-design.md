# Design: Rebalanceamento de Performance do Carro, Investimento e Dinastias

**Data:** 2026-06-16
**Status:** Proposta (design — nenhum código alterado ainda)
**Autor:** Slooowlly + Claude

---

## 1. Contexto e evidência (análise do save atual)

Análise do save `career_003` (temporada 29/2028, 102 equipes, última temporada cheia = 28):

- **Carro decide o rookie.** Correlação `car_performance × posição final` na temporada 28:
  - Toyota Rookie **−0.87**, Mazda Rookie **−0.79**, BMW M2 **−0.78**, Mazda Amador **−0.74**.
  - GT3 −0.50, GT4 −0.37 (moderado).
  - **Production e Endurance: ~0 (saturado).**
- **Saturação no topo.** `car_performance` tem teto rígido em **16.0**. **39/102 equipes (38%)** estão no teto; Production e Endurance estão **100% maxados**, GT3 5/14. No topo o carro **não diferencia** — é só piloto + sorte.
- **Sem dinastias.** Títulos parecem aleatórios; não há separação estrutural entre equipes de elite e o resto.
- **Economia "boa demais":** 60% das equipes elite/healthy, **zero dívida no grid inteiro**, caixa mediana de ~600k (rookie) a ~59M (endurance LMP2). O ciclo `caixa → +carro no offseason (até +1.4/temp) → prêmio → caixa` satura sem freio.

### Problema central
1. No rookie, o carro promove **carro em vez de talento** — pilotos ruins sobem por estarem em carros bons.
2. No topo, a saturação **anulou** o impacto do carro, justamente onde ele deveria ser máximo.
3. O teto rígido impede dinastias e torna os títulos aleatórios.

---

## 2. Objetivos

1. **Rookie:** impacto do carro o **mais baixo possível** — peças padrão (spec) + peso baixo na simulação. Talento decide.
2. **Topo:** impacto do carro **maior** — peso alto + spread grande entre carros.
3. **Investimento ilimitado com retornos decrescentes** — sem teto rígido; equipes muito ricas investem mais, mas cada ponto custa mais.
4. **Estratégia de longo prazo (3 temporadas) por equipe** — o jogador não controla o time (só troca de time); cada equipe opera segundo um arco estratégico de 3 temporadas.
5. **Dinastias com sustos** — ancoradas em **3 equipes elite por classe premium** (Production de cada carro; Endurance de cada classe), gerando guerra direta entre elas e separando a elite do meio do grid.

---

## 3. Sistema atual (referência precisa)

| Componente | Local | Comportamento atual |
|---|---|---|
| Escala de `car_performance` | `models/team.rs:163,169-172` | clamp **[−5.0, 16.0]**; seed = base do template ±2.0 |
| Normalização p/ simulação | `simulation/math.rs:28-30` | `(cp+5)/21*100`, satura em cp=16 → 100 |
| Pesos do carro na corrida | `simulation/race.rs:440-493` | **fixos, iguais p/ toda categoria**: carro 0.20 (Start/Late/Finish), 0.30 (Early/Mid) |
| Aplicação do carro no score | `simulation/race.rs:495-510` | `normalize_car_performance(cp) * weights.car_performance` |
| Pesos do carro na quali | `simulation/qualifying.rs:26-48` | carro 0.18–0.27 por caráter de pista |
| Ganho de carro no offseason | `finance/cashflow.rs:131-168` | `Δcp = (caixa·0.55 − dívida·0.65 + viés_estado + viés_estratégia + breakthrough) · eficiência`, clamp **±1.4** |
| Estratégia da equipe | campo `season_strategy` (teams) | **1 temporada**: balanced / expansion / all_in / austerity / survival |
| Custo técnico (manter carro) | `commands/race.rs:101-103` | `base·0.16 + cp·base·0.015` (16% fixo + 1.5%/ponto) |

**Limitações que viabilizam o problema:** pesos do carro não conhecem a categoria; normalização satura em 16; ganho clampado em ±1.4 e cp clampado em 16 → saturação garantida; estratégia é só de 1 temporada (sem arcos plurianuais); nada designa equipes de elite.

---

## 4. Design proposto

Quatro pilares. Números são **pontos de partida para tuning**, não finais.

### Pilar A — Impacto do carro por categoria (peso + variância)

**A1. Peso do carro por categoria (na simulação).**
Introduzir um multiplicador `car_weight_scale(category)` aplicado ao peso de `car_performance` em `segment_weights` (corrida) e na quali. O **delta** retirado/adicionado ao peso do carro é **redistribuído proporcionalmente aos atributos de piloto** para manter a soma dos pesos = 1.0 (rookie: carro ↓ → piloto ↑).

| Categoria | `car_weight_scale` | Carro ≈ % do resultado |
|---|---|---|
| Rookie (mazda/toyota) | **0.15** | ~3–4% |
| Amador | 0.50 | ~12% |
| BMW M2 | 0.70 | ~16% |
| GT4 | 1.00 | ~23% (atual) |
| GT3 | 1.30 | ~30% |
| Production (todas) | 1.40 | ~32% |
| Endurance (gt3/gt4/lmp2) | 1.60 | ~37% |

Hook: passar a categoria (já disponível em `SimulationContext`) para `calculate_segment_score`/`segment_weights` e escalar o termo do carro.

**A2. Peças padrão / spec por categoria (variância do carro).**
O peso sozinho não basta: se os carros do rookie variam muito, mesmo com peso baixo ainda há viés. Solução: nas categorias spec o `car_performance` é **fixado num baseline da categoria com variância mínima** e **o investimento não compra carro** ali.

| Categoria | Regime | Spread de `car_performance` |
|---|---|---|
| Rookie | **Spec total** | baseline fixo da categoria, ±0.3 (carros ~idênticos) |
| Amador | Semi-spec | baseline ±2.0 |
| BMW M2 / GT4 | Aberto (moderado) | investimento gera spread moderado |
| GT3 / Production / Endurance | Aberto (amplo) | investimento gera spread grande (elite separa) |

Implementação: no world-gen e no offseason, categorias spec **ignoram** o update de investimento e re-fixam o carro no baseline. Resultado direto: a correlação carro×resultado no rookie cai para ~0.

> **Efeito combinado:** rookie = peso 0.15 + spec (sem spread) → carro praticamente irrelevante; talento + sorte decidem. Topo = peso 1.4–1.6 + spread amplo → carro decisivo e elite separada.

### Pilar B — Investimento ilimitado com retornos decrescentes (sem teto)

**B1. Remover o teto rígido** de `car_performance` (sem clamp em 16; manter um piso, ex. −5). O carro passa a poder crescer indefinidamente.

**B2. Custo marginal crescente (retornos decrescentes).**
Cada ponto de carro custa mais conforme o carro já é bom. Custo marginal no nível `cp`:

```
m(cp) = m0 · (1 + cp / K)        // K controla a curva; cp maior → ponto mais caro
```

Dado um orçamento de desenvolvimento `B` (vindo do plano estratégico + caixa), o ganho `Δcp` sai de integrar `m`:

```
B = m0 · (Δcp + (cp·Δcp + Δcp²/2) / K)   →   resolver p/ Δcp (raiz da quadrática)
```

Propriedades: **sem teto** (sempre dá pra subir), **decrescente** (dobrar `B` rende menos que o dobro), e **auto-equilibra** com o custo de manutenção (que já cresce com cp em `race.rs:101-103`). Uma equipe muito rica sobe mais alto que o resto, mas a um custo crescente → separa a elite sem runaway infinito.

**B3. Normalização assintótica (sem saturar a 16).**
Trocar `(cp+5)/21*100` por uma curva que **continua respondendo acima de 16** e nunca trava em 100, mantendo diferenciação entre carros de elite:

```
normalize_car_performance(cp) = 100 · (cp − FLOOR) / ((cp − FLOOR) + K_norm)
// FLOOR = −5; K_norm tunado p/ a faixa de elite (cp 15–35) ficar responsiva
```

Exemplo (K_norm=30): cp=8→27.9, cp=16→47.6, cp=25→60, cp=35→66.7. Diferenciação preservada bem além de 16 (hoje todos viram 100). O **peso por categoria** (Pilar A) é o dial primário de quanto isso importa.

> Migração: o save atual tem 39 carros pinados em 16.0. Após a mudança, eles continuam em 16 e voltam a divergir naturalmente nas próximas temporadas via investimento. (Ver §6.)

### Pilar C — Estratégia de longo prazo (3 temporadas)

O jogador não controla a equipe; cada equipe roda um **arco estratégico de 3 temporadas** que governa quanto do caixa vira desenvolvimento de carro (orçamento `B` do Pilar B) e o apetite a risco/dívida.

Novos campos em `teams`: `strategic_plan_type` (TEXT), `strategic_plan_remaining_years` (INT, 3→0).

Arquétipos de plano (exemplos):

| Plano | Investimento no carro | Risco | Quando a IA escolhe |
|---|---|---|---|
| **Title Push** | máximo sustentável por 3 temps (drena caixa) | alto | caixa forte + ambição de título |
| **Sustainable** | crescimento estável | baixo | meio de grid saudável |
| **Rebuild** | austeridade 1–2 temps p/ recuperar caixa, depois empurra | médio | saindo de crise/colapso |
| **Elite Dominance** | máximo permanente, com piso de recursos | alto | equipes elite (Pilar D) |

Ao chegar a 0, o plano é **re-avaliado** a partir do estado financeiro + status de elite. Arcos plurianuais criam **build-ups sustentados** → uma equipe em Title Push de 3 temporadas se separa → dinastia. Substitui/estende o `season_strategy` de 1 temporada.

### Pilar D — Equipes elite (3 por classe premium) → dinastias com sustos

**D1. Designar 3 equipes elite por classe premium (só Production e Endurance):**
- Production: 3 elites em **mazda**, 3 em **toyota**, 3 em **bmw**.
- Endurance: 3 elites em **gt3**, 3 em **gt4**, 3 em **lmp2**.
- GT3/GT4 standalone **não** têm elites (decisão §5).

Cada classe premium tem 6 equipes; **3 são elite**. Novo campo `teams.elite_tier` (INT: 0 = normal, 1 = elite).

**D2. Comportamento elite:** plano padrão **Elite Dominance** + um **piso de recursos** (patrocínio/caixa garantidos por tier) que sustenta investimento máximo no carro → as 3 elites mantêm `car_performance` claramente acima do meio do grid → **guerra direta** entre elas pelos títulos.

**D3. "Sustos":** entre as 3 elites, **qualidade de piloto + sorte + retornos decrescentes** (uma 4ª elite raramente alcança; uma elite em ano ruim de piloto tropeça) fazem o título **rodar entre as 3** com zebra ocasional do meio do grid. Não é determinístico.

**D4. Elite fixa + desafiante orgânico raro:** as 3 elites por classe são **fixas** (sem rotação programada). Porém o sistema de investimento (Pilar B) permite que, ocasionalmente, uma equipe **não-elite** acumule caixa e contrate um **piloto excepcional**, subindo o carro o suficiente pra rivalizar — formando uma **"4ª força" temporária**. As 3 principais seguem brigando no geral; o 4º competidor é exceção emergente, não regra.

---

## 5. Decisões (confirmadas 2026-06-16)

1. **Elites só em Production e Endurance** (class-split). GT3/GT4 standalone **não** recebem elites.
2. **Spec parts só no rookie.** Amador é aberto (sem semi-spec).
3. **Elite fixa**, mas com **desafiante orgânico raro**: ocasionalmente uma equipe de fora junta dinheiro + contrata um piloto incrível e rivaliza → forma uma "4ª força" temporária, enquanto as 3 principais seguem brigando no geral. (D4 vira: tier de elite fixo + caminho orgânico raro pra um 4º competidor, sem rotação programada das 3.)

**Ainda a calibrar (durante implementação, via simulação — não bloqueiam o Pilar A):**
4. Forma da curva de custo marginal (`K`) e de normalização (`K_norm`) — Pilar B.
5. Gap de recursos da elite vs meio do grid — Pilar D.

---

## 6. Modelo de dados e migração

**Novos campos em `teams`:**
- `strategic_plan_type TEXT` (default por estado financeiro)
- `strategic_plan_remaining_years INTEGER DEFAULT 0`
- `elite_tier INTEGER DEFAULT 0`

**Migração (nova versão de schema):**
- Adicionar colunas com defaults.
- **Seed das elites:** escolher 3 por classe premium (critério inicial: maior `reputacao` / `carreira_titulos` / `historico_pontos` da classe), setar `elite_tier=1` e `strategic_plan_type='Elite Dominance'`.
- **Remover o clamp 16** não exige migração de dados; os 39 carros pinados em 16 voltam a divergir organicamente.
- Demais equipes: `strategic_plan_type` inicial derivado do `financial_state` atual.

**Compatibilidade:** `season_strategy` pode coexistir (deriva do plano) durante a transição, ou ser substituído.

---

## 7. Plano de testes

### 7.1 Unitários (Rust, `#[test]`)
- **`normalize_car_performance` v2:** monotônica crescente; **não satura** dentro da faixa de design (cp=16 e cp=30 mapeiam para valores distintos); slope decrescente; piso em FLOOR.
- **Peso por categoria:** `car_weight_scale(rookie) << car_weight_scale(endurance)`; após redistribuição, **soma dos pesos do segmento = 1.0** em toda categoria.
- **Spec parts:** após world-gen + offseason, **variância de `car_performance` no rookie ≈ 0**; investimento não altera carro em categoria spec.
- **Curva de investimento:** **retornos decrescentes** (`Δcp(2B) < 2·Δcp(B)`); **sem teto** (`B` grande → cp > 16); custo marginal cresce com cp.
- **Plano de 3 temporadas:** `remaining` decrementa; re-avalia em 0; elite mantém `Elite Dominance`.
- **Seed de elite:** exatamente 3 `elite_tier=1` por classe premium após migração.

### 7.2 Estatísticos / simulação (integração — validam o balanceamento)
Harness que roda N temporadas simuladas e mede:
- **Talento no rookie:** `|corr(car_performance, posição_final)| < 0.15` **e** `corr(skill, posição_final)` alto. (Carro não decide; talento decide.)
- **Carro no topo:** `corr(car_performance, posição_final)` alto em Endurance/Production (faixa-alvo a definir).
- **Dinastia com sustos:** ao longo de M temporadas numa classe premium, **≥70% dos títulos** ficam com as 3 elites, **mas nenhuma equipe sozinha > X%** (ex.: 55%) — dinastia sim, domínio absoluto não.
- **Sem saturação:** após M temporadas, carros de elite **excedem 16** e mantêm **spread > limiar** entre si (não voltam a pinar todos juntos).
- **Promoção justa:** quem sobe do rookie correlaciona com **skill**, não com carro (verificar que a escada promove talento).

### 7.3 Não-regressão
- Suites existentes de `promotion::`, `simulation::`, `finance::` continuam verdes.
- Conservação econômica (sistema fechado) preservada.

---

## 8. Ordem de implementação sugerida

1. **Pilar A** (peso por categoria + spec) — **✅ IMPLEMENTADO (2026-06-16).** `car_weight_scale` + `category_car_performance` em [math.rs](../../../src-tauri/src/simulation/math.rs); aplicado no scoring de corrida ([race.rs](../../../src-tauri/src/simulation/race.rs) — peso redistribuído via `scale_segment_car_weight`, soma=1.0) e quali ([qualifying.rs](../../../src-tauri/src/simulation/qualifying.rs)). Testes: unitários em math.rs + comportamental `test_pilar_a_car_decides_at_top_not_rookie` (carro decide endurance, não rookie). Suíte completa verde (1486). Pesos: rookie 0.15 (+spec), amador 0.50, bmw 0.70, gt4 1.0, gt3 1.30, production 1.40, endurance 1.60.
2. **Pilar B** (normalização assintótica + custo marginal + remover teto) — desbloqueia o topo.
3. **Pilar C** (planos de 3 temporadas) — substitui `season_strategy`.
4. **Pilar D** (elites + piso de recursos) — depende de B e C.
5. Harness estatístico + tuning dos `K`/pesos/gaps com base nas métricas de §7.2.

Cada pilar entra com seus testes (§7.1) antes do próximo. O tuning final é guiado pelos testes estatísticos (§7.2).
