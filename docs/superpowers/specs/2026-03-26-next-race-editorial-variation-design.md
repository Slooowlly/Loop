# Next Race Editorial Variation Design

## Goal

Refinar a editorial da `NextRaceTab` para que a previa varie pelo contexto esportivo real da etapa, combinando pressao de campeonato e temperatura do fim de semana sem depender apenas de trocas cosmeticas de frase.

## Current Context

A aba ja recebe contexto suficiente para esse refinamento:

- `next_race_briefing.track_history`
- `next_race_briefing.primary_rival`
- `next_race_briefing.weekend_stories`
- classificacao e forma recente do grid
- progresso do campeonato e interesse do evento

Hoje a copy principal ainda mistura tons diferentes e decide boa parte da narrativa por regras lineares dentro de `src/pages/tabs/NextRaceTab.jsx`.

## Editorial Direction

O tom dominante passa a ser `jornalismo esportivo`, com:

- manchetes puxando levemente para `transmissao de TV`
- chamadas praticas puxando levemente para `box`

O objetivo e soar competitivo e contextual, sem teatralizar demais nem parecer texto gerado por template aleatorio.

## Variation Model

Toda a previa passa a nascer da combinacao de dois eixos.

### Championship Axis

Estados editoriais:

- `leader`
- `chase`
- `pressure`
- `survival`
- `outsider`

Sinais usados:

- posicao no campeonato
- gap para o lider
- gap para tras
- rodadas restantes
- forma recente

### Weekend Axis

Estados editoriais:

- `history_positive`
- `history_negative`
- `weather_unstable`
- `rival_spotlight`
- `weekend_hot`
- `weekend_neutral`

Sinais usados:

- historico do jogador na pista
- rival principal
- clima
- numero e peso das noticias do fim de semana

## Content Mapping

Cada bloco textual usa os dois eixos, mas com pesos diferentes:

- `headline`
  mistura campeonato + etapa, com leitura curta e forte
- `paragraphs[0]`
  abre pela pressao competitiva do campeonato
- `paragraphs[1]`
  conecta rival, forma e margem de erro
- `rivalSummary`
  enfatiza o duelo esportivo direto
- `rivalSupport`
  explica o que muda na tabela se esse duelo virar
- `scenario`
  descreve a consequencia esportiva da rodada
- `actionHint`
  traduz a leitura editorial em objetivo pratico
- `historyMeta`
  deve usar historico real da pista com linguagem menos burocratica
- `weekendStoriesEmpty`
  fallback neutro e limpo

## Structural Change

Para evitar que `NextRaceTab.jsx` concentre classificacao, contexto e copy ao mesmo tempo, a logica editorial deve ser separada em uma unidade dedicada.

### Proposed Responsibility Split

- `src/pages/tabs/NextRaceTab.jsx`
  montagem visual e ligacao com a store
- `src/pages/tabs/nextRaceBriefing.js`
  permanece responsavel pelas variacoes dos favoritos
- `src/pages/tabs/nextRaceEditorial.js`
  novo modulo para:
  - classificar estado de campeonato
  - classificar estado da etapa
  - escolher intencoes editoriais
  - resolver frases por bloco

## Fallback Rules

- Se faltar `track_history`, a copy cai para historico generico sem fingir dado especifico.
- Se faltar `primary_rival`, o texto volta para rival direto derivado da tabela.
- Se nao houver `weekend_stories`, o bloco do radar continua existindo com leitura neutra.
- Se sinais conflitarem, `championship axis` define a pressao base e `weekend axis` atua como modulador.

## Non-Goals

- Nao alterar o payload do backend.
- Nao reescrever a lista de favoritos.
- Nao introduzir persistencia nova para textos principais neste ciclo.
- Nao transformar a aba em sistema procedural aberto com dezenas de combinacoes ocultas.

## Testing Strategy

Cobrir principalmente comportamento editorial, nao implementacao interna:

- muda o `headline` quando muda o estado de campeonato
- muda o `scenario` quando muda o estado de etapa
- usa historico positivo e negativo como gatilho real de copy
- usa noticias do fim de semana para aquecer o bloco de paddock/radar
- mantem fallback neutro quando faltam sinais

## Acceptance Criteria

- A previa deixa de parecer uma unica voz fixa.
- A variacao nasce de contexto esportivo, nao de sinonimos.
- Dois cenarios com tabela parecida, mas fim de semana diferente, produzem copys diferentes.
- Dois cenarios com a mesma pista, mas pressao de campeonato diferente, produzem copys diferentes.
- A interface continua enxuta e coerente com o layout atual.
