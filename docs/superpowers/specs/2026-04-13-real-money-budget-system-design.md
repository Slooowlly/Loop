# Real Money Budget System Design

## Goal

Transformar o sistema financeiro das equipes para trabalhar diretamente com valores de dinheiro real, em vez de usar `budget` 0-100 como fonte principal de decisao.

O usuario deve enxergar dinheiro acumulado de forma clara, mas nao precisa gerenciar manualmente. As equipes, por outro lado, usam esse dinheiro para tomar decisoes profundas: investir em carro, contratar pilotos, melhorar pit crew, segurar caixa, aceitar divida, apostar contra rebaixamento ou construir uma subida futura.

## Core Principle

`cash_balance` passa a ser a fonte de verdade financeira.

`budget` deixa de ser criterio principal de IA. Durante a migracao, ele pode continuar existindo como campo legado ou indice derivado, mas novos sistemas devem consultar funcoes financeiras explicitas baseadas em dinheiro real.

```text
cash_balance
- debt_balance
+ projected_income
- committed_costs
= contexto financeiro real da equipe
```

## Category Money Scale

O dinheiro sempre deve ser interpretado relativo a categoria. Uma equipe com R$ 3 milhoes e rica na base, mas apertada no GT3.

```text
Mazda Rookie / Toyota Rookie: R$ 100 mil a R$ 700 mil
Mazda/Toyota Amador: R$ 250 mil a R$ 1,5 milhao
BMW M2 / Production: R$ 750 mil a R$ 4 milhoes
GT4: R$ 2 milhoes a R$ 9 milhoes
GT3: R$ 6 milhoes a R$ 25 milhoes
Endurance: R$ 12 milhoes a R$ 60 milhoes
```

Esses intervalos nao sao caps absolutos. Eles definem a escala esperada para calcular riqueza relativa, custos, credito e risco.

## Financial Inputs

Cada equipe deve conseguir gerar um plano financeiro operacional com estes dados:

```text
cash_balance
Dinheiro acumulado em caixa. Pode ficar negativo ate um limite controlado.

debt_balance
Divida total. Aumenta juros, reduz saude financeira e limita credito futuro.

projected_income
Receita esperada ate o fim da temporada: patrocinio, premio provavel, bonus e auxilio.

committed_costs
Custos assumidos: salarios, operacao de corrida, manutencao, estrutura e divida.

safety_reserve
Reserva minima que a equipe tenta preservar, variando por estado financeiro e ambicao.

available_credit
Quanto ainda consegue tomar emprestado antes de entrar em risco severo.

parachute_payment_remaining
Auxilio restante para equipes rebaixadas.
```

## Spending Power

`spending_power` representa quanto a equipe pode gastar de forma realista, considerando caixa, futuro, risco e divida.

```text
spending_power =
  cash_balance
  + projected_income * income_confidence
  + parachute_payment_remaining
  + available_credit * credit_aggressiveness
  - committed_costs
  - debt_pressure
  - safety_reserve
```

### Income Confidence

```text
Elite/saudavel: 0.75 a 0.90
Estavel: 0.60
Pressionada: 0.45
Crise/colapso: 0.25 a 0.35
```

Equipes saudaveis confiam mais em receita futura. Equipes em crise nao podem contar tanto com dinheiro que talvez nao venha.

### Credit Aggressiveness

```text
Elite: 0.10
Saudavel: 0.20
Estavel: 0.30
Pressionada: 0.55
Crise: 0.75
Colapso: 0.40
```

Equipes pressionadas e em crise aceitam se endividar. Equipes em colapso nao usam 100% do credito porque o mercado ja nao confia nelas.

## Financial States

Os estados financeiros devem continuar simples e legiveis:

```text
Elite
Pode errar, investe com confianca e evita all-in desnecessario.

Saudavel
Investe bem e aceita risco moderado quando ha oportunidade esportiva.

Estavel
Escolhe prioridades. Nao consegue melhorar tudo ao mesmo tempo.

Pressionada
Precisa de resultado em breve e pode sacrificar reserva.

Em crise
Aposta agora ou morre devagar. Aceita divida e cortes.

Colapso
Sobreviver > competir. Vende futuro, corta qualidade e depende de resgate.
```

## Operating Costs

Custos minimos por temporada, usados para reserva, risco e capacidade de sobrevivencia:

```text
Mazda Rookie / Toyota Rookie: R$ 120k a R$ 250k
Mazda/Toyota Amador: R$ 250k a R$ 600k
BMW M2 / Production: R$ 600k a R$ 1.6M
GT4: R$ 1.5M a R$ 4M
GT3: R$ 4M a R$ 12M
Endurance: R$ 8M a R$ 25M
```

Esses custos devem cair parcialmente em recessao global, junto com receitas, para evitar colapso sistemico de todas as equipes.

## Safety Reserve

```text
safety_reserve = category_min_operating_cost * risk_policy
```

```text
Elite: 1.50x
Saudavel: 1.20x
Estavel: 0.90x
Pressionada: 0.45x
Crise: 0.10x
Colapso: 0.00x
```

Isso faz equipes seguras preservarem futuro e equipes desesperadas gastarem quase tudo.

## Team Spending AI

A IA da equipe deve decidir gastos a partir de:

```text
financial_state
sporting_context
calendar_profile
promotion_relegation_risk
category_pressure
spending_power
```

### Strategic Profiles

```text
Title Favorite
Prioridade: consistencia.
Gasto: carro balanceado, pit crew forte, pilotos estaveis.
Risco: baixo/medio.

Promotion Hunter
Prioridade: pico de performance.
Gasto: carro e pilotos.
Risco: medio/alto.

Relegation Fighter
Prioridade: sobreviver.
Gasto: foco em pistas onde pode pontuar.
Risco: alto.

Long-Term Builder
Prioridade: estrutura futura.
Gasto: engenharia, facilities e rookies baratos.
Risco: baixo.

Collapsed Survivor
Prioridade: nao morrer.
Gasto: minimo operacional.
Risco: baixo em custo fixo, alto em apostas baratas.
```

## Spending Buckets

Cada equipe distribui o dinheiro da temporada em carteiras de gasto:

```text
car_development
Melhora performance, confiabilidade e perfil do carro.

drivers
Contratacao, renovacao e salarios.

pit_operations
Pit crew quality e capacidade operacional.

engineering
Eficiencia do gasto, evolucao e breakthrough tecnico.

facilities
Estabilidade, teto operacional, confiabilidade e longo prazo.

debt_service
Juros e amortizacao.

reserve
Caixa guardado para evitar colapso.
```

Exemplos de distribuicao:

```text
Equipe rica favorita:
car 30%, drivers 25%, pit 15%, engineering 15%, facilities 10%, reserve 5%

Equipe pobre em risco:
car 45%, drivers 15%, pit 5%, engineering 10%, facilities 0%, reserve 0%

Equipe em construcao:
car 20%, drivers 15%, pit 10%, engineering 25%, facilities 20%, reserve 10%
```

## Conversion Efficiency

Dinheiro nao deve virar performance de forma linear. O retorno depende de eficiencia de gestao:

```text
result = money_spent * management_efficiency * strategy_fit * stability_modifier
```

`management_efficiency` deve considerar:

```text
engineering
facilities
reputacao
morale
financial_state
```

Isso permite historias importantes:

```text
Rica + bem gerida = dominante e consistente.
Rica + mal gerida = forte, mas desperdica dinheiro.
Pobre + inteligente = escolhe batalhas e surpreende.
Pobre + desesperada = all-in, podendo salvar ou quebrar.
Colapsada = carro pior, pit pior, pilotos saem, precisa resgate.
```

## Car Impact

O dinheiro afeta o carro por tres vias:

```text
car_base_investment
Evolucao geral do carro.

car_profile_cost
Custo de construir carro balanceado, semi-focado ou extremo.

reliability_budget
Quanto sobra para evitar carro rapido e fragil.
```

Delta sugerido:

```text
car_delta =
  log(1 + car_spend / category_car_cost_unit)
  * efficiency
  * strategy_modifier
```

Limites por offseason:

```text
car_performance_delta: -2.5 a +2.0
reliability_delta: -10 a +8
engineering_delta: -5 a +4
facilities_delta: -4 a +3
```

## Car Profile Cost

Carro balanceado deve ser mais caro. Perfis extremos sao mais baratos, mas sacrificam versatilidade.

```text
Balanceado: 1.25x
Semi-focado em aceleracao/potencia/dirigibilidade: 1.05x
Extremo em aceleracao/potencia/dirigibilidade: 0.85x
```

Isso incentiva equipes pobres a apostar em nichos do calendario, enquanto equipes fortes tendem a buscar equilibrio.

## Pit Operations

```text
pit_crew_quality
Sobe com dinheiro, facilities, categoria e estabilidade.

pit_strategy_risk
Sobe quando a equipe esta pobre, pressionada ou em risco de rebaixamento.
Desce quando a equipe esta segura, rica ou favorita.
```

Caps por categoria continuam necessarios para impedir uma equipe de Mazda Rookie ter pit crew elite de Endurance.

## Drivers

Dinheiro real deve controlar:

```text
salary_ceiling
Quanto a equipe consegue oferecer.

retention_power
Chance de manter pilotos fortes.

rookie_bargain_behavior
Procura por talento barato.

star_overpay_risk
Risco de equipe desesperada pagar demais por nome forte.
```

Uma equipe quebrada pode perder pilotos bons, mas tambem pode sobreviver achando rookies eficientes.

## Debt and Collapse

Juros por rodada devem variar com estado financeiro:

```text
Elite/saudavel: 0.5% a 1.0%
Estavel: 1.0% a 1.5%
Pressionada: 1.5% a 2.5%
Crise: 2.5% a 4.0%
Colapso: 4.0% a 6.0%
```

Limiares:

```text
debt_ratio > 1.25x receita anual esperada = crise
debt_ratio > 2.00x receita anual esperada = colapso
```

Colapso nao deve ser automaticamente irreversivel. Deve haver mecanismos raros e condicionais:

```text
emergency loan
investor rescue
technical breakthrough
cost cutting
parachute aid after relegation
```

## Round Events

Eventos fortes por rodada devem ser condicionais e relativamente raros.

```text
Base: 3% a 6%
Pressionada: +3%
Crise: +6%
Colapso: +8%
```

Eventos positivos:

```text
sponsor boost
technical breakthrough
investor rescue
cheap rookie discovery
efficient cost cutting
```

Eventos negativos:

```text
sponsor scare
operational failure
debt spiral
facility degradation
overpaid contract pressure
```

Boa engenharia aumenta chance de evento tecnico positivo. Divida alta, reputacao baixa e maus resultados aumentam chance de evento negativo.

## Promotion And Relegation

Promocao:

```text
Aumenta receita futura.
Aumenta custo minimo.
Pode expor equipe pobre a categoria cara demais.
```

Rebaixamento:

```text
Reduz receita.
Reduz custo minimo.
Ativa auxilio de rebaixamento.
```

O auxilio evita morte imediata, mas nao deve transformar rebaixamento em premio.

## Budget Legacy Migration

`budget` deve virar compatibilidade temporaria.

```text
budget = derive_budget_index_from_money(team)
```

Ele pode continuar no banco para evitar migracao perigosa imediata, mas a meta e substituir decisoes baseadas nele por funcoes explicitas:

```text
calculate_spending_power(team)
calculate_salary_ceiling(team)
calculate_car_development_budget(team)
calculate_pit_operations_budget(team)
calculate_driver_budget(team)
calculate_safety_reserve(team)
```

Ordem recomendada:

```text
1. Criar modulo central de planejamento financeiro.
2. Converter decisoes de carro para dinheiro real.
3. Converter pit crew e pit risk.
4. Converter salario e mercado de pilotos.
5. Converter saude financeira.
6. Deixar budget apenas como campo derivado.
7. Remover budget no futuro, se nao houver mais uso.
```

## Testing Scenarios

Casos obrigatorios:

```text
Equipe rica favorita preserva reserva e investe em equilibrio.
Equipe pobre em risco sacrifica reserva e escolhe nicho do calendario.
Equipe com muito caixa mas divida alta nao tem spending power infinito.
Equipe rebaixada recebe auxilio, mas continua limitada.
Equipe promovida ganha receita, mas sofre com custo maior.
Equipe em colapso pode recuperar com evento raro, mas nao sempre.
Equipe pobre bem gerida supera equipe rica mal gerida em eficiencia pontual.
Recessao reduz receitas e custos para evitar colapso geral.
```

## Success Criteria

O sistema sera considerado bem-sucedido quando:

```text
Dinheiro real explicar claramente as decisoes da IA.
Equipes ricas forem fortes sem serem invenciveis.
Equipes pobres conseguirem historias de sobrevivencia e surpresa.
Divida for uma ferramenta perigosa, nao apenas penalidade fixa.
Rebaixamento, promocao e calendario alterarem comportamento financeiro.
O usuario conseguir entender a situacao financeira das equipes sem microgerenciar.
```
