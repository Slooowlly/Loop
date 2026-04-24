# OC-1 Simulacao Simultanea de Todas as Categorias

## Objetivo

Quando o jogador simula uma corrida na sua categoria, o jogo deve avancar proporcionalmente as outras categorias para que o calendario global nunca fique para tras a ponto de bloquear `advance_season`.

## Decisao Principal

- `season.rodada_atual` continua representando apenas a rodada do jogador.
- O progresso das outras categorias e determinado exclusivamente pelo numero de corridas concluidas no calendario de cada categoria.

## Abordagem

- Extrair um pipeline compartilhado de simulacao de categoria.
- Reutilizar `run_full_race` e a logica atual de persistencia de stats/calendario/historico/noticias.
- Criar um orquestrador que calcula quantas corridas cada categoria deveria ter concluido com base na proporcao do progresso do jogador.
- Na ultima corrida do jogador, forcar a simulacao de todas as pendencias restantes das outras categorias.

## Backend

- Criar `simulation/batch.rs` com:
  - `races_should_be_completed`
  - `simulate_category_race`
  - `simulate_other_categories`
  - structs de retorno resumido para outras categorias
- Adicionar query para buscar corridas pendentes por categoria.
- Atualizar `simulate_race_weekend` para retornar `RaceWeekendResult { player_race, other_categories }`.
- Persistir `race_results.json` para todas as corridas simuladas em background.
- Gerar noticias apenas para highlights das categorias automaticas.

## Frontend

- O store passa a separar `player_race` de `other_categories`.
- `RaceResultView` ganha uma secao expansivel com o resumo das outras categorias.

## Risco Controlado

O ponto mais sensivel e extrair a logica atual de `commands/race.rs` sem criar divergencia entre corrida do jogador e corrida automatica. Por isso a implementacao deve manter um caminho comum de simulacao/persistencia e cobrir esse comportamento com testes de contagem, calendario e historico.
