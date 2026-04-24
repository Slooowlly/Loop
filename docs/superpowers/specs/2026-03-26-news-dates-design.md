# News Dates Design

## Objective
Integrar o sistema temporal da carreira ao rótulo das notícias para que notícias antigas não apareçam como `Ha pouco` ou `Mais cedo`.

## Approved Output
O campo temporal da notícia passa a usar contexto esportivo + data da carreira no mesmo texto.

Exemplos aprovados:

- `Rodada 3 · 15 mar 2026`
- `Pre-temporada Semana 2 · 08 jan 2026`

## Current Problem
Hoje a aba de notícias usa um relativo curto baseado apenas no contador interno de `timestamp`.

Isso gera rótulos como:

- `Agora`
- `Ha pouco`
- `Mais cedo`
- `Edicao recente`

Esse texto ignora o calendário da carreira, então uma notícia velha dentro da simulação pode parecer nova demais.

## Data Sources
O projeto já possui os dados necessários em camadas separadas:

- `NewsItem.rodada`
- `NewsItem.semana_pretemporada`
- `CalendarEntry.display_date`
- `CalendarEntry.week_of_year`
- ano atual da temporada

## Rendering Rules

### Corridas e notícias ligadas a rodada
Quando a notícia tiver `categoria_id` e `rodada > 0`, o backend deve resolver a `display_date` daquela rodada no calendário da categoria e montar:

- `Rodada X · DD mmm AAAA`

### Notícias de pré-temporada
Quando a notícia tiver `semana_pretemporada`, o backend deve montar:

- `Pre-temporada Semana X · DD mmm AAAA`

A data da pré-temporada deve ser derivada a partir da primeira corrida da categoria:

- encontrar a data da rodada 1 no calendário da categoria
- descobrir o maior número de `semana_pretemporada` existente naquela temporada
- voltar as semanas necessárias até posicionar a semana da notícia

Isso mantém a cronologia coerente mesmo sem um campo de data explícita na notícia.

### Fallback
Se a notícia não tiver rodada nem semana de pré-temporada resolvível, o backend deve cair para um rótulo estável de temporada:

- `Temporada X · AAAA`

## Architecture
A montagem continua acontecendo no backend em `news_tab.rs`, dentro da transformação de `NewsItem` para `NewsTabStory`.

O frontend não precisa criar lógica nova de data. Ele continua exibindo `time_label`, mas agora esse texto já vem alinhado com o calendário da carreira.

## Validation
- notícias de rodada mostram `Rodada X · data`
- notícias de pré-temporada mostram `Pre-temporada Semana X · data`
- notícias sem data editorial clara usam fallback estável de temporada
- nenhum item da aba de notícias usa mais `Agora`, `Ha pouco`, `Mais cedo` ou `Edicao recente`
