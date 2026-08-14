# Economia do Loop — inventário e redesign

Documento de trabalho. Parte 1 é o **inventário** do que existe hoje: todo sistema que
move, lê ou influencia dinheiro. Parte 2 é o **diagnóstico** do que está quebrado e por quê,
com número medido. Parte 3 é a **proposta** de um modelo novo, escrito do zero ao lado do
atual. Parte 4 são os **critérios de aceitação** e a costura de troca.

Contexto: a decisão de reescrever foi tomada depois que o harness de economia
(`commands::race::tests::medicao_financeira`) passou a reproduzir um save real — 34 de 36
células de validação dentro de 10%. Antes dele não havia como julgar uma economia; agora há.
Saves antigos **não** precisam ser preservados.

---

## Parte 1 — Inventário

### 1.1 As âncoras

Toda a economia sai de duas tabelas de constantes, e elas não conversam.

**`finance::planning::category_finance_scale`** — quatro números por categoria:

| Categoria | cash_min | cash_max | oper_min | oper_max |
|---|---:|---:|---:|---:|
| rookie (mazda/toyota) | 100 k | 700 k | 120 k | 250 k |
| amador (mazda/toyota) | 250 k | 1,5 M | 250 k | 600 k |
| bmw_m2 / production | 750 k | 4 M | 600 k | 1,6 M |
| gt4 | 2 M | 9 M | 1,5 M | 4 M |
| gt3 | 6 M | 25 M | 4 M | 12 M |
| lmp2 | 10 M | 45 M | 7 M | 20 M |
| endurance | 12 M | 60 M | 8 M | 25 M |

**`car::cost::category_cost_scale`** — uma segunda escala, escrita à mão, que ancora o preço
de peça: 120 / 280 / 715 / 1.800 / 5.200 / 8.800 / 10.700. Ela é declarada como "placeholder
calibrável" e nunca foi derivada da primeira.

Da primeira nasce o número mais consequente do sistema:

```
round_operating_base = operating_cost_midpoint / corridas_por_temporada
```

Quase toda linha de receita e de despesa é uma fração desse `base`.

### 1.2 Entradas de dinheiro

| # | Canal | Onde | Fórmula |
|---|---|---|---|
| 1 | Patrocínio | `race/financas.rs` | `0,27 × caixa_médio/rodadas + reputação×base×0,004 + budget_index×base×0,002 + fama×base×0,004` |
| 2 | Bilheteria | `cashflow::calculate_gate_income` | `base × 0,12 × (prestígio/60) × cota_do_time` |
| 3 | Bônus por resultado | `race/financas.rs` | `(pontos×0,042 + vitórias×0,26 + pódios×0,081 + top5×0,065) × base` |
| 4 | Prêmio parcial | `race/financas.rs` | `pontos × 0,0078 × base` |
| 5 | Prêmio de construtores | `finance/prize.rs` | `operacional × (0,15 + 0,50 × posição_relativa)`, por classe, no fim da temporada |
| 6 | Ajuda / paraquedas | `race/financas.rs` | `min(parachute_remaining, 25 000)` — **número absoluto** |
| 7 | Empréstimo de emergência | `finance/events.rs` | só em `collapse`; entra no caixa e vira dívida × 1,18 |
| 8 | Piso de recursos das elites | `finance/strategy.rs` | 3 equipes por classe premium têm o caixa **elevado ao caixa-médio da categoria** todo ano |
| 9 | Venda por colapso | `finance/rescue.rs` | dívida zerada + injeção de 45% do caixa-médio |
| 10 | Debug | `commands/career/debug.rs` | +8 M por categoria |

### 1.3 Saídas de dinheiro

| # | Linha | Onde | Fórmula |
|---|---|---|---|
| 1 | Salários | contratos ÷ rodadas | folha real dos contratos; alvo de projeto = 15% do operacional |
| 2 | Operação da etapa | `finance/operacao.rs` | 9 linhas, todas frações do `base` (detalhe em 1.4) |
| 3 | Manutenção estrutural | `race/financas.rs` | `base × 0,18 + engineering×base×0,0025 + pit_crew×base×0,0015` |
| 4 | Investimento técnico | `race/financas.rs` | `base × 0,16` + custo real de peça |
| 5 | Serviço da dívida | `finance/events.rs` | 0,75% (elite) a 5,0% (collapse) do saldo devedor **por rodada** |
| 6 | Amortização | `cashflow.rs` | 25% do caixa acima da reserva, todo evento |
| 7 | Peça: reposição / esticada | `car/wear.rs` + `car/cost.rs` | `part_cost(cat, peça, nível)`, +23,85%/nível, parede acima do teto |
| 8 | Peça: upgrade | `car/cost.rs` | `part_cost × upgrade_price_multiplier`, ancorado em 50% do operacional para o carro inteiro |
| 9 | Peça destruída | `car/crash.rs`, `car/breakdown.rs` | troca forçada a débito, mesmo sem caixa |

### 1.4 A fatura da etapa (`finance/operacao.rs`)

Nove linhas, cada uma um peso fixo do `base`, moduladas por fatores:

| Linha | Peso | Modulado por |
|---|---:|---|
| gasolina | 0,070 | voltas rodadas |
| pneus | 0,073 | desgaste final |
| peças | 0,041 | — |
| frete | 0,125 | viagem × instalações |
| viagem | 0,095 | viagem × instalações |
| estadia | 0,080 | viagem amortecida × instalações |
| inscrição | 0,045 | nada (âncora fixa) |
| diárias | 0,045 | pit crew × instalações |
| estrutura | 0,051 | instalações |

### 1.5 Quem LÊ dinheiro para decidir

Estes são os acoplamentos — o que quebra se a economia mudar de forma.

| Sistema | O que lê | Para quê |
|---|---|---|
| Cérebro de manutenção (`market/car_maintenance`) | `spending_power / 12` | orçamento de peça de UMA corrida |
| Mercado / salários (`finance/salary.rs`) | `spending_power`, reputação | teto salarial e valor da oferta |
| Assédio (`market/poaching.rs`) | `team_cash` | teto de leilão, taxa de rescisão |
| Estado financeiro (`finance/state.rs`) | caixa ÷ caixa-médio, spending_power, dívida | 6 faixas: elite → collapse |
| Estratégia e foco (`strategy.rs`, `focus.rs`) | estado financeiro | plano de temporada, foco da equipe |
| Offseason (`cashflow.rs`) | `cash_balance` | move confiabilidade, engineering, facilities — **sem debitar nada** |
| Fama (`fame.rs`) | `budget_index` (0–100) | fator de carência do time |
| Promoção (`promotion/effects.rs`) | — | credita paraquedas ao rebaixar |
| Dossiê / UI (`career_team_dossier/financas.rs`) | `team_finance_history` | gráfico de caixa, ledger, fôlego |

### 1.6 Fama (entrou no escopo)

A fama alimenta duas linhas de receita — o termo de patrocínio por fama do lineup
(`fama × base × 0,004`) e a cota de bilheteria — então ela é parte da economia, não vizinha
dela.

| Peça | Onde | Regra |
|---|---|---|
| Atributo | `drivers.midia` (0–100) | gerado em 26–74, com piso de 36; rookies em 20–55 |
| Presença da equipe | `public_presence/team.rs` | `melhor × 0,7 + segundo × 0,3` sobre a mídia da dupla |
| Ganho do JOGADOR | `race/financas.rs` | +3 vitória, +2 pódio, +1 top-5, −1 resto, −2 DNF, modulado por carisma |
| Ganho da IA | `event_interest/public_impact.rs` | +3 vencedor, +1,5 pole, +1 pódio, +1,5 incidente principal, lesão |
| Decaimento | `fame.rs` | **todo piloto, toda corrida**: 2,5% da distância até um piso **global de 25** |
| Valor comercial | `fame.rs` | 6 níveis convexos, pesados pela carência do time (`budget_index`) |

### 1.7 O ledger

`team_finance_history` grava, por equipe e por rodada, 11 linhas nomeadas + totais:
`sponsorship_income`, `gate_income`, `result_bonus`, `partial_prize_income`,
`constructor_prize_income`, `aid_income`, `salary_expense`, `event_operations_cost`,
`structural_maintenance_cost`, `technical_investment_cost`, `debt_service_cost`.
A linha de fechamento de temporada usa `round = 1000`. É a fonte da aba My Team.

---

## Parte 2 — Diagnóstico

### 2.1 Estoque e fluxo estão confundidos na raiz

`expected_cash_midpoint` (a média de `cash_min`/`cash_max`) é usado ao mesmo tempo como:

- **estoque**: "quanto caixa uma equipe desta categoria deveria ter" (score de saúde, piso de
  elite, injeção da venda, crédito disponível);
- **fluxo**: a base do patrocínio anual (`0,27 × esse número`).

São grandezas de dimensões diferentes. Confundi-las é o motivo de o patrocínio de uma
categoria escalar com a riqueza esperada dela em vez de escalar com o custo de operar nela — e
é a origem estrutural do descasamento entre categorias.

### 2.2 O calendário é um multiplicador escondido

`base = operacional_anual / rodadas`. Categoria com poucas rodadas tem base inflado por
rodada. Os prêmios por resultado são múltiplos do base, mas os pontos por corrida **não**
encolhem junto — no Endurance eles crescem (tabela própria 35-28-23 e pontuação por classe).

Prova medida: BMW e Production usam a **mesma** `category_finance_scale`, letra por letra.
BMW roda receita/despesa 1,07; Production roda 1,44. A única diferença é 8 rodadas com 10
equipes contra 10 rodadas com 18 equipes em 3 classes.

### 2.3 As linhas físicas têm magnitude de orçamento

O `base` da BMW é 137.500 por etapa. A fatura cobra:

| Linha | Valor por etapa (BMW) | Ordem de grandeza real |
|---|---:|---|
| gasolina | ~9.400 | corrida de 30 min num M2: ~40 litros |
| pneus | ~11.300 | 2 jogos de slick |
| inscrição | ~6.200 | plausível |

No Endurance o base é 2,75 M por etapa: a linha de **gasolina sozinha dá ~188.000 por
corrida**. Nenhum desses números tem relação com a coisa que o rótulo nomeia. São frações de
um orçamento abstrato vestindo nomes de objetos físicos — e o jogador lê o nome, não a fração.

Esse é o problema que o redesign precisa resolver de forma diferente: não recalibrando o peso,
e sim **invertendo o sentido da conta** (ver 3.3).

### 2.4 Circularidade: dinheiro → índice → receita → dinheiro

`budget` é um índice 0–100 derivado do dinheiro (`derive_budget_index_from_money`), gravado na
tabela a cada escrita (`sync_legacy_budget_index`), e depois **realimentado na receita**:
`patrocínio += budget_index × base × 0,002`. Equipe rica capta mais patrocínio por ser rica.
É um laço de reforço positivo sem amortecimento, e ele existe por acidente histórico: `budget`
é a coluna legada de antes de existir `cash_balance`.

### 2.5 Dinheiro que aparece do nada

Três mecanismos criam caixa sem contrapartida:

1. **Piso de recursos das elites** — as 3 melhores de cada classe premium têm o caixa
   *elevado* ao caixa-médio da categoria toda pré-temporada. Não é receita, é um `if cash <
   floor { cash = floor }`. Garante que o topo nunca empobreça.
2. **Venda por colapso** — no save real, a T101 acumulou **24,6 M de dívida**, foi vendida,
   teve a dívida perdoada e **recebeu 6,975 M de caixa**. Falir é lucrativo.
3. **Offseason gratuito** — `apply_offseason_competitiveness_impact` melhora confiabilidade,
   engineering e facilities proporcionalmente ao caixa, e **não debita nada**.

### 2.6 Não existe ralo

Fora o fluxo da rodada, os únicos débitos de caixa em todo o código são compra e reposição de
peça. Nada mais escala com a riqueza. Consequência medida na varredura: **nenhuma
configuração de receita mantém o caixa estável**. A melhor deriva alcançada em ~60
configurações foi 1,4× em 20 temporadas — e só com 29,5% do mundo em crise. Com os números de
hoje, 3,8× (Rookie chega a 8,5×).

### 2.7 Botões mortos e código órfão

| Item | Situação |
|---|---|
| `portao_piso` (GATE_FLOOR_WEIGHT) | varrer de 0,0 a 1,0 **não muda nada** — a fama é uniforme demais para a cota diferenciar |
| `BONUS_OVERALL_1ST/2ND/3RD` | declarados em `scoring.rs`, nunca usados |
| `car_performance` | ainda calculado no offseason; o comentário do próprio código diz que ninguém lê para ritmo |
| `spending_power` | grandeza ANUAL de 6 termos, consumida em 4 lugares com significados diferentes: orçamento de UMA corrida (÷12) no cérebro de peça, teto salarial, score de saúde e estratégia de pit |
| `aid_income` | `min(parachute, 25 000)` — único valor absoluto num sistema proporcional; no Endurance são 25 k contra uma rodada de 5,5 M |

### 2.8 A fama é um sistema que se auto-comprime

Por corrida, numa GT3 de 28 carros: **5 pilotos ganham** fama (vencedor, pole, dois pódios,
incidente principal) e **23 só decaem**. O decaimento é universal — 2,5% da distância até o
piso, toda corrida, para todo mundo. Numa temporada de 14 etapas isso fecha ~30% da distância
até 25.

O resultado é uma população achatada logo acima do piso. Medido no save:

| Categoria | mídia média | mín | máx |
|---|---:|---:|---:|
| Endurance | 47,6 | 25,9 | 83,8 |
| GT3 | 43,3 | 25,0 | 63,6 |
| BMW | 38,6 | 23,0 | 54,7 |
| Mazda Rookie | 38,1 | 21,0 | 52,8 |

Um novato tem 38 e um piloto de Endurance tem 47,6 — 25% de diferença ao longo da pirâmide
inteira. Como a presença da equipe é `melhor × 0,7 + segundo × 0,3`, todo grid tem presença
~40 ± 8, e a cota de bilheteria `0,5/N + 0,5 × (presença/total)` colapsa em `1/N` para
todos. **É por isso que `portao_piso` é um botão morto**: não há sobre o que ele variar.

Duas causas somadas: o piso do decaimento é uma constante global (um campeão decai para o
mesmo 25 de um anônimo) e o ganho é reservado a cinco pilotos por corrida.

### 2.9 O placar de hoje

Medido no save real (temporadas 28–31) e reproduzido pelo harness:

| Categoria | receita/despesa | Estado |
|---|---:|---|
| Rookie | 1,61 | imprime |
| Cup (amador) | 1,17 | folgada |
| BMW | 1,07 | no limite |
| Production | 1,44 | imprime |
| GT4 | 1,00 | no limite |
| **GT3** | **0,78** | **sangra** |
| Endurance | 1,45 | imprime |

A GT3 é a pior em **100% das ~60 configurações varridas**. Nenhum botão de receita muda a
ordem — todos escalam as categorias na mesma proporção. A causa da GT3 está na despesa: o
técnico é 31% da fatura dela e a manutenção medida dá **48% do custo operacional da
temporada**, contra uma âncora declarada de 41% no teto.

---

## Parte 3 — Proposta

### 3.1 Princípios

1. **Uma unidade de conta: dinheiro.** Nenhum índice 0–100 paralelo, nenhuma realimentação
   de riqueza em receita.
2. **Estoque e fluxo separados.** `orçamento_anual` (fluxo) e `caixa` (estoque) têm âncoras
   distintas e nunca se substituem.
3. **A forma da categoria entra explícita.** Número de etapas, tamanho do grid e classes são
   argumentos das fórmulas, não vazam por uma divisão.
4. **A fatura é bottom-up.** Quantidade física × preço unitário. O rótulo e o número contam
   a mesma história.
5. **Dinheiro do nada é evento narrado, não regra silenciosa.**
6. **Existe ralo.** O caixa tem equilíbrio, não integra para sempre.

### 3.2 A âncora nova

Um número por categoria: **`custo_operacional_anual_referencia`** — o que uma equipe mediana
gasta numa temporada, sem desenvolvimento de carro. Dele derivam somente as coisas que são
*de fato* proporcionais ao porte da operação (folha da equipe técnica, sede, seguro).

O caixa esperado deixa de ser uma constante de tabela e passa a ser uma **consequência**:
quantos meses de operação a equipe consegue bancar. `financial_state` mede isso em meses, não
em fração de um número inventado.

### 3.3 Despesa bottom-up

O ponto que resolve 2.3. Em vez de `gasolina = 7% do orçamento`, a conta passa a ser:

```
combustível  = voltas × km_por_volta × consumo_l_por_km × preço_do_litro × nº_de_carros
pneus        = jogos_usados × preço_do_jogo(categoria) × nº_de_carros
frete        = distância_até_a_pista × tarifa_por_km(porte_da_operação)
viagem       = pessoas × passagem(distância)
estadia      = pessoas × noites × diária
inscrição    = taxa da categoria (fixa, conhecida)
```

O `nº_de_carros`, as `voltas`, a `distância` e o `desgaste` já existem no save — a simulação
os produz. Falta só usá-los.

Consequência desejada: uma etapa de MX-5 passa a custar centenas em combustível e milhares em
logística, e uma etapa de Endurance passa a custar dezenas de milhares em combustível e
centenas de milhares em logística e equipe. A diferença entre categorias deixa de ser um
multiplicador e passa a ser **o que realmente é diferente**: a distância percorrida, o número
de pessoas, o preço do pneu.

### 3.3.1 O que a primeira medição física disse

Construída a fatura bottom-up (`economia/ancora.rs` + `economia/evento.rs`, indexada por
**divisão competitiva** e não por categoria — um MX-5 na Production bebe como MX-5), os
números mudaram o entendimento do problema.

**O alvo da seção 2.3 foi atingido.** A gasolina de uma etapa de BMW saiu de ~9.400 para
**$589** — 173 litros, que cabem no tanque. A de Endurance saiu de ~188.000 para **$4.496**,
1.322 litros numa prova de 3h45. Correção de ~42× no topo.

**A âncora antiga não erra de nível, erra de FORMA.** Comparando o custo de eventos de uma
temporada com a porção equivalente do `operating_cost_midpoint`, a divergência aponta para o
mesmo lado na escada inteira — mas o tamanho do buraco varia **12×**:

| Divisão | eventos/temporada | midpoint (porção de eventos) | razão |
|---|---:|---:|---:|
| mazda_amador | 126.544 | 263.500 | 0,48 |
| mazda_rookie | 37.753 | 114.700 | 0,33 |
| bmw_m2 | 172.960 | 682.000 | 0,25 |
| gt4 | 318.162 | 1.705.000 | 0,19 |
| gt3 | 810.190 | 4.960.000 | **0,16** |
| lmp2 | 727.991 | 8.370.000 | 0,09 |
| endurance:gt3 | 593.707 | 10.230.000 | **0,06** |

Se a tabela velha estivesse só superestimada, essa razão seria aproximadamente constante. Ela
não é. **A pirâmide financeira do Loop é muito menos íngreme do que a economia atual assume**:
a escada de eventos vai de 37,7 k a 810 k (**21,5×**), contra os 185 k → 16,5 M (**89×**) da
tabela antiga.

**Ressalva que impede tirar a conclusão cedo demais:** isso mede só EVENTOS. A folha fixa, a
sede, a frota e o seguro são a seção 3.4 e ainda não existem — e é exatamente ali que o topo
escala de verdade (uma equipe de GT3 tem 24 pessoas fixas contra 4 de uma Rookie). Com folha
estimada, a GT3 vai a ~2,25 M contra os 8 M atuais. A amplitude final da escada só se conhece
depois de `temporada.rs`, e é o número que decide se a escada nova fica em 20× ou em 60×.

**Dois achados que a fatura revelou sozinha:**

- **O frete domina a base da escada.** 31% da fatura de uma etapa de Rookie é transporte,
  contra 2% de combustível. É o que quebra equipe pequena de verdade, e agora está legível.
- **Uma prova 4,5× mais longa custa 1,9× mais.** GT3 sprint $54.966 → GT3 no Endurance
  $101.341. O custo de *aparecer* — frete, inscrição, comitiva, hotel — não sabe quantas horas
  a corrida dura. Isso está travado em teste, e é uma propriedade do modelo, não um acidente.

### 3.3.2 Armadilhas registradas na costura

- **`estimate_laps` grampeia em 50 voltas.** Uma etapa de Endurance tem ~143 voltas reais, e o
  campo `voltas` do calendário é ficção lá. Quem integrar a economia nova **não pode**
  alimentar a fatura com `calendar.voltas` no Endurance.
- **Dois carros por equipe no Endurance é o correto para este jogo**, ainda que divirja do
  automobilismo real (onde 2–3 pilotos dividem 1 carro). A simulação grida 36 carros para 18
  equipes e dá a cada piloto posição, voltas e abandono próprios — a economia acompanha a
  simulação, não a realidade. Mudar isso seria mudar o motor de corrida, não a economia.
- **A duração do Endurance não pode ficar achatada na média.** `resolve_race_duration` sorteia
  120/180/240/360 min; combustível e revisão já escalam pelas voltas reais, mas jogos de pneu
  e quilometragem de treino precisam escalar pela duração REAL do evento, não pelos 225 min de
  média. Uma prova de 6 h não pode cobrar pneu de 3h45.

### 3.3.3 A âncora anual fechada, total contra total

Com os recorrentes de `economia/temporada.rs` no lugar e o fator de ponte de 0,62 morto, os
dois lados passaram a cobrir o ano inteiro e a comparação virou direta. O resultado é o achado
mais consequente do redesign inteiro:

A primeira versão desta tabela comparava escopos diferentes: o midpoint velho é **all-in**
(`finance/salary.rs` documenta `DEFAULT_TEAM_SALARY_SHARE_OF_OPERATING = 0.15`, "fração do
custo operacional que a FOLHA de uma equipe (2 pilotos) deve representar") e o bottom-up era
técnico-only. Com o salário de piloto dentro, escrito por mercado e não por fração:

| divisão | TOTAL/ano bottom-up | midpoint antigo | razão |
|---|---:|---:|---:|
| mazda_rookie / toyota_rookie | ~210 000 | 185 000 | **1,13 · 1,14** |
| amador (×2) | ~394 000 | 425 000 | 0,92 |
| bmw_m2 / production | 640 000 – 718 000 | 1 100 000 | 0,58 – 0,65 |
| gt4 | 1 461 012 | 2 750 000 | 0,53 |
| gt3 | 3 372 419 | 8 000 000 | 0,42 |
| lmp2 | 4 218 470 | 13 500 000 | 0,31 |
| end:gt4 · end:gt3 · end:lmp2 | 1,57 M · 3,19 M · 4,28 M | 16 500 000 | **0,09 · 0,19 · 0,26** |

Corroboração que não é calibração: a folha de piloto sai sozinha em **10,2–14,3%** do ano,
contra os 15% que o `salary.rs` projeta por outro caminho.

A escada é **20,4×** com os pilotos dentro — eles escalam junto com a folha técnica, então
quase não mexem na íngreme.

**O achado não é o nível, é o sinal cruzar 1,0.** A tabela velha **subfinancia a base** (1,14)
e superfinancia o topo (0,09): 12,7× de amplitude na divergência. Não é uma escada 4× menor —
é uma escada torta. Todo consumidor que sobrevive à âncora *encolher* pode não sobreviver a
ela *entortar*.

O pior número da auditoria está na última linha. O Endurance tem um midpoint único de 16,5 M
para as **três classes**, então uma equipe de GT4 no Endurance é orçada como se fosse LMP2 —
12× o custo real dela. É a seção 2.2 (o calendário como multiplicador escondido) somada a uma
âncora cega para classe.

Sanidade externa: 3,0 M/ano para uma GT3 de cliente com dois carros e 24 pessoas é o que uma
equipe de campeonato nacional gasta sem desenvolvimento. Os 8 M da tabela velha são orçamento
de equipe de fábrica — o jogo estava cobrando de todo mundo preço de time oficial.

**A escada é de ~21×, não de ~89×.** A tabela velha é 4,2× mais íngreme do que qualquer conta
honesta de corrida de cliente produz.

#### A hipótese dos custos categóricos caiu

A ideia era que a pirâmide viesse de linhas que **nascem** num degrau da escada em vez de
crescerem por ele. O mecanismo é real e ficou modelado — suporte de fábrica e dados nascem no
tier 2, simulador nasce no GT4, e a linha nem aparece na fatura abaixo do degrau. Uma equipe de
Rookie não tem simulador barato: ela não tem simulador.

Mas a magnitude não chega perto. Categórico é 0% na base e no máximo 9,9% do ano em qualquer
divisão; removê-los inteiros muda a escada de 21,0× para 19,4×, **8% da íngreme**. Triplicá-los
dá ~24×, ainda um quarto dos 89×.

**O que faz a pirâmide é a folha técnica.** 4 pessoas a 28k contra 30 a 78k: headcount 7,5× ×
salário 2,8× = 20,9×, contra 21,0× da escada inteira. A pirâmide é "mais gente do mesmo tipo,
paga melhor" — e isso é legível e realista, então fica.

Consequência de desenho: **as linhas categóricas continuam, mas o papel delas mudou.** Elas
não sustentam a escada; elas fazem a fatura da seção 3.7 dizer em que altura da pirâmide a
equipe está. É valor narrativo, não estrutural, e é motivo suficiente.

Três candidatos foram recusados, e as recusas valem mais que a hipótese:

- **Motor selado / aluguel de motor** e **motorhome e hospitalidade** — ambos reais, ambos **já
  contados** (em `revisao_por_km` e entre `frota_anual` e a comitiva da etapa). Cobrá-los de
  novo somaria ~200k ao topo por contagem dupla: seria fabricar a confirmação da hipótese.
- **Túnel de vento não existe em degrau nenhum** — o exemplo era meu e está errado. O Loop
  modela **corrida de cliente**: carro homologado, aero congelada por BoP, chassi spec até no
  LMP2. Nenhuma equipe do jogo desenvolve aerodinâmica, então não há rung em que essa linha
  nasça.

### 3.3.4 O raio de alcance da âncora

Auditados **32 call sites** não-teste. A divisão que importa não é por arquivo, é por **qual
âncora**.

**A categoria "sobrevive porque é razão" estava errada, e a troca provou.** A pergunta certa
não é *se* o consumidor usa razão — é **de qual âncora vem cada lado dela**.
`finance/salary.rs:9` calcula `spending_power ÷ operating_cost_midpoint`: numerador de escala
de **estoque**, denominador de **fluxo**. Só o denominador andou, a razão inflou 2,4× na GT3, e
as equipes foram empurradas contra o teto de `money_factor` — teto comum achata diferença, e a
margem "equipe rica paga mais que equipe quebrada" caiu de 1,20× para 1,176×. A mesma
armadilha espera os 8 consumidores do estoque.

**Consumidores do fluxo (`operating_cost_midpoint`) — quase todos sobrevivem**, porque usam
razão: reserva de dívida, prêmio de construtores, injeção da venda, reparo, salário,
planejamento, upgrade. A exceção é `round_operating_base` (`race/financas.rs:176,242`): nove
linhas de despesa e cinco de receita são frações dele, calibradas contra o nível velho. Quando
o nível se move por fator **diferente por categoria** (1,14 → 0,09), cada categoria reequilibra
em direção diferente. É o caso exemplar de "sobrevive a encolher, não sobrevive a entortar".

**Consumidores do estoque (`expected_cash_midpoint`) — todos os 8 quebram**, porque a seção
3.2 transforma caixa de constante em consequência: receita projetada (`planning.rs:147`,
`0,45 × caixa-médio` — a seção 2.1 em pessoa), crédito, score de saúde, piso de elite, geração
de equipe, paraquedas, pacote de última chance, caixa inicial do draft histórico. E o pior:
**`race/financas.rs:248`, o patrocínio, `caixa-médio ÷ rodadas × 0,27`** — o canal que hoje
carrega 70% da receita está ancorado num estoque.

**As duas âncoras velhas andam travadas.** `expected_cash ÷ operating ≈ 2,05` em toda a
escada: a tabela velha diz, sem nunca dizer, que uma equipe deve guardar **~24 meses de
operação em caixa**. Isso não é equipe de corrida de cliente, é um fundo. E o `spending_score`
é dimensionalmente `âncora_de_estoque ÷ âncora_de_fluxo` — enquanto a razão for 2,05 nada se
move, mas no instante em que o caixa virar consequência **todo time do mundo troca de faixa de
uma vez**.

**`car/cost.rs::category_cost_scale` não é uma segunda âncora — é uma cópia velha que não sabe
que é cópia.** Provado: `operating_cost_midpoint × 0,00065`, desvio < 1,5% nas sete linhas. Se
não andar junto, peça de reposição fica 2,4× mais cara na GT3, **10,5× no Endurance GT4**, e
na base o efeito **inverte** (0,9×, mais barata).

E há uma armadilha dentro dela: `upgrade_price_multiplier` calcula `alvo/base` lendo o midpoint
**ao vivo** contra um `base` **congelado**, então o preço de *upgrade* se autocorrige e o de
*reposição* não. **O caminho que se conserta esconde o que não se conserta.** Como a compra de
peça é o único ralo que escala com riqueza (seção 2.6), o sintoma vai aparecer como "peça de
reposição ficou impagável no Endurance" sem nenhum sinal em upgrade.

**Endurance por classe custa pouco:** `Team.classe: Option<String>` já existe
(`models/team.rs:99`), então é assinatura, não schema — sem migração. A forma tem precedente no
próprio código (`car::cost::category_ceiling_for(category_id, class_id: Option<&str>)`, criada
pelo mesmo motivo). Dos 32 call sites, 15 têm `&team` no escopo e são mecânicos, 8 precisam da
classe encadeada, ~9 são testes.

O único ponto **não** mecânico é `planning.rs:91 operating_cost_midpoint_for_tier`, que mapeia
tier → categoria assumindo uma categoria por tier (`5 => "lmp2"`, `6 => "endurance"`). Com
Endurance por classe o mapeamento fica ambíguo — e ele é a fonte da escada salarial inteira.
Essa função precisa morrer ou virar por divisão.

### 3.3.5 A troca medida — velho contra novo no mesmo binário

**A assinatura do modelo velho, numa linha.** A despesa técnica de uma temporada dividida pelo
custo anual declarado dá **1,16 exato nas catorze divisões**. Não é coincidência: quando toda
linha é fração de um orçamento, o total só pode ser uma constante vezes o orçamento. A escada
inteira gastava a mesma fração, e o número não sabia se estava descrevendo um MX-5 ou um LMP2.
O modelo físico cai em `1 − folha de pilotos ÷ anual` (0,84–0,91) com folga de 0 a 3 pontos —
o alvo não é 1,00 porque o piloto sai do caixa como contrato, não como despesa técnica.

Fatura de uma etapa, velho → novo, razão na faixa das 9 categorias:

| linha | razão | o que está saindo |
|---|---:|---|
| combustível | **0,06 – 0,11×** | o critério 10 fechando: a linha que media 16–39× a âncora física agora **é** a âncora |
| estadia · inscrição · diárias | 0,14 – 0,65× | gente × preço inflado por um peso de orçamento vestindo nome de coisa contável |
| frete · viagem | 0,28 – 1,07× | o multiplicador de escala saindo; quilômetro não sabe qual é o orçamento da categoria |
| mecânica | 0,29 – 0,99× | `0,16 × base` sem referente virando revisão por km — 0,99× na GT3, onde peça real já dominava |
| **estrutura** | **1,14 – 1,44×** | a única linha que **sobe**, e sobe em toda categoria |
| **etapa** | 0,75 – 0,97× | |

**Três efeitos que ninguém previu:**

1. **A operação permanente estava subprecificada em cerca de um terço**, e agora é **50–60% da
   fatura por rodada**. A etapa ficou mais barata e o ano ficou mais caro, então a conta
   responde menos ao que a equipe faz na pista. **Isso fica**, e é decisão, não acidente: em
   corrida de cliente o custo fixo domina mesmo, e a sensibilidade a resultado mora na
   **receita** — prêmio por etapa é 50% da renda, com curva convexa γ=6,5. Despesa estável,
   renda dirigida por desempenho, é a forma certa. Não "conserte" isso depois.
2. **Trocar só a despesa deixa o mundo mais RICO.** A despesa caiu 3–25% e a receita **não se
   mexeu** — patrocínio 0,98–1,08×, bilheteria e fechamento exatamente 1,00×. É a prova
   aritmética de que a receita estava ancorada num número que a despesa não toca. A troca
   isolada é um aumento de renda real distribuído a todo mundo, e por isso a recalibração da
   receita é **unidirecional: ela desce**.
3. **A gt4 é a única que piora, e piora muito** — crise 23,5 → 39,5%, colapso 17,5 → 36,0%,
   vendas 8,0 → 17,5%. **Mecanismo achado: o freio automático sumiu.**

   No modelo velho a fatura era majoritariamente variável, e variável **encolhe sozinho num ano
   ruim** — carro que abandona queima menos combustível, equipe sem caixa compra menos peça. A
   equipe que ia mal gastava menos sem decidir nada, e atravessava o ano. O físico moveu ~40%
   da fatura para o bloco fixo. A conta agora chega igual, e um ano ruim virou **terminal** em
   vez de sobrevivível.

   A medição: no legado **nenhuma** das 108 equipes da escada tinha conta fixa acima de 54% da
   receita; no físico 38 passam de 60%, e em duas categorias há equipe cuja conta fixa é maior
   que a receita inteira. A autópsia de quem quebra na gt4 mostra o fim da linha — estrutura
   78,6% da receita (era 32,7%), juros 53,0%, técnica **0,0%**: já cortou tudo que dava.

   Por que a gt4 e não a gt3: as duas cruzaram, mas a gt3 **já estava saturada** (82 → 77
   anos-colapso, ela não tinha para onde piorar) e a gt4 dobrou (35 → 72). A gt4 estava sentada
   em cima da linha. Quem cruza é decidido pelo produto de dois fatores — o mediano de
   fixo÷receita (gt4 82,8% e gt3 83,1% contra 45–64% de todas as outras) e o espalhamento da
   receita dentro do grid.

   **E a causa raiz é a receita velha, não a despesa física:** a receita de produção não é
   proporcional à âncora, e receita÷despesa por etapa vai de 1,10 (gt4) a 1,70 (endurance) — a
   gt4 é onde a receita velha é mais magra contra o custo real. **A troca é aceitável porque a
   receita nova corrige**: sobre `economia::receita` com âncora física a gt4 sai de 17,50% de
   vendas para 2,50%, dentro do critério 8, e a gt3 para 0,71%.

**Armadilha de medição registrada:** a migração v61 é **destrutiva** em `team_finance_history`.
Abrir uma cópia de save real para inspeção roda a migração e apaga o ledger — uma medição feita
depois disso reporta "zero linhas" e parece um defeito de escrita. Os quatro saves reais têm
1.803 a 7.346 linhas de ledger, lidos crus, sem migração. **Inspeção de save real usa leitura
crua (`node:sqlite`), nunca o caminho do Rust.**

**Armadilha operacional descoberta aqui, e ela invalida comparações antigas:** o harness é
determinístico, mas a **árvore não estava**. Duas execuções separadas por dez minutos
compilaram mundos diferentes porque outras sessões editavam `fame.rs`, `public_presence/`,
`event_interest/` e `finance/rescue.rs`. **Qualquer comparação velho-contra-novo precisa rodar
os dois lados dentro do mesmo binário** — confrontar dois relatórios não prova nada.

**Correção de fator para a recalibração:** o `~1,96` registrado antes era a despesa **total**
medida (técnica + folha real + peça + serviço de dívida). A troca moveu só o bloco técnico,
1,16 → ~0,87, fator **1,33×**. Folha, peça e dívida não mudaram de tamanho.

### 3.4 Linhas de despesa novas

O que hoje não existe e deveria — tanto por realismo quanto para diluir as linhas que hoje
carregam sozinhas o peso do orçamento.

**Recorrentes de temporada** (mensais ou anuais, não por etapa):

- Folha da equipe técnica: nº de pessoas × salário médio, por categoria. Hoje só o piloto tem
  salário; uma equipe de GT3 tem 20–40 pessoas.
- Sede: aluguel/manutenção do galpão, proporcional a `facilities`.
- Frota: caminhão, motorhome, van — compra amortizada e manutenção.
- Seguros e licenças: cobertura de equipe e inscrição no campeonato.
- Simulador / túnel de vento / CFD: só nas categorias altas, e é uma **escolha** (gasta e
  ganha desenvolvimento).

**Recorrentes de quilometragem** (o que dilui a peça):

- Revisão de motor a cada N km — ou aluguel de motor por temporada nas categorias onde isso é
  a norma.
- Revisão de câmbio e diferencial.
- Revisão de freio: discos e pastilhas por evento.

**Eventuais**:

- Reparo de dano de batida (já existe parcialmente via `car/crash.rs`).
- Multas: bandeira preta, incidente com culpa, infração técnica.
- Teste privado entre etapas: custa dinheiro, melhora acerto. Decisão, não cenário.

**O ralo que falta** (resolve 2.6) — MEDIDO, e a proposta original desta seção estava errada.

O harness varreu três formas de ralo. A pergunta era quanto ele precisa drenar; a resposta é
que **não existe um número**, porque o superávit varia 113 pontos ao longo da escada:

| Categoria | superávit (% do operacional anual, por equipe, por temporada) |
|---|---:|
| toyota_rookie · endurance · mazda_rookie | +90,5% · +87,6% · +86,8% |
| production_challenger | +54,2% |
| bmw_m2 · mazda_amador · toyota_amador | +30,5% · +28,2% · +25,5% |
| gt4 | +7,3% |
| **gt3** | **−22,5%** |

Média +43,1%. A GT3 não tem excedente para drenar — ela tem déficit.

As duas formas que esta seção propunha **não funcionam em magnitude nenhuma**:

- **Manter estrutura** (custo fixo proporcional ao porte): a curva trava em 5 de 9 categorias
  entre 0,30 e 1,00 e nunca fecha. E estoura o critério 7 antes de a deriva sair do lugar — em
  `manter 0,15` a BMW já caiu de 1,76× para 0,55× enquanto a Mazda Rookie só saiu de 8,08×
  para 7,00×. O mesmo ralo que mal arranha uma já matou a outra.
- **Melhorar estrutura** (preço por ponto): só fecha cobrando **128× o operacional anual por
  100 pontos** de estrutura, com dois terços do mundo em crise. A causa está medida: o
  offseason só regala **2,76 pontos de estrutura por equipe por temporada**. Esse é o
  denominador. Para drenar 43% com 2,76 pontos, o ponto teria que custar ~15% do orçamento
  anual — um preço sem sentido para subir `facilities` de 55 para 56.
- **A + B combinados**: a melhor das 12 grades fecha 9 de 9 drenando 76,7%, mas com 29,7% de
  crise, o dobro do teto.

**O que funciona é condicionar o ralo ao EXCEDENTE, não ao PORTE.** Um dreno sobre o caixa
acima de ~12 meses de operação, a 0,40, fecha 9 de 9 com a pior deriva em 0,96× e a crise em
12,2%. A razão é estrutural: porte não tem correlação com superávit — a GT3 tem o segundo
maior porte da escada e superávit negativo. Drenar por porte cobra caro de quem não tem. O
dreno por excedente é auto-escalável: **a GT3 fica intocada em todas as magnitudes**, porque
nunca tem caixa acima de 12 meses, enquanto a Rookie paga quase 97% do operacional.

**Como isso vira desenho, e não um imposto.** A leitura literal de "melhorar estrutura passa a
custar" é a forma fraca. A forma que funciona é: **a equipe INVESTE o excedente e recebe
estrutura proporcional ao que investiu**, com retornos decrescentes — em vez de ganhar pontos
de graça e ser cobrada por eles a preço de tabela. Isso é a forma C vestida de B: o dreno
escala com o caixa disponível porque a *decisão de gastar* escala com o caixa disponível. É
mecanicamente eficaz e realista ao mesmo tempo, e é o que `economia/desenvolvimento.rs` deve
implementar.

E confirma a leitura da seção 2.9: **o ralo não conserta a GT3**. Ela é a única categoria em
que qualquer ralo é dano puro, porque o problema dela não é caixa sobrando, é despesa.

#### Resultado medido — `economia/desenvolvimento.rs`

O módulo reproduz o dreno sintético da forma C dentro de ~4% em toda linha: pior deriva 1,01×
(alvo 0,96×), 9 de 9 categorias abaixo de 1,3×, GT3 intocada em 0,25×, dreno médio 64,8% do
operacional anual.

As duas divergências são efeitos de segunda ordem que o dreno sintético não podia ter, e
ambas apontam para o lado certo:

- **Dreno 4% maior.** O módulo também absorve os 2,76 pontos de graça, e estrutura menor
  reduz o `facilities_factor`, ou seja a própria fatura de operação — o excedente se forma
  sobre uma base diferente.
- **Crise 2 pontos MENOR** que o dreno sintético (17,7% contra 19,7%), porque o módulo passa
  pelo `apetite_do_foco`: uma equipe em `Sobrevivencia` investe 15% do excedente, não 40%. O
  dreno sintético cobrava igual de todo mundo. O módulo é mais gentil com quem afunda, e é o
  comportamento certo.

**A estrutura deprecia 2% ao ano.** Não estava no enunciado; entrou porque sem ela o modelo
vira catraca — todo mundo sobe até o teto e, ao chegar lá, **para de drenar**, secando o ralo
exatamente na equipe mais rica. Depreciação é a resposta direta à seção 2.6: é o primeiro
débito do jogo que **escala com riqueza**, porque 2% de uma estrutura grande é mais do que 2%
de uma pequena. Ficar no topo passa a custar todo ano.

O efeito colateral é desejável e vale registrar: a depreciação tira ~4 pontos/ano de quem está
no teto e ~0,8 de quem está no fundo, contra 2,76 pontos que o offseason regalava a todos por
igual. Sem investir, o rico encolhe e o pobre cresce — uma força de convergência que **não
passa pelo caixa**.

**Sem teto de investimento.** Todo teto que morde (≤ 1,0× do operacional anual) devolve a
deriva onde ela era pior — 7,15× nas Rookies, 7,41× no Endurance no teto de 0,25× — e um teto
de 2,0× nunca é alcançado, sendo indistinguível de não ter. O receio de que sem teto a escada
esportiva vire função do caixa não se materializa: **com o módulo a estrutura mediana nunca
encosta no teto de 200, e com o modelo de hoje ela encosta em quatro categorias.** O retorno
saturante já limita o que o dinheiro compra; limitar o gasto só desliga o ralo.

**O resultado principal não é a amplitude da estrutura, é a ordenação.** Hoje só **2 de 9**
categorias terminam com rico > mediano > pobre em estrutura; em Mazda Rookie a equipe mais
pobre termina com 176 e a mais rica com 126. Com o módulo, **6 de 9** ficam ordenadas. As três
que sobram — as duas Rookies e o Endurance — ficam desordenadas porque com o ralo o caixa
converge (deriva ~1,0), e "a mais rica no fim" vira quase sorteio onde todo mundo paga tudo.
Não é falha do modelo: é o caixa deixando de ser o eixo de diferenciação nas categorias em que
ele não deveria ser.

**Os dois parâmetros são separáveis** — um botão por critério, sem se atropelarem:

| parâmetro | move | não move |
|---|---|---|
| `fracao_do_excedente` | deriva (critério 2): 0,20 → 5/9 · 0,30 → 9/9 no limite · 0,40 → 1,01× com margem | — |
| `depreciacao_anual` | crise (critério 7): 0,00 → 19,7% · 0,04 → 15,8% | deriva (0,93× → 1,05× no intervalo inteiro) |

`depreciacao_anual` funciona sobre a crise porque estrutura menor significa fatura de operação
menor, e isso é alívio de custo para quem está apertado. **É o botão de acabamento e deve ser
gasto por último**, depois da seção 3.5 — crise também é função da receita, e mexer nele antes
é calibrar contra alvo móvel.

### 3.5 Receita

Cinco canais, na ordem de peso pretendida:

1. **Prêmio por etapa** — por posição na classe, pago toda corrida. O canal principal.
   Escala com a categoria e é o que faz resultado virar dinheiro.
2. **Prêmio de volta mais rápida** — pequeno, por etapa. Hoje não existe como dinheiro (o
   `BONUS_FASTEST_LAP` é +1 ponto de campeonato).
3. **Patrocínio** — contrato, valor conhecido, negociado por reputação e fama do lineup. Sem
   realimentação de riqueza. ~~É o piso que permite sobreviver.~~ **Essa descrição estava
   errada e foi a medição do critério 6 que mostrou.** A razão campeão ÷ lanterna é
   `(S + P_campeão) / (S + P_lanterna)`, e toda receita de patrocínio entra em `S`, nos **dois**
   lados da fração: cada ponto de participação do patrocínio empurra a razão em direção a 1,
   monotonicamente. Com o patrocínio em 21% o critério 6 já falha nas Rookies; a 40% afunda.
   Patrocínio fica em **~20% da receita, dinheiro de bolso**. Quem permite sobreviver passa a
   ser o prêmio de etapa até o meio da classe — que é o que "prêmio por etapa é o canal
   principal" já dizia.
4. **Bilheteria** — bolo de **evento** (`público × ingresso médio`, escalado por prestígio da
   etapa), dividido por cota de público. Para isso funcionar como diferenciador, a **amplitude
   da fama precisa mudar antes**: hoje a mídia dos pilotos vive entre 21 e 84 com médias de
   categoria entre 36 e 48, e por isso a cota colapsa em 1/N.
5. **Prêmio de fim de temporada** — reduzido ao papel de bônus, não de muleta. Medido hoje:
   zerá-lo derruba a GT3 de 0,89 para 0,71 e dobra a crise, o que é a definição de muleta.

### 3.6 Fama com amplitude

Objetivo: a presença de público precisa ter **espalhamento real** dentro da categoria, senão
qualquer bilheteria — grande ou pequena — paga igual para todo mundo.

Três mudanças, na ordem de impacto:

1. **O piso do decaimento passa a ser pessoal.** Hoje todo piloto decai para 25. Passa a ser
   `25 + f(títulos, vitórias de carreira, temporadas na elite)` — um campeão não vira anônimo
   porque teve um ano ruim. É o que trava a compressão na base.
2. **O público não vem só pela fama.** A cota de bilheteria deixa de ser `presença/total` e
   passa a ser um composto: fama do lineup **×** competitividade atual (posição no
   campeonato, forma recente) **+** vínculo local (equipe sediada no país da pista) **+**
   história (títulos da equipe). Posição no campeonato tem espalhamento total por construção,
   então o composto tem amplitude mesmo enquanto a fama ainda estiver comprimida.
3. **O tamanho da plateia é da categoria, não do piloto.** Ser famoso no Endurance vale mais
   dinheiro que ser famoso na Rookie, mesmo com a mesma `midia`. Isso entra como escala do
   bolo do evento, não como distorção do atributo — que precisa continuar comparável, porque
   o mercado e as notícias leem os mesmos 6 níveis.

O que **não** muda: a faixa 0–100 do atributo e os 6 níveis da ficha. Mexer neles quebraria
`fame.rs`, o mercado e o gerador de notícias de uma vez.

#### Resultado medido

O desvio da mídia dentro da categoria subiu de ~8 pontos (save real) para 12,6–19,3, batendo o
alvo de 15 em 4 das 5 categorias medidas. A Rookie fica em 12,6 **de propósito**: a escala por
tier (0,85) segura o ganho na base para a pirâmide ter inclinação. A atração de público separa
a melhor da pior equipe do grid por mais de 80 pontos numa escala de 100 — contra os 1,2× de
hoje.

As peças que produziram isso: piso de decaimento pessoal
(`25 + 12·√títulos + 1,6·√vitórias + min(10, temporadas na elite)`, com teto em 78 — dentro de
"Estrela", nunca "Ídolo", para ninguém receber fama de topo por decreto); faixa de visibilidade
**relativa ao tamanho do grid** (~40%, presa entre P5 e P10, porque uma faixa fixa até P10
premiava 10 dos 12 carros de uma Rookie e premiar todo mundo comprime igual a premiar
ninguém); escala por tier de 0,85 a 1,45; e a taxa de decaimento de 0,025 para 0,021, que é o
divisor do espalhamento de equilíbrio (`piso + ganho/taxa`).

**Consequência que a economia precisa saber:** a população se move entre os 6 níveis da ficha
— "Estrela" vai de 0,9% para 5,2% e "Ídolo" de 0,9% para 3,4%, ou seja, de 2 para 10 pilotos
acima de "Nome forte" num mundo de ~116. Como `fame_commercial_units` salta de 30 para 55
entre esses níveis, o mercado passa a ter mais candidatos cujo apelo comercial cobre um gap de
skill. E a presença pública das equipes sobe junto, então o termo de patrocínio por fama
(`fama × base × 0,004`) **vai render mais** — o modelo novo não pode herdar esse coeficiente
sem recalibrar.

Fonte de fama ainda não modelada no harness: `evolution/growth.rs` dá +1 a +3 de mídia por
temporada boa, no fim do ano. Os números acima são conservadores por essa margem.

### 3.7 A fatura que o jogador vê

Como o jogador **só pilota**, a fatura não é um painel de decisão — é a prestação de contas
que faz o mundo parecer real. Isso separa duas coisas que hoje são a mesma:

- **Modelo interno**: ~20 linhas, cada uma com quantidade física e preço unitário. Precisa ser
  detalhado para as equipes de IA se comportarem certo e para o harness poder medir.
- **Tela do pós-corrida**: **4 blocos, 8 linhas visíveis**, detalhe no expandir.

```
CORRIDA          combustível · pneus · desgaste de peça
LOGÍSTICA        frete · viagem e estadia · inscrição
EQUIPE           diárias do fim de semana · rateio da folha fixa
RECEITA          prêmio da etapa · volta mais rápida · bilheteria · patrocínio (rateio)
```

Regra de ouro que a fatura de hoje viola: **o rótulo e o número contam a mesma história**.
Se a linha diz "combustível", o valor tem que ser o que caberia no tanque.

### 3.8 Onde mora

Módulo novo, `src-tauri/src/economia/`, sem dependência de Tauri e sem tocar o banco:

```
economia/
  mod.rs          — a fachada e o struct de saída
  ancora.rs       — custo operacional de referência por categoria
  evento.rs       — a fatura de uma etapa (bottom-up)
  temporada.rs    — os recorrentes (folha, sede, frota, seguro)
  receita.rs      — os cinco canais
  desenvolvimento.rs — o preço de melhorar carro e estrutura (o ralo)
  estado.rs       — saúde financeira em meses de operação
  tipos.rs        — entrada e saída, puros
```

Entrada: um struct com tudo que a economia precisa (equipe, carro, resultado da etapa,
categoria, posição no calendário, pista) — sem `Connection`, sem `AppHandle`. Saída: as linhas
do ledger + os deltas de estado. Puro, testável, e o harness dirige as duas implementações
lado a lado.

### 3.9 A troca

A economia atual entra por uma função e sai por um struct
(`calculate_team_round_finance_context` → `TeamRoundFinanceContext`), chamada de um lugar só
(`race/persistencia.rs`). Isso é uma costura de verdade. A troca é:

1. O módulo novo produz um `LancamentosDaEtapa` com as linhas novas.
2. `persistencia.rs` passa a chamar o novo.
3. `team_finance_history` ganha as colunas novas (migração; saves antigos não importam).
4. O dossiê da aba My Team passa a ler as linhas novas.

---

## Parte 4 — Critérios de aceitação

Escritos como teste no harness **antes** de escrever o modelo. "Ficou bom" deixa de ser
opinião.

**Regra geral: todo alvo é POR CATEGORIA, não agregado.** O agregado esconde exatamente a
patologia que estamos consertando — o prêmio por corrida do mundo está a 2 pontos do alvo
enquanto o da GT3 está a 18. Onde a métrica só faz sentido no mundo, está dito.

| # | Métrica | Alvo | Hoje |
|---|---|---|---|
| 1 | receita ÷ despesa | 0,95 – 1,15 em toda categoria | 0,89 – 1,54 (7 de 9 fora) |
| 2 | fôlego ao fim de 20 temporadas | **toda equipe** entre 3 e 18 meses (banda — ver 4.9) | **22,9 – 246,0 meses · 9 de 9 fora, todas por excesso** |
| 3 | prêmio por corrida, % da receita | ≥ 40% em toda categoria | 22,3 – 42,6% |
| 4 | bilheteria, % da receita | 10 – 20% em toda categoria | 0,29 – 0,55% |
| 5 | prêmio de fim de temporada, % da receita | ≤ 10% em toda categoria | 14,3 – 21,6% |
| 6 | campeão ÷ lanterna (receita de temporada) | ≥ 3× em toda categoria | 1,67 – 3,51× (4 falham) |
| 7 | equipes em crise · e colapso < crise | crise 5 – 12% no mundo | **0,6%** honesto (ver nota) |
| 8 | equipes vendidas por falência | 0,5 – 5% do grid por temporada em categoria de **10+ equipes**; sem piso abaixo disso | 4 de 9 fora, todas grid pequeno |
| 9 | nenhuma categoria pior em todos os cenários da varredura | obrigatório | GT3 é a pior em 100% |
| 10 | linha de combustível de uma etapa | ≤ 10× a âncora física (4.1) | 24× – 84× |
| 11a | espalhamento do portão **somado na temporada** | melhor ÷ pior ≥ 2,5× | 1,4 – 2,6× ✗ (ver nota) |
| 11b | portão como % da receita da equipe de **pior atração** | ≥ 10% em toda categoria | 11,9 – 21,4% ✓ |

O critério de amplitude da **mídia dos pilotos** saiu daqui: ele não é mensurável neste
harness, que sintetiza a presença da equipe a partir de uma constante. Ele mora no harness de
fama (`public_presence`), com alvo de desvio ≥ 15 pontos dentro da categoria contra os ~8 de
hoje.

Notas de leitura sobre o placar de hoje:

- O critério 8 estava mal formulado na primeira versão ("> 0 e < 1 venda por categoria por
  temporada") e **passava** com os números de hoje. Normalizado pelo tamanho do grid ele
  morde: a GT3 vende 14,3% do grid por temporada (toda equipe trocaria de dono a cada 7 anos)
  e o Endurance vende zero (ninguém quebra nunca). As duas pontas são o mesmo defeito.

  **Depois do termômetro consertado ele virou o sinal mais afiado do projeto, e quem mudou não
  foi a reescrita da venda.** O gatilho é `collapse` em duas temporadas seguidas, e o colapso
  caiu de comum para 0,4% — o `rescue.rs` mudou o *prêmio* de falir, o divisor mudou a
  *frequência*. De 82 vendas (0,00–14,29%) para **3** (0,00–1,00%): 7 de 9 categorias em zero,
  ninguém quebra nunca. E com a receita nova ligada estoura na ponta oposta, **19,64%** na pior
  categoria.

  As duas pontas bracketam o alvo e nomeiam o culpado: é a **convexidade γ=5** da curva de
  prêmio matando o fundo de cada grid. O critério 8 passa a ser a restrição que amarra a
  calibração da receita — mais do que o 6, que já foi afrouxado.

  **Confirmado depois por experimento direto: o critério 8 não é sobre a venda.** Varrendo o
  aporte da falência em 4,2 / 3,0 / 2,0 meses, Rookie, Production e Endurance ficam em 0,00%
  nos três regimes (rec/desp 1,49–1,69, 240 meses de fôlego — lá ninguém quebra nunca, e
  nenhum valor de aporte alcança isso por baixo), enquanto a GT3 faz 13,6% com rec/desp 0,89
  (lá todo mundo quebra, e o aporte só regula a frequência do ciclo). **As duas pontas do
  critério 8 são o mesmo defeito, e ele é da receita.**
- O critério 6 não é uma tendência, é um **corte limpo no calendário**: toda categoria com 8
  ou mais etapas passa, toda categoria abaixo falha, sem exceção nos dois sentidos.

  | Categoria | rodadas | campeão ÷ lanterna |
  |---|---:|---:|
  | toyota_rookie / mazda_rookie | 5 | 1,67× · 1,72× ✗ |
  | endurance | 6 | 2,24× ✗ |
  | amador (×2) / bmw_m2 | 8 | 3,22× · 3,40× · 3,16× ✓ |
  | production / gt4 | 10 | 3,03× · 3,48× ✓ |
  | gt3 | 14 | 3,51× ✓ |

  ~~Requisito de projeto para a seção 3.5: o prêmio por etapa precisa escalar inversamente ao
  número de etapas.~~ **Retirado — a medição derrubou.** O expoente do calendário é quase
  inerte no critério 6 (mazda_rookie vai de 1,86 para 2,08 quando o expoente vai de 1,0 a 2,2),
  porque aumentar o bolo multiplica os dois lados da razão. Quem move o critério 6 é a
  **convexidade da curva por posição** (γ): γ=1,0 dá 1,45× nas Rookies, γ=5,0 dá 6,73× na GT3.
  O mecanismo real: com poucas corridas o campeonato é dominado por ruído, então o campeão
  termina **perto** do lanterna em posição — só uma curva convexa transforma uma diferença
  pequena de posição numa diferença grande de dinheiro.

  **E o alvo de 3× fixo caiu junto: os critérios 6 e 3 são incompatíveis.** O expoente é o
  único parâmetro que age seletivamente no calendário curto, e ele só ajuda as Rookies
  **tirando** prêmio do calendário longo. Para as Rookies chegarem a 3× (expoente 4,0), a fatia
  de prêmio de corrida da GT3 cai a 35% — abaixo do piso do próprio critério 3 — e o critério 1
  quebra junto (0,92–1,07).

  O alvo passa a ser **função do calendário**, porque o que o critério protege é "ganhar muda a
  vida da equipe", e 2,2× de receita já muda:

  | rodadas | alvo campeão ÷ lanterna |
  |---|---|
  | ≤ 6 | ≥ 2,0× |
  | ≥ 8 | ≥ 3,0× |

  Registrado como afrouxamento deliberado de um critério que eu escrevi: o 3× nunca foi
  derivado, foi escolhido antes de existir a medição que mostra por que o calendário curto não
  o alcança sem dano. Os critérios 1 e 3 **não** foram afrouxados.

- **O critério 2 mudou de unidade pelo mesmo motivo que o 7.** O alvo de "< 1,3× de deriva" foi
  escrito quando a equipe nascia com ~24 meses de caixa; com a âncora de estoque honesta ela
  nasce com **1–11 meses, mediana 6**. Uma razão é adimensional, mas o destino não é: 3,0× de
  uma base de 6 meses são 18 meses — dentro de "saudável", longe de "elite". E 1,3× de uma base
  inflada era um mundo pior. O alvo passa a ser o **destino em meses**, não a razão.

- **O critério 11 estava medindo duas estatísticas diferentes sem escolher uma.** A atração de
  uma equipe **numa etapa** espalha 2,70–5,13×; o **portão somado da temporada** entre a melhor
  e a pior equipe espalha 1,4–2,6×. Posição de campeonato e forma recente oscilam ao longo do
  ano, e o agregado comprime. Não era discordância de medição.

  **A que importa é a da temporada**, porque a economia de uma equipe é anual: um espalhamento
  de etapa que se dissolve ao longo do ano não diferencia a vida financeira de ninguém. Nessa
  definição **o critério 11a falha**, e nenhum afrouxamento razoável o salva — nem tornando-o
  função do calendário como o 6, porque falha também em GT3 (14 etapas, 2,04×).

  A causa é a mesma do critério 6: em grid pequeno e calendário curto o campeonato é dominado
  por ruído, e ruído se cancela no agregado anual. Diferenciar exigiria um termo **persistente**
  — prestígio histórico da equipe, ou corrida em casa — que hoje não existe.

  **Mas a medição encontrou uma propriedade melhor do que a que o critério pedia**, e por isso
  ele virou 11a/11b: o portão vale **11,9–21,4% da receita da equipe de pior atração**, e em 5
  das 9 categorias vale **mais** para ela do que para a média da categoria. A convexidade do
  prêmio faz o fundo do grid ganhar pouco em pista, então o portão ocupa fatia maior da receita
  menor dele. **O portão sustenta o fundo em vez de amplificar o topo** — e isso é um desenho
  mais saudável do que o que eu tinha escrito como alvo.

- **Os critérios 2, 8 e 11a falham nas MESMAS quatro categorias** — as duas Rookies e as duas
  Amadores — e nenhuma alavanca alcança: cortar receita mata as outras antes, γ não move porque
  a curva convexa precisa de posições para separar e há 6, e o ralo não cria falência porque
  drenar **reduz** a crise. É o mesmo *ruído em grid pequeno se cancelando no agregado* que já
  derrubou o 6 e o 11a, medido agora por um quarto caminho independente.

  Por isso o **critério 8 virou função do tamanho do grid**, como o 6 virou função do
  calendário. Uma categoria de 6 equipes tem 120 equipe-temporadas em 20 anos: exigir 0,5% dela
  é exigir que uma em cada seis equipes do **degrau de entrada** quebre. Não é o mundo que se
  quer no primeiro degrau — quem entra na pirâmide encontra equipes pobres, não equipes
  falindo.

- **O ✓ do critério 7 foi medido num mundo que lavava a evidência.** Quando `finance/rescue.rs`
  parou de injetar 45% do caixa-médio da categoria na venda por falência (correção do item 2
  da seção 2.5), a crise no baseline **sem ralo nenhum** subiu de 12,2% para 18,1%, e a deriva
  da GT3 caiu de 0,43× para 0,21×. Não é regressão: antes, a equipe quebrada voltava com caixa
  cheio e **saía** da faixa de crise: o defeito estava apagando o próprio rastro. 18,1% é o
  número honesto de um mundo com o ralo ainda quebrado.

  Consequência de leitura: a faixa 8–15% foi escrita contra dados contaminados, do mesmo jeito
  que as medições de referência estavam contaminadas pelo join de categoria. O ralo, aliás,
  *melhora* a crise contra o baseline (17,7% contra 18,1%); a receita nova a levou a 21,6%.

  **Os dois números foram retirados.** Com as bandas reancoradas em meses, a taxa honesta do
  mundo de hoje é **0,6%**, não 18,1% nem 21,6% — os dois foram lidos no termômetro quebrado.

  **E o termômetro não era passivo, que é o achado maior.** `financial_state` dispara
  empréstimo de emergência a 1,18×, escolhe a estratégia da temporada e arma a venda por
  colapso. Uma equipe falsamente rotulada de "crise" tomava empréstimo, virava `survival` e era
  vendida. **3,6% de leitura falsa realimentada virava 18,1% de doença real.** O instrumento
  quebrado não estava só lendo o mundo errado — estava adoecendo o mundo.

  **A faixa 8–15% mede a grandeza errada, não só com dado contaminado.** Ela não distingue
  crise de colapso, e a forma importa mais que o nível: no mundo com ralo + receita os 14,7%
  são 2,9% de crise e **11,8% de colapso** — quatro vezes mais equipes mortas do que morrendo.
  Isso passaria no critério, e é pior que 12% de crise com 2% de colapso, que também passaria.
  Um mundo saudável tem mais gente adoecendo do que morta.

  **Critério 7 passa a ser: crise 5–12% no mundo, E colapso < crise.** A segunda metade é
  derivada da medição acima. A primeira é palpite calibrado e está registrada como tal — vai
  ser re-derivada quando a curva de prêmio parar de matar o fundo dos grids.

- **Ressalva no critério 3, para não ser mal lido depois da reescrita.** A única categoria que
  hoje atinge o alvo de prêmio por corrida é o Endurance (42,6%) — e ela chega lá exatamente
  pelo defeito da seção 2.2: com 6 etapas, o `base` por rodada é o mais inflado da escada, e o
  prêmio por resultado é múltiplo desse base. Quando o multiplicador escondido do calendário
  sumir, o Endurance vai cair abaixo de 40%. **Isso não é regressão** — é o número saindo de
  trás do bug. O alvo passa a ser atingido pelo desenho do prêmio, não pela divisão.

### 4.1 Âncora física de combustível

A conta é por **litro por hora de pista**, não por quilômetro: é assim que uma equipe
dimensiona stint, e evita ter que inventar uma velocidade média por categoria.
`duração × L/h × nº de carros × fator de fim de semana × preço do litro`. Duração vem de
`duracao_corrida_min`; no Endurance, da média do sorteio de `resolve_race_duration` (225 min).
Fator de fim de semana 2,5× nas sprints (treino + classificação + corrida) e 1,8× no
Endurance, onde a corrida domina. Preço: US$ 3/L, combustível de competição em tambor.

| Categoria | L/h | Âncora por etapa | Hoje | Razão |
|---|---:|---:|---:|---:|
| mazda_rookie | 20 | 75 | 2 582 | 34× |
| toyota_rookie | 24 | 90 | 2 585 | 29× |
| mazda_amador | 20 | 125 | 3 669 | 29× |
| toyota_amador | 24 | 150 | 3 673 | 24× |
| bmw_m2 | 35 | 219 | 9 164 | 42× |
| production_challenger | 28 | 210 | 7 504 | 36× |
| gt4 | 40 | 300 | 18 843 | 63× |
| gt3 | 57 | 712 | 39 451 | 55× |
| endurance | 50 | 2 025 | 169 674 | 84× |

Esta tabela é **alvo de aceitação**, derivada de fora do modelo. O modelo bottom-up da seção
3.3 chega ao mesmo número por um caminho independente (litros consumidos na simulação). As
duas derivações precisam concordar dentro de uma ordem de grandeza — se divergirem, uma das
duas está errada e isso é informação, não erro de arredondamento.

---

### 4.2 Placar de aceitação — modelo novo inteiro, um binário

`economia::evento` + `temporada` na despesa, `economia::receita` (γ 6,5, nível 1,00),
`economia::desenvolvimento` (reserva 9 meses, 40% do excedente), âncora física por classe.
9 categorias × 20 temporadas.

**A coluna da direita é a re-medição com `spending_power` re-derivado (4.5).** A coluna do meio
é o placar que este documento carregava, medido com o defeito ainda de pé — a equipe mediana
com poder de gasto negativo, o que travava três consumidores no piso ao mesmo tempo. As duas
colunas saíram do mesmo harness; a da direita saiu de dois runs consecutivos que bateram número
a número, então o que separa as duas é o `spending_power`, não deriva de árvore.

**De 9 de 12 para 7 de 12.**

| # | alvo | medido (antes da 4.5) | medido (agora) | veredito |
|---|---|---|---|---|
| 1 | 0,95–1,15 | 1,02–1,04 · 0 fora | **0,90–1,03 · 2 fora** | TRABALHO PENDENTE |
| 2 | 3–18 meses, **toda equipe** | 10,9–22,1 · 2 fora *(mediana)* | **−6,7 – 86,6 · 9 fora** (34↓ 40↑ de 102) | TRABALHO PENDENTE |
| 3 | ≥40% | 48,7–51,4% · 0 fora | 48,7–51,4% · 0 fora | **passa** |
| 4 | 10–20% | 12,6–16,6% · 0 fora | 12,6–16,6% · 0 fora | **passa** |
| 5 | ≤10% | 3,2–3,3% · 0 fora | 3,2–3,3% · 0 fora | **passa** |
| 6 | ≥2× até 6 · ≥3× de 8 | 2,16–5,70× · 0 fora | 2,16–6,63× · 0 fora | **passa** |
| 7 | 5–12% · colapso < crise | 5,0% · 56 crise / 47 colapso | **30,9% · 196 crise / 435 colapso ✗forma** | alavanca esgotada |
| 8 | 0,5–5% · grid ≥10 | 0,00–3,33% · 2 fora | **3,06–22,14% · 5 fora** (208 vendas) | defeito conhecido |
| 9 | nenhuma categoria refém | a pior muda de cenário | a pior muda de cenário | **passa** |
| 10 | ≤10× a âncora 4.1 | 1,34–2,90× · 0 fora | 1,34–2,90× · 0 fora | **passa** |
| 11a | ≥2,5× | 1,38–5,70× · 7 fora | 1,38–5,16× · 7 fora | defeito conhecido |
| 11b | ≥10% | 11,3–20,9% · 0 fora | 11,3–24,5% · 0 fora | **passa** |

**A leitura das duas colunas é limpa, e é um resultado.** As cinco linhas de composição de
receita (3, 4, 5, 10, 11b) não se moveram um décimo — o `spending_power` não toca receita, e a
medição confirma isso em vez de pressupor. Tudo que se moveu é do lado do **gasto**: a razão
receita ÷ despesa (1), o fôlego (2), a crise (7) e a falência (8). O defeito estava
subestimando a despesa técnica da escada inteira, e as quatro linhas que ele contaminava são
exatamente as quatro que a medem.

**A troca de sinal do critério 7 é a maior.** De 56 crise / 47 colapso para 196 crise / 435
colapso: passou de um mundo em que mais gente adoece do que morre para um em que morre quatro
vezes mais gente do que adoece — que é literalmente a restrição de forma que a nota do critério
7 diz existir para pegar. O critério 8 sobe junto (22,14% do grid da GT3 vendido por temporada)
porque a venda por colapso crônico é o mesmo mecanismo lido a jusante.

**O critério 10 fechou por correção, não por afrouxamento** — de 16–39× para 1,34–2,90×, com a
âncora da 4.1 intocada. E não virou tautologia: a linha do modelo passa por quilometragem e
consumo por km, a âncora passa por consumo por **hora**. Duas derivações independentes que
concordam dentro de 3×. Sobreviveu intacto à re-derivação, como esperado de uma linha física.

~~As três que falham são um defeito só, e ele é do degrau de entrada.~~ **Retirado — a
re-medição derrubou.** O diagnóstico dizia que o critério 2 estourava por cima em duas
categorias de grid pequeno. Com o `spending_power` honesto ele estoura nas **nove**, e nas duas
pontas: 40 equipes acima do teto e 34 abaixo do piso. O defeito não é do degrau de entrada — é
da escada inteira, e a metade de baixo dele é nova.

O placar é `#[ignore]` e **reprova** com as cinco linhas abertas: ele não fica verde com
critério em aberto, e o veredito classifica a falha em vez de perdoá-la.

*(O critério 1 falha sem `porque` preenchido — ele nunca teve um, porque nunca tinha falhado.
Fica registrado como pendência de anotação, não de medição.)*

### 4.3 O que a primeira leitura de tela achou

Quatro defeitos que só olhar encontrou, todos invisíveis a teste: token cru no expandir, preço
unitário **arredondado ao dólar inteiro** (uma linha lia "198 km × $0" e cobrava $96 — a falsa
precisão do redesign ao contrário), "5,0 diárias", e o rodapé **inalcançável** (`max-h` +
`overflow-y-auto` sob `pointer-events-none`: totais e rodapé existiam no DOM sem caminho até
eles, descoberto rolando e não conseguindo).

Combustível na tela: **$182 · 49,5 L × $3,67** num MX-5 de 7 voltas com dois carros. A queixa
que originou o projeto está resolvida.

Três leituras que soam estranhas e ficaram registradas em vez de ajustadas:

1. **Frete é 46% da fatura de uma Mazda Rookie** — 57× o combustível, 4,5× os pneus. Equipe
   brasileira correndo na Europa cai na faixa intercontinental. Realista, e o efeito é que a
   maior linha do degrau de entrada é a que a equipe menos controla.
2. **Snetterton e Rudskogen dão faturas idênticas ao centavo** (mesmos 9 679,4 km). O modelo tem
   três faixas de distância, então a linha não distingue para onde se viajou.
3. **Bônus por resultado de $68 413 numa etapa cuja operação custa $22 862.** Vencer uma corrida
   da categoria de base paga três operações; o saldo da etapa fica em +$92 105 num ano cujo
   custo fixo é $175 120. É a mesma coisa que o critério 2 mede nas Rookies, vista de perto.

### 4.4 O defeito que sobrou: `spending_power` ficou na unidade velha

`decide_car_maintenance` orça a compra de peça em `spending_power ÷ ETAPAS_DE_REFERENCIA`, e
`spending_power` **soma um estoque (caixa) com fluxos anuais** e subtrai compromissos e reserva,
que são múltiplos do operacional anual. A âncora de estoque encolheu de ~24 para 6 meses e
nenhum coeficiente de fluxo acompanhou:

| | caixa | receita | crédito | compromissos | reserva | total |
|---|---:|---:|---:|---:|---:|---:|
| âncora velha | +2,05 | +0,57 | +0,11 | −1,04 | −0,90 | **+0,79** |
| âncora nova | +0,50 | +0,57 | +0,11 | −1,04 | −0,90 | **−0,76** |

*(múltiplos do custo operacional anual, uniforme na escada)*

**A equipe precisa de 15,2 meses de caixa para `spending_power` virar positivo, e a faixa
declarada do modelo vai de 1 a 11.** Cruzando com o critério 2 (10,9–22,1 meses), boa parte da
escada está no ou abaixo do limiar em regime permanente: essas equipes **não compram peça
nenhuma**.

Não tem conserto por constante — zerar a reserva inteira leva de −0,76 para +0,14, um décimo do
que era. Os +0,79 antigos vinham quase todos de um caixa de 24 meses que o redesign eliminou de
propósito. `spending_power` precisa ser **re-derivado na unidade nova**, como foram o
`financial_state` e a fatura.

É a mesma armadilha da 3.3.4 — razão cujos dois lados vêm de âncoras diferentes — encontrada
pela quarta vez, e a primeira em que o lado que se moveu foi o do estoque.

### 4.5 A re-derivação, e o que estava preso no piso

`spending_power` passou a responder a pergunta que a função sempre quis responder — *quanto
esta equipe pode gastar nesta temporada sem se matar* — em três parcelas, cada uma com unidade:

```
folga de caixa       (meses_de_operacao − 3 meses) × custo mensal
resultado projetado  receita − comprometido, ponderado pela confiança do estado
crédito usável       crédito × agressividade do estado
```

| | fórmula | caixa | mediana |
|---|---|---|---:|
| VELHO | antiga | 24 meses | **+0,79** |
| QUEBRADO | antiga | 6 meses | **−0,76** |
| NOVO | re-derivada | 6 meses | **+0,26** |

Break-even: **15,2 → 2,9 meses**, dentro da faixa declarada de 1–11. Quatro mudanças
estruturais, nenhuma de constante:

- **A folga sai de `state::meses_de_operacao`**, a mesma medida que define o estado do time.
  Ela já abate a dívida do caixa, então `debt_pressure` deixou de entrar como termo separado:
  subtrair os dois era **cobrar a dívida duas vezes**.
- **A reserva virou meses**, e reusa `FaixasDeMeses::default().pressionada` — o piso abaixo do
  qual o mundo já declara crise. A afirmação vira verificável: *uma equipe não planeja gastar o
  que a levaria para dentro da crise.* Uma constante própria aqui seria uma cópia que
  envelheceria quando alguém recalibrasse as faixas.
- **O custo comprometido só aparece dentro do resultado projetado.** A folga já é medida em
  meses desse mesmo custo; subtraí-lo outra vez, cheio, pedia à equipe que pré-pagasse a
  temporada duas vezes.
- **A confiança ficou assimétrica.** Resultado projetado positivo entra pela confiança do
  estado; negativo entra inteiro. Antes o desconto caía sobre a receita bruta enquanto o custo
  comprometido era cobrado a 100%, o que fazia a equipe mediana planejar um buraco de meio
  custo operacional **todo ano, para sempre**.

**O que os −0,76 estavam segurando.** Três consumidores grampeiam a razão
`spending_power ÷ operacional`, e com o valor negativo os três ficavam no piso ao mesmo tempo,
para o grid inteiro:

- `finance::salary` grampeia em −0,5 → **todo o grid oferecia o salário mínimo**, e o mercado
  parou de separar quem podia pagar de quem não podia;
- os dois gatilhos de `choose_season_strategy` (0,20 e 0,50) disparavam para todo mundo;
- `spending_score` ficava no −25 permanente.

Um defeito de unidade em uma função virou um mercado de trabalho sem diferenciação. É o segundo
caso, depois do termômetro da 2.9, em que um número lido errado adoeceu um sistema que não tem
nada a ver com ele.

**Por que atravessou duas rodadas sem teste vermelho:** os quatro testes de `spending_power`
eram todos **direcionais** — rico > endividado, colapso < 0. Comparação sobrevive a um nível
errado. Fechado com duas asserções de nível: a equipe no caixa de referência de cada uma das
catorze divisões precisa poder gastar alguma coisa, e o piso de sobrevivência precisa ser a
fronteira entre poder e não poder.

### 4.6 `budget_index` satura — a mesma família, quinta vez

```
effective_money = caixa + spending_power×0,45 + receita×0,25 − dívida×0,35
```

comparado contra uma janela de caixa puro de 10 meses. Depois da re-derivação os três termos
extras já estão **dentro** de `spending_power` — é triplo-contado. Resultado: `budget_index`
**satura em 100 a partir de 9 meses de fôlego**, e o placar da 4.2 mede o mundo em regime a
10,9–22,1 meses. Boa parte do grid lê 100.

A saturação foi prevista e medida uma rodada antes, e a medição refutou a previsão — corretamente,
para o mundo daquele momento. Foi o mundo debaixo dela que mudou.

Ficou **aberto por escolha** naquela rodada: `budget` é lido pela IA de mercado, pela geração
de equipe e pela fama, e redefinir o que ele significa é decisão do mesmo peso que a
re-derivação, num escopo que não era o dela. **Fechado na 4.6.1.**

### 4.6.1 `budget_index` re-derivado sobre a escada de estados

A fórmula virou uma linha: **os meses de operação projetados para o fim da temporada, lidos
pela escada de estados**. `meses_de_operacao` (que já abate a dívida) mais o resultado do ano
convertido para meses; a escada é `FaixasDeMeses`, a mesma que define `financial_state`.

Duas coisas ficaram DE FORA de propósito, e são elas que impedem o índice de ser
`spending_power` com outro nome:

- **o crédito** — a agressividade de crédito é *maior* no estado pior (0,75 em crise contra
  0,10 na elite). Um índice que somasse crédito premiaria a doença;
- **a ponderação de confiança do estado** — postura é o que a equipe decide, não o que ela é.
  Sem ela, o índice não lê `financial_state`, e metade da circularidade da 2.4 se desfaz.

**O mapa não tem constante calibrada.** Cada um dos seis estados ocupa uma fatia igual da
escala (100 ÷ 6), linear dentro da banda: 50,0 é o piso de `estavel`, 66,7 o de `saudavel`,
83,3 o de `elite`. Quem recalibrar as faixas move o índice junto, em vez de fundar uma segunda
calibração em paralelo. As duas bandas abertas (colapso e elite) usam `x ÷ (x + largura)` com
a largura da banda fechada vizinha — **0 e 100 viraram assíntotas, não valores**. É a
propriedade estrutural que impede o empilhamento, e não uma escolha de coeficiente.

**Medido, mesmo binário, `relatorio_distribuicao_do_budget_index`:**

| população | | mín | Q1 | med | Q3 | máx | em 0 | em 100 |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| regime (4.7, 27 pontos) | VELHO | 51,6 | 100,0 | 100,0 | 100,0 | 100,0 | 0 | **23/27** |
| | NOVO | 28,0 | 61,4 | 71,7 | 85,7 | 96,9 | 0 | **0/27** |
| nascimento (14 divisões) | VELHO | 20,8 | 49,8 | 93,3 | 100,0 | 100,0 | 0 | **70/154** |
| | NOVO | 15,4 | 26,4 | 43,0 | 54,9 | 60,4 | 0 | **0/154** |
| grid gerado (68 equipes) | VELHO | 16,5 | 47,8 | 76,2 | 96,9 | 100,0 | 0 | **14/68** |
| | NOVO | 9,8 | 20,4 | 34,3 | 43,5 | 64,1 | 0 | **0/68** |

**Vinte e três dos vinte e sete pontos do mundo em regime liam exatamente 100.** O índice novo
usa 69 pontos da escala na mesma população e não encosta em ponta nenhuma.

O grid de nascimento ficar na metade de baixo (15–60) **não é defeito do índice, é o modelo
falando**: a faixa de caixa declarada vai de 1 a 11 meses e o mundo em regime roda a 10,9–22,1.
O grid nasce pobre e enriquece correndo. Vale registrar que **só seis categorias têm equipe de
nascimento** — as outras oito divisões recebem equipe por promoção.

#### O que os consumidores fizeram com isso — `relatorio_consumidores_do_budget_index`

| consumidor | VELHO | NOVO |
|---|---|---|
| `fame::team_need_factor` | 0,412–0,660, **13 de 15 no mesmo 0,412** | 0,429–0,801, todos distintos |
| patrocínio (termo do índice) | 0,1936 | 0,1439 — **−25,7%**, ou −7,3% do canal |
| `pit_strategy`, bônus abaixo de 30 pts | 0/15 (nunca disparava) | 1/15 (a equipe de 3,3 meses) |

**A queda do patrocínio é o custo assumido desta mudança e é real:** −7,3% de um canal que a
seção 3.7 diz valer ~70% da receita, ou seja ~5% da receita do grid. Não foi compensada por
constante — o coeficiente 0,002 continua parado, pelo mesmo motivo que o `patrocinio_base`
continua em 0,27: é alvo da sessão de receita, e maquiá-lo aqui esconderia o deslocamento.

#### `promotion::effects` — o rótulo que não bate mais com a escala

`budget_delta` é declarado em PONTOS de índice (+5 a +15 ao subir). Pela escada nova, dez
pontos perto de `saudavel` valem ~7 meses de operação; o pacote paga **0,35**. O pacote é
~20× menor do que o próprio nome dele afirma.

Não é regressão: a fórmula antiga (`janela_de_caixa × delta/100 × 0,35`) nunca leu a escala do
índice, só a janela — e a janela é `11 − 1 = 10` meses por construção, então ela sempre valeu
`0,035 × delta` meses. Foi reescrita nessa unidade, **magnitude idêntica ao centavo**, com o
descompasso documentado. Corrigi-lo é calibração do anti-snowball, não conserto de unidade.

#### Os testes de NÍVEL

O precedente da 4.5 — quatro asserções direcionais deixaram passar um `spending_power`
negativo na escada inteira — se repetiria aqui: `100 > 100` nunca foi perguntado, e nenhuma
asserção de ordem podia ver a saturação. Fechado com:

- **dispersão sobre o mundo em regime**: nada abaixo de 1 nem acima de 99, e o alcance
  ocupando mais de 50 pontos da escala;
- **as fronteiras caem em pontos declarados** (50,0 / 66,7 / 83,3);
- **a escada concorda com a binagem** em toda a varredura — o índice cai na fatia do estado
  que `estado_por_meses` declara, senão a equipe seria exibida por um instrumento e lida por
  outro;
- **o índice não é transformação monotônica de `spending_power`**: uma equipe em crise tem
  MAIS poder de gasto que uma estável com o mesmo caixa (crédito), e exatamente o mesmo índice.

### 4.7 O critério 2 mede a mediana, e a mediana não descreve quase ninguém

A pergunta era se a Rookie estava rica. A resposta é que **a Rookie é o único grid uniforme da
escada inteira**, e que o critério que a reprova é cego em sete das nove categorias.

| categoria | grid | campeão | meio | lanterna | camp÷lant | campeões distintos |
|---|---:|---:|---:|---:|---:|---:|
| mazda_rookie | 6 | 19,8 | 22,1 | 20,2 | **1,0×** | **6/6** |
| toyota_rookie | 6 | 18,0 | 20,5 | 18,5 | **1,0×** | **6/6** |
| mazda_amador | 10 | 76,7 | 3,3 | 10,5 | 7,3× | 3/10 |
| toyota_amador | 10 | 62,8 | 11,4 | 15,3 | 4,1× | **1/10** |
| bmw_m2 | 10 | 61,2 | 16,6 | 14,3 | 4,3× | 3/10 |
| production | 18 | 66,1 | 14,3 | 6,7 | 9,9× | 4/18 |
| gt4 | 10 | 78,4 | 12,6 | 6,3 | **12,5×** | 4/10 |
| gt3 | 14 | 46,4 | 16,9 | 9,6 | 4,9× | 4/14 |
| endurance | 18 | 43,2 | 14,8 | 9,2 | 4,7× | 11/18 |

**O mecanismo é a persistência do vencedor, não o tamanho do grid.** Uma curva convexa só
concentra dinheiro sobre alguém que **continua ganhando**. Na Rookie as seis equipes foram
campeãs e nenhuma passou de 30% dos títulos em 20 temporadas, então o prêmio grande vai para
outra todo ano e se redistribui sozinho. Nas outras a corrida trava — 3 ou 4 campeões num grid
de 10 a 18, uma delas segurando 70–100% — e aí a convexidade tem em quem se acumular.

**Isso mata a hipótese de γ por tamanho de grid.** O tamanho não prediz nada: gt4 tem 10 e
12,5×, endurance tem 18 e 4,7×, a Rookie tem 6 e é a mais uniforme de todas. A alavanca de γ
segue esgotada — agora por motivo medido e não por eliminação. Concentrar a Rookie exigiria
criar vantagem **permanente** de desempenho no degrau de entrada, o equivalente a dar fábrica
para duas das seis; é decisão sobre a porta da pirâmide, não calibração.

**A inversão que fecha a pergunta original:** o campeão da Rookie tem o **menor** saldo anual do
grid ($11.144) e o 4º colocado tem o maior ($18.454). O dinheiro de ganhar vira carro melhor e
folha maior — `calculate_offer_salary_from_money` lê o caixa — não banco. Vencer ali compra
competitividade, não patrimônio. As duas medições que pareciam brigar são compatíveis, e a
ponte é a escala de tempo: uma etapa de uma equipe contra 20 temporadas de um grid.

**O achado que vale mais que a pergunta.** A gt4 passa o critério 2 com mediana de 12,6 meses,
com campeão em 78,4 e lanterna em 6,3. A production passa com 14,3, campeão em 66,1, lanterna
em 6,7. **O critério passa onde não está medindo nada e reprova onde mede corretamente** — a
Rookie é a única categoria em que a mediana é o retrato do grid, e é uma das duas que reprovam.

A forma que ele deveria ter é uma **banda sobre a distribuição** — piso no lanterna, teto no
campeão — em vez de um ponto no meio. Nessa forma, sete categorias que hoje passam reprovariam.
Isso não é aumentar o rigor: um lanterna de gt4 com 6,3 meses de fôlego é exatamente o que o
critério 2 existe para pegar, e ele está passando despercebido debaixo de uma mediana saudável.

Dois casos com nome próprio: **toyota_amador tem uma equipe que ganhou os 20 títulos** —
dinastia sem sucessão, e a dispersão dela só não é maior porque as outras nove são
uniformemente pobres. E a **gt3 tem um degrau de 4× entre 2º e 3º** ($18,3 M contra $4,8 M):
duas fábricas se descolaram e a terceira não.

#### A re-medição com `spending_power` re-derivado

~~A dispersão relativa provavelmente sobrevive; os níveis absolutos não.~~ **A previsão estava
invertida.** Os níveis do **topo** é que sobreviveram quase intactos; o meio e o fundo
desabaram, e a dispersão **triplicou ou pior** em toda categoria que não fosse Rookie.

| categoria | grid | campeão | meio | lanterna | camp÷lant |
|---|---:|---:|---:|---:|---:|
| mazda_rookie | 6 | 19,8 → **18,1** | 22,1 → **20,9** | 20,2 → **18,4** | 1,0× → **1,0×** |
| toyota_rookie | 6 | 18,0 → **16,7** | 20,5 → **18,2** | 18,5 → **16,5** | 1,0× → **1,0×** |
| mazda_amador | 10 | 76,7 → **55,4** | 3,3 → **0,4** | 10,5 → **0,4** | 7,3× → **137,9×** |
| toyota_amador | 10 | 62,8 → **86,6** | 11,4 → **1,2** | 15,3 → **4,6** | 4,1× → **18,6×** |
| bmw_m2 | 10 | 61,2 → **73,6** | 16,6 → **8,9** | 14,3 → **1,9** | 4,3× → **39,6×** |
| production | 18 | 66,1 → **67,5** | 14,3 → **3,1** | 6,7 → **−5,5** | 9,9× → **12,2×** |
| gt4 | 10 | 78,4 → **66,9** | 12,6 → **6,0** | 6,3 → **2,0** | 12,5× → **33,4×** |
| gt3 | 14 | 46,4 → **40,5** | 16,9 → **−1,0** | 9,6 → **2,0** | 4,9× → **20,3×** |
| endurance | 18 | 43,2 → **23,1** | 14,8 → **12,4** | 9,2 → **2,5** | 4,7× → **9,2×** |

**O controle que fecha a atribuição: a rotação de campeão é IDÊNTICA nas duas rodadas** —
6/6, 6/6, 3/10, 1/10, 3/10, 4/18, 4/10, 4/14, 11/18, com os mesmos percentuais de título. O
lado esportivo do harness não se moveu um ponto. Tudo que mudou entre as duas tabelas passou
pelo caminho do dinheiro, e nada passou pelo caminho da corrida.

**O mecanismo.** Com `spending_power` negativo, todo o grid comprava no piso: salário mínimo,
nenhuma peça, os dois gatilhos de estratégia disparados. Corrigido, a despesa técnica real
apareceu — e ela é **regressiva no fôlego**. O campeão tem excedente para absorvê-la; o meio e
o fundo não têm, e a diferença sai do caixa deles todo ano. É por isso que o topo mal se moveu
e o resto caiu para o piso.

**A dinastia da toyota_amador ficou pior, não melhor.** A equipe de 20/20 títulos foi de 62,8
para **86,6 meses** enquanto o meio da tabela dela caiu de 11,4 para **1,2**. Ela é a única
categoria em que o campeão *ganhou* fôlego na re-medição, e é a que tem o vencedor mais
persistente da escada — a convexidade tem exatamente um lugar em que se acumular, e agora tem
mais o que acumular.

**O degrau de 4× da gt3 sumiu, e o que ficou no lugar é pior.** Entre 2º e 3º agora há 1,3×
($15,8 M contra $12,0 M): as fábricas se aproximaram. A descontinuidade migrou para **entre o
6º e o 7º**, e ali não é degrau de receita (1,5×) — é degrau de **solvência**: o 6º fecha o ano
em +$17 mil, o 7º em **−$1,40 milhão**, e daí para baixo as oito equipes restantes têm receita
praticamente igual (~$2,8 M) e saldo entre −$0,96 M e −$1,72 M. A conta fixa salta de 57% para
81% da receita na mesma linha. O grid da GT3 não é mais uma escada: são seis equipes solventes
e oito mortas empatadas.

### 4.8 O frete passou a distinguir destino

`constants/geografia.rs`: distância de grande círculo entre o país da equipe e o país do
circuito, 34 países. Os pontos **não são centroides** — são onde a corrida acontece naquele
país. O centroide da Noruega fica a 64,5°N e Rudskogen a 59,4°N, 600 km ao sul. Sem coordenada,
cai nas três faixas antigas: falta de dado não vira número inventado. Um guard falha se um país
novo entrar no catálogo sem coordenada.

Sobre os 10.914 pares equipe × pista: distância média 5.853 → 6.260 km (**+7,0%**), e destinos
distintos por sede saltam de no máximo 3 para **16,9 em média**. Duas consequências assumidas: o
mundo ficou 7% mais caro em logística, e a ponta longa cresceu muito mais que a média —
Brasil→Japão dobrou, de 8.500 para 18.596 km. A faixa única achatava um intervalo real de 2,6×
entre o transatlântico curto e a volta ao mundo.

O empate era visível: Snetterton e Rudskogen davam faturas idênticas ao centavo. Um jogador
atento vê duas corridas em países diferentes cobrarem o mesmo e conclui que o número é
decorativo — que é a percepção que este projeto existe para consertar. Verificado na tela em
três etapas: $10.454 → $9.912 → $9.597.

**O frete ser 46% da fatura de uma Rookie fica como está.** Equipe brasileira correndo na
Europa é assim mesmo.

### 4.9 O critério 2 virou banda sobre a distribuição

A forma nova está em `criterios_de_aceitacao`: **toda equipe do grid entre o piso e o teto**,
em vez da mediana entre eles.

**A conta dos dois limites — e nenhum dos dois é número novo.**

| lado | valor | de onde vem | o que significa |
|---|---:|---|---|
| piso | **3 meses** | `FaixasDeMeses::default().pressionada` | a fronteira em que o **próprio mundo** declara a equipe pressionada e `financial_state` arma empréstimo de emergência |
| teto | **18 meses** | 2 × `ParametrosDeDesenvolvimento::meses_de_reserva` (9) | o **dobro** do que a equipe escolheu guardar; o ralo drena 40% do que passa da reserva, e chegar ao dobro dela é o ralo não ter drenado |

Os dois são **os mesmos 3 e 18 do alvo anterior, de propósito**. O que mudou nesta rodada foi
a estatística; mexer nos limites junto tornaria impossível dizer qual das duas coisas moveu o
placar. E os dois passaram a ser **lidos das constantes** em vez de escritos no teste — uma
cópia envelheceria calada no dia em que alguém recalibrasse as faixas, que é exatamente a
armadilha da 4.4 pela quinta vez.

Uma diferença de definição que vale registrar: o teste checa a **distribuição inteira**, não o
par (campeão, lanterna) ordenado por pontos. Quem termina mais pobre nem sempre é quem terminou
em último — na gt3 o 8º e o 10º de pontos terminam abaixo do 14º —, e o que o critério protege
é a equipe pobre, não a posição dela na tabela.

**O resultado: 9 de 9 categorias reprovam, e nas duas pontas.** 34 equipes abaixo do piso, 40
acima do teto, de 102.

| categoria | pior | mediana | melhor | abaixo | acima | o que a mediana escondia |
|---|---:|---:|---:|---:|---:|---|
| mazda_rookie | 16,7 | 20,9 | 27,3 | 0 | 5 | reprova nas duas formas |
| toyota_rookie | 16,5 | 18,2 | 31,1 | 0 | 4 | reprova nas duas formas |
| mazda_amador | −1,2 | 2,0 | 55,4 | 7 | 3 | reprova nas duas formas |
| toyota_amador | −1,3 | 4,6 | 86,6 | 4 | 4 | **a mediana passa e o grid não** |
| bmw_m2 | 1,9 | 11,9 | 73,6 | 2 | 3 | **a mediana passa e o grid não** |
| production | −5,5 | 6,7 | 68,8 | 6 | 6 | **a mediana passa e o grid não** |
| gt4 | −3,5 | 11,1 | 66,9 | 4 | 3 | **a mediana passa e o grid não** |
| gt3 | −6,7 | 2,0 | 49,8 | 8 | 4 | reprova nas duas formas |
| endurance | −0,1 | 15,6 | 36,1 | 3 | 8 | **a mediana passa e o grid não** |

Na forma velha seriam **4** categorias fora (as quatro marcadas "nas duas formas"). As outras
**cinco** são a cegueira medida: mediana confortável, grid partido embaixo dela. A gt4 tem
mediana de 11,1 meses com quatro equipes negativas e três acima de 66; a endurance tem 15,6 com
oito acima do teto. Nenhuma das duas estava dizendo nada.

#### As duas perguntas, separadas

Elas se misturam na mesma linha do placar e pedem coisas diferentes.

**(a) O critério tinha a forma errada.** Está consertado, e o conserto é atribuível: os limites
não se mexeram, a estatística sim. Um critério cego em cinco das nove categorias que avalia não
era um critério frouxo — era um critério medindo a grandeza errada, do mesmo jeito que o 11
media duas estatísticas sem escolher uma e o 7 media crise sem distinguir de colapso. É a
terceira vez neste projeto que o instrumento, e não o mundo, era o problema.

**(b) O mundo está doente na ponta de baixo.** Isso é verdade e **não é decisão do teste**. 34
de 102 equipes terminam 20 temporadas abaixo do piso de crise, sete categorias têm equipe com
fôlego negativo, e a GT3 tem oito das catorze empatadas em prejuízo anual de ~$1,5 M. Não é a
banda que criou esse número — a forma velha já o teria mostrado se olhasse para lá, e os
critérios **7** (435 colapsos contra 196 crises) e **8** (22,14% do grid vendido por temporada)
já o estavam medindo por dois outros caminhos independentes. **São os três a mesma doença**, e
o nível de receita que a cura é decisão de calibração — a mesma que o item 11 da Parte 6
reserva para depois do ralo.

O que a forma nova acrescenta em (b) é **onde** doer: a doença não está distribuída, está
concentrada no fundo de cada grid, e um corte uniforme de receita a piora (medido: nível 1,00 →
0,50, todo corte mata o fundo antes de corrigir o topo). O botão que falta é **redistributivo**,
não de nível.

#### O que a re-medição matou por engano de previsão

A nota da 4.7 dizia que a dispersão relativa provavelmente sobreviveria e os níveis absolutos
não. Saiu ao contrário: **os níveis do topo sobreviveram e a dispersão explodiu.** Registrado
como previsão errada e não corrigido para trás — é a segunda vez neste documento que uma
previsão medida uma rodada antes foi refutada pelo mundo ter mudado debaixo dela (a primeira
foi a saturação do `budget_index`, 4.6), e o padrão vale mais que o acerto.

### 4.10 O botão não é redistributivo — a equipe pobre não tem como encolher

A 4.9 pede um botão redistributivo. **É o enquadramento errado**, e o código diz por quê em uma
struct:

```rust
pub struct EquipeNaTemporada {
    pub instalacoes: f64,   // e mais nada
}
```

`fatura_de_temporada` monta a folha com `fisico.equipe_fixa` (headcount) e
`p.salario_medio_anual` — **os dois saem da tabela por categoria/classe, nenhum sai da equipe**.
`instalacoes` escala só a linha de sede, de 0,60 a 1,40, e a sede é a menor das linhas fixas.

Ou seja: **toda equipe de GT3 paga folha de 24 pessoas a $68.000, a fábrica de $18,3 M de
receita e a oitava colocada de $2,8 M.** É exatamente o retrato que a 4.9 mediu — as oito de
baixo empatadas em ~$2,8 M de receita com conta fixa em 81–90%.

Isso identifica o **freio automático** da 3.3.5 pela terceira vez, e agora com a causa
mecânica em vez do sintoma. A autópsia da gt4 já dizia a frase inteira sem que ninguém a
lesse assim: *estrutura 78,6% da receita, juros 53,0%, técnica **0,0%** — já cortou tudo que
dava*. Técnica é o único bloco variável que sobrou, e ela é pequena demais para ser freio.
Uma equipe a 85% de conta fixa pode zerar a técnica e ainda assim morrer.

**Por que apareceu só agora:** com o `spending_power` quebrado ninguém comprava peça, a técnica
era ~0 para o grid inteiro e a despesa real ficava abaixo da verdadeira. A 4.5 devolveu a compra
e o fundo de cada grid encontrou a parede que já estava lá.

**A hipótese a medir** — e é hipótese, não decisão travada: headcount e salário médio passam a
ser função da **equipe**, não da categoria. Equipe pobre roda com menos gente e paga menos, que
é o que uma equipe de corrida de cliente faz de verdade quando o ano é ruim. Isso restaura o
freio sem tocar em receita — e a 3.3.5 já mediu que mexer em receita não resolve (corte
uniforme mata o fundo antes de corrigir o topo).

Duas coisas a verificar antes de acreditar: se encolher a folha também piora o desempenho, o
freio vira espiral em vez de amortecedor — precisa de piso; e a escada de 20,4× da 3.3.3 sai da
folha técnica (`4 pessoas × 28k` contra `30 × 78k`), então headcount variável por equipe mexe na
âncora que ancora tudo. Medir a escada antes e depois é obrigatório.

### 4.11 Este documento contém números de mais de um mundo

A extração do modelo velho rodou o comparador três vezes. Runs 2 e 3 bateram dígito a dígito; o
run 1 divergiu. A sessão provou que a causa não era dela por quatro caminhos independentes —
`find -newermt` mostrando `finance/planning.rs` (00:53:38), `finance/state.rs` (00:54:02) e
`promotion/effects.rs` (00:52:52) modificados por outras sessões entre o run 1 (00:47:59) e o
run 2; o padrão do desvio poupando exatamente as linhas que não dependem de
`facilities`/`pit_crew`/`engineering`; a queda geográfica provada neutra `to_bits()` em 1.000+
pares; e a cópia congelada com o mesmo multiconjunto de constantes do original.

**A consequência é do documento, não do teste.** As medições da Parte 4 foram tomadas ao longo
de várias rodadas, com três sessões editando o crate em paralelo, e nem toda tabela aqui
descreve o mesmo mundo. A tabela de abertura da Parte 4 tem uma linha na forma nova (critério 2
como banda) ao lado de linhas medidas antes do redesign — um leitor não consegue dizer, olhando,
de que época é cada número.

Regra que fica: **toda tabela nova declara em que rodada foi medida.** E qualquer comparação
entre duas colunas só vale se as duas saíram do mesmo binário — o que já era regra para
velho-contra-novo e agora vale para o documento inteiro.

### 4.12 Inventário de órfãos — levantado, não apagado

Órfão de verdade e seguro: `get_overall_bonus` em `constants/scoring.rs` (a 2.7 sentiu o cheiro
nas constantes `BONUS_OVERALL_*` e errou o alvo por um nível — as constantes são lidas, quem não
tem chamador é a função) e `calculate_gate_income` em `finance/cashflow.rs`, cujo `_with` é o que
a produção usa.

Parece órfão e não é: `meses_na_posicao` (`state.rs`) tem chamador a caminho em
`promotion::effects`; `safety_reserve_multiplier_for_state` vive dentro da fórmula congelada
`spending_power_legado` e é história; `DespesaDaRodada.tecnica` vale sempre `0.0` em produção e
fica porque é o que dá aos dois lados do comparador a mesma forma.

**21 comandos Tauri sem nenhuma ocorrência em `src/`.** Nenhum será apagado por ora: os
`engenheiro_*`, `ptt_gatilho_atual` e os onze `iracing_*` são exatamente a área em obra, e as
janelas `overlay`/`engineer` são pontos de entrada separados que um grep sobre `src/` não
distingue de código morto. Ausência de string literal não é ausência de chamador quando o nome
pode ser montado.

```
advance_transfer_window   get_driver   get_race_reading
get_race_results_by_category   get_window_maximized   toggle_maximize_window
overlay_window_set_interactive   engenheiro_catalogo   engenheiro_classificar
engenheiro_dossie_completo   ptt_gatilho_atual   iracing_read_session
iracing_read_telemetry   iracing_log_caminho   iracing_poll_race
iracing_estado_agora   iracing_reset_race   iracing_career_race_result
iracing_throw_yellow   iracing_send_chat_macro   iracing_spotter_restore
```

> ⚠️ **Esta lista é um retrato de quando o documento foi escrito, e quatro nomes dela já não existem.**
> `advance_transfer_window`, `get_driver`, `get_window_maximized` e `toggle_maximize_window` foram
> removidos do `generate_handler!` **e do crate** em 11/08/2026 (registro em
> [divida-tecnica.md](divida-tecnica.md)). O inventário vivo não é mais uma lista em prosa: está
> congelado em `SEM_CONSUMIDOR_CONHECIDO`, no guard
> [`invoke-contra-generate-handler`](../scripts/tests/invoke-contra-generate-handler.test.mjs), que
> quebra quando a lista muda. Rode `node --test scripts/tests/invoke-contra-generate-handler.test.mjs`
> em vez de conferir contra este bloco.

## Parte 5 — Decisões travadas

1. **O jogador só pilota.** A economia é simulação de mundo, não camada de gestão. Nenhuma
   linha de despesa vira decisão dele.
2. **O piloto não tem dinheiro pessoal.** `salario_anual` segue sendo custo da equipe e não
   vira carteira de ninguém.
3. **Patrocínio fica fora do radar do jogador.** Continua fórmula automática — mas sem a
   realimentação de riqueza da seção 2.4.
4. **Fatura: 4 blocos, 8 linhas visíveis** (seção 3.7). O modelo interno segue detalhado.
   A aritmética: 8 travadas aqui, −1 pelo fim do rateio (decisão 10), +1 pela **peça comprada**
   = 8. *(Eu disse 9 em duas rodadas seguidas carregando o número sem refazer a conta; a
   constante hoje traz a aritmética escrita ao lado dela.)* O reparo não entra no teto porque
   tem uma linha **por peça danificada** — até seis — e por isso é bloco à parte no DTO.

   A peça comprada não deixou margem para virar rodapé: **96–100% das rodadas têm compra em
   todas as catorze divisões**, e do GT3 para cima ela vale em média mais que a fatura visível
   inteira. Rodapé para o maior item da página é letra miúda. O número é piso — troca forçada
   por quebra e por contato destrutivo não entra nele.

   | divisão | rodadas com compra | peça ÷ fatura | pior rodada |
   |---|---:|---:|---:|
   | Rookie | 96% | 30% | 44% |
   | Amador | 98% | 40% | 58% |
   | BMW M2 | 100% | 68% | 76% |
   | Production | 98% | 73–81% | 88% |
   | GT4 | 98% | 89% | 110% |
   | **GT3** | 96% | **109%** | **135%** |
   | LMP2 | 98% | 119% | 136% |
   | Endurance | 97% | 43–65% | 74% |

   Medido no **ponto de projeto** (caixa de 6 meses). A tabela anterior — 151% e 333% na GT3 —
   estava certa a 16 e 24 meses, que era o único caixa onde o `spending_power` quebrado ainda
   comprava alguma coisa. A frequência não se moveu entre os dois pontos; só os picos caíram.
   Uma tabela medida no único regime em que o sistema funcionava não descreve o regime em que
   ele vai rodar.

5. **Os dois rótulos de peça não podem convergir.** `revisao_mecanica` (era `desgaste_de_peca`)
   é desgaste amortizado por km, existe em toda etapa e é pequeno; `peca_de_reposicao` é a
   **compra**. Um teste crava que os dois tokens não compartilham nenhuma palavra. Se os nomes
   se parecerem, o jogador soma as duas.
6. **A fama entra junto** com a economia, na forma da seção 3.6.
7. **O investimento em estrutura não tem teto.** Medido: todo teto que morde devolve a deriva
   onde ela era pior, e o que não morde é indistinguível de não existir. Quem limita o
   crescimento é o retorno saturante, não um limite de gasto.
8. **A escada de custo é de ~21×, não de 89×**, e o Endurance passa a ser orçado **por
   classe** — o midpoint único de 16,5 M cobrava de uma GT4 preço de LMP2. Ver 3.3.3.
9. **A bilheteria não tem piso igualitário** (`bilheteria_piso = 1,0`, cota inteiramente por
   atração). Medido: com fama de hoje nem uma cota 100% por atração passa do 1,68×, e com
   σ≈15 mas piso igualitário de 65% dá 1,65×. O critério 11 só passa com **as duas** coisas.
10. **Patrocínio é ~20% da receita, não o piso de sobrevivência** — ver 3.5, canal 3.
11. **O rateio da folha fixa sai da fatura pós-corrida** e vai para o fechamento de temporada,
    com uma linha de rodapé na etapa dizendo quanto é o custo fixo do ano. Medido: o rateio
    ocupa **70–84%** da fatura da etapa, então as sete linhas físicas que o redesign tornou
    honestas somam 16–30% do que o jogador lê. Folha e sede não variam por corrida — mostrá-las
    por corrida é a mesma falsa precisão que o redesign existe para remover.
12. **A reserva do ralo é 9 meses, e não é a fronteira `saudavel` de 12.** As duas deixaram de
    ser o mesmo número de propósito: uma é o que a equipe **escolhe guardar**, a outra é como o
    estado **é lido**. Medido: `meses_de_reserva = 12` era o piso do fôlego de toda equipe
    saudável do mundo — um ralo que se recusa a descer de 12 garante que ninguém termine 20
    temporadas abaixo disso, e a faixa do critério 2 é 3–18. Era ali que a sobra estava, não na
    receita.
13. **A estrutura deprecia 2% ao ano.** É o único débito do jogo que escala com riqueza, e sem
   ele o ralo seca justamente na equipe mais rica. `depreciacao_anual` é o botão de acabamento
   do critério 7 e só deve ser gasto depois da seção 3.5.

### O que "só pilota" implica no desenho

Não é uma decisão neutra — ela redefine para que serve a economia.

- **A economia existe para o mundo ser consequente, não para o jogador otimizar.** O que ela
  precisa entregar é: equipes que sobem e caem por motivo legível, um assento que fica em
  risco quando a equipe quebra, e uma prestação de contas que soe verdadeira.
- **Riqueza tem que se converter em algo que o jogador SINTA na pista.** Se dinheiro não vira
  decisão dele, precisa virar carro melhor, dupla melhor e assento mais disputado — senão é
  um número que passa na tela. Isso põe peso no acoplamento caixa → nível de carro → ritmo,
  que hoje existe mas é fraco.
- **O modelo pode ser mais rico do que a tela.** Sem painel de gestão, não há custo de
  interface em modelar 20 linhas: elas alimentam o comportamento da IA e o harness, e chegam
  ao jogador agrupadas.
- **Falir tem que doer no lugar certo.** Como o jogador não administra, a consequência de uma
  equipe quebrar não pode ser um `if` que injeta caixa (seção 2.5) — tem que ser perder o
  carro bom, perder o companheiro de equipe, e ele receber proposta de fuga. A falência é um
  evento narrativo, e hoje ela é invisível.

---

## Parte 6 — Ordem de trabalho

### Restrição de ordem descoberta na execução: o ralo vem ANTES da calibração da receita

Não é preferência, é o sinal da derivada. Rodando a busca do par (patrocínio × escala do
bônus) nos dois regimes:

| | menor espalhamento da escada | par que o produz |
|---|---:|---|
| sem ralo | 0,55 | patrocínio 0,10 × bônus 1,00 |
| com ralo (excedente 0,40) | **0,17** | patrocínio 0,27 × bônus 3,00 |

O espalhamento cai 69% — mas o que decide a ordem é a **inversão de sinal**. Sem ralo,
aumentar a escala do bônus *alarga* a escada (0,55 → 1,33 conforme o bônus vai de 1,0 a 3,0).
Com ralo, aumentar o bônus a *estreita* (0,37 → 0,29). Calibrar a receita antes do ralo é
procurar o mínimo de uma função com o sinal trocado — a calibração daria a resposta oposta à
correta, e pareceria certa.

É também a primeira evidência medida de que a escada **pode** ficar plana.


1. Escrever os critérios da Parte 4 como teste no harness — falhando.
2. `economia/ancora.rs` + `economia/tipos.rs`: a âncora nova e os structs de entrada/saída.
3. `economia/evento.rs`: a fatura bottom-up. É o maior salto de qualidade percebida.
4. ~~`economia/temporada.rs`: os recorrentes~~ — **feito**. Fechou a âncora anual total contra
   total (3.3.3) e derrubou a hipótese dos custos categóricos.
5. ~~`economia/desenvolvimento.rs`: o ralo~~ — **feito**. Reproduz a forma C em 9/9, sem teto,
   com depreciação de 2%.
6. ~~`economia/receita.rs`: os cinco canais~~ — **feito em forma, pendente em nível.** Achou os
   conflitos entre critérios e as leis de escala; os coeficientes estão pinados na despesa
   medida, que muda quando a âncora nova entrar.
7. ~~Fama: piso pessoal + composto de público~~ — **feito e em produção.** Atração espalha
   2,70–5,13×, critério 11 coberto nas 9 categorias.
8. ~~Reancorar `derive_financial_state` em meses~~ — **feito.** Revelou que o termômetro
   adoecia o mundo, e que a crise honesta era 0,6%.
9. ~~Trocar a âncora de fluxo~~ — **feito.** `category_finance_scale` por divisão,
   `category_cost_scale` descongelada, `representative_division_for_tier` no lugar do mapa por
   tier, escada salarial comprimida de 89,2× para 20,4×.
10. **Trocar a âncora de estoque** (`expected_cash_midpoint`): 8 consumidores, incluindo o
    patrocínio, que é 70% da receita ancorado num estoque. Última peça estrutural.
11. Recalibrar a receita sobre a âncora nova, com o critério 8 amarrando.
12. `economia/fatura.rs`: os 4 blocos e 8 linhas da seção 3.7.
13. Trocar a chamada em `race/persistencia.rs`, migrar o ledger, atualizar o dossiê.
14. Rodar o harness, ajustar até os critérios passarem, apagar o modelo velho.
