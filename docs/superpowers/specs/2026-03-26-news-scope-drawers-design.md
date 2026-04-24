# News Scope Families Design

## Objective
Reduzir o espaco ocupado pelo seletor da aba de noticias sem perder a leitura de progressao entre campeonatos.

## Approved Interaction
O seletor abaixo de "Panorama do Campeonato" passa a funcionar com duas camadas:

1. uma linha compacta de familias clicaveis
2. uma unica trilha expandida logo abaixo

Familias aprovadas:

- `Mazda`
- `Toyota`
- `BMW`
- `GT4`
- `GT3`
- `LMP2`
- `Mais famosos` separado como escopo especial

## Behavior
Clicar em uma familia faz duas coisas ao mesmo tempo:

- expande a linha daquela familia
- aplica automaticamente o filtro base da familia

Filtros base:

- `Mazda` -> `Mazda Rookie`
- `Toyota` -> `Toyota Rookie`
- `BMW` -> `BMW M2`
- `GT4` -> `GT4 Series`
- `GT3` -> `GT3 Championship`
- `LMP2` -> `Endurance Championship`

Depois da expansao, o usuario pode clicar nos campeonatos internos para refinar ainda mais o recorte.

## Shared Class Filtering
Quando o usuario escolhe um campeonato compartilhado dentro de uma familia, o recorte precisa carregar a classe daquela familia em toda a aba, nao apenas nos chips de pilotos ou equipes.

Exemplos aprovados:

- `Mazda` -> `Production` => `scope_id=production_challenger` com `scope_class=mazda`
- `Toyota` -> `Production` => `scope_id=production_challenger` com `scope_class=toyota`
- `BMW` -> `Production` => `scope_id=production_challenger` com `scope_class=bmw`
- `GT4` -> `Endurance` => `scope_id=endurance` com `scope_class=gt4`
- `GT3` -> `Endurance` => `scope_id=endurance` com `scope_class=gt3`
- `LMP2` -> `Endurance` => `scope_id=endurance` com `scope_class=lmp2`

Esse subescopo precisa afetar:

- briefing principal
- lista de historias
- filtros contextuais
- label do escopo ativo

## Family Layout

### Mazda
- `Mazda Rookie` -> `Mazda Championship` -> `Production`

### Toyota
- `Toyota Rookie` -> `Toyota Cup` -> `Production`

### BMW
- `BMW M2` -> `Production`

### GT4
- `GT4 Series` -> `Endurance`

### GT3
- `GT3 Championship` -> `Endurance`

### LMP2
- `LMP2 Class` -> `Endurance`

## Visual Direction
O componente deve ser compacto e direto:

- botoes curtos na linha superior
- apenas uma familia expandida por vez
- trilha horizontal simples, sem um painel grande dominante
- destaque no item atualmente filtrado

## Data Strategy
A hierarquia continua derivada no frontend a partir dos `scopes` do bootstrap e de uma configuracao local.

Para campeonatos compartilhados, a snapshot passa a receber um campo adicional:

- `scope_class` opcional para distinguir a classe ativa dentro de `production_challenger` e `endurance`

## Validation
- clicar em uma familia dispara o `scope_id` base esperado
- clicar em um campeonato da trilha troca o `scope_id` para o recorte especifico
- clicar em `Production` ou `Endurance` dentro de uma familia envia tambem o `scope_class` correto
- o backend exclui noticias, equipes e pilotos de outras classes no recorte compartilhado
- `Mais famosos` continua funcionando como escopo separado
