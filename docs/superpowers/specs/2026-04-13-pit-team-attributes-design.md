# Pit Team Attributes Design

**Goal**

Adicionar dois atributos persistidos de equipe, ambos na escala `0-100`, para preparar a futura integracao com o iRacing:

- `pit_strategy_risk`
- `pit_crew_quality`

Os valores passam a fazer parte do mundo da carreira, evoluindo ao longo das temporadas e ficando disponiveis para UI/payload mesmo antes da exportacao real.

**Context**

- Hoje o jogo ja persiste `budget`, `engineering` e `facilities` na equipe em [src-tauri/src/models/team.rs](</C:/Users/rodri/.config/superpowers/worktrees/iracerapp-v1/seasonal-car-build-strategy/src-tauri/src/models/team.rs:92>), mas esses atributos ainda estao pouco conectados ao lado operacional do time.
- `confiabilidade` ja afeta incidentes mecanicos na simulacao, mas ainda nao existe sistema real de pit strategy nem pit crew dentro da corrida local.
- A offseason ja recalcula atributos estrategicos da equipe na preseason, incluindo `car_build_profile`, em [src-tauri/src/market/preseason.rs](</C:/Users/rodri/.config/superpowers/worktrees/iracerapp-v1/seasonal-car-build-strategy/src-tauri/src/market/preseason.rs:216>).

**Core Decisions**

- `pit_strategy_risk` mede apetite por risco, nao qualidade tecnica.
- `pit_crew_quality` mede capacidade operacional do box.
- Os dois atributos sao independentes:
  - equipe forte/rica tende a `risk` menor e `quality` maior
  - equipe fraca/pobre tende a `risk` maior e `quality` menor
- `pit_crew_quality` precisa melhorar ou piorar ao longo das temporadas com base em resultados e estrutura.
- `pit_crew_quality` tem cap por categoria para impedir equipes de base de atingirem nivel operacional de elite global.
- `pit_strategy_risk` recebe apenas um viés comportamental por categoria, sem cap duro.

**Pit Crew Quality**

`pit_crew_quality` nasce da estrutura da equipe e recebe um ajuste sazonal de momento:

```text
base_quality =
  budget * 0.45 +
  engineering * 0.35 +
  facilities * 0.20
```

Depois disso entra um `seasonal_momentum`, baseado no desempenho da temporada anterior:

- resultado final no campeonato
- desempenho acima/abaixo da meta da equipe
- promocao ou rebaixamento

Para evitar saltos bruscos, a qualidade final e suavizada entre temporadas:

```text
new_quality =
  old_quality * 0.65 +
  target_quality * 0.35
```

Por fim, aplica-se o cap por categoria:

- `mazda_rookie`: `55`
- `toyota_rookie`: `55`
- `mazda_amador`: `64`
- `toyota_amador`: `64`
- `bmw_m2`: `72`
- `production_challenger`: `76`
- `gt4`: `84`
- `gt3`: `93`
- `endurance`: `97`

**Pit Strategy Risk**

`pit_strategy_risk` mede o quanto a equipe aceita apostar para maximizar teto em vez de consistencia.

Fatores principais:

- risco de rebaixamento
- chance/pressao de promocao
- forca relativa do carro na categoria
- pressao de titulo
- budget como amortecedor leve
- identidade leve e deterministica da equipe para evitar grids homogeneos

Leitura da escala:

- `0-20`: ultra conservadora
- `21-40`: conservadora
- `41-55`: equilibrada
- `56-75`: agressiva
- `76-100`: desesperada/oportunista

O valor e recalculado na offseason e responde mais rapido do que `pit_crew_quality`:

```text
new_risk =
  old_risk * 0.40 +
  target_risk * 0.60
```

Vies por categoria:

- rookies/cups puxam um pouco para baixo
- `gt4` / `gt3` aceitam um pouco mais de agressividade
- `endurance` puxa levemente de volta para consistencia

**Persistence**

`Team` passa a persistir:

- `pit_strategy_risk: f64`
- `pit_crew_quality: f64`

Migracoes legadas devem preencher valores seguros calculados a partir do estado atual da equipe ou, quando isso nao for possivel no SQL puro, usar defaults medianos e deixar a preseason recalcular.

**Seasonal Recalculation**

O recalculo entra no mesmo fluxo da preseason que hoje escolhe o `car_build_profile`.

Em cada offseason, a equipe:

1. escolhe `car_build_profile`
2. recalcula `pit_strategy_risk`
3. recalcula `pit_crew_quality`

O input do recálculo combina:

- categoria da nova temporada
- calendario/contexto competitivo da nova temporada
- estrutura atual da equipe
- resultado da temporada anterior

**UI And Payload**

`TeamSummary` passa a expor os dois campos.

A `MyTeamTab` mostra:

- `pit_strategy_risk`
- `pit_crew_quality`

como telemetria de equipe pronta para futura exportacao ao iRacing.

**Testing**

Cobertura minima:

- persistencia e migration dos novos campos
- caps por categoria em `pit_crew_quality`
- comportamento esperado:
  - equipe rica -> `pit_crew_quality` maior
  - equipe ameaçada/pobre -> `pit_strategy_risk` maior
- preseason recalculando ambos junto do `car_build_profile`
- `TeamSummary` serializando os novos campos

