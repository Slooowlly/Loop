# PT-2 News System Design

## Goal

Adicionar um sistema de noticias persistidas para eventos de mercado, fim de temporada e corridas, sem alterar o schema existente do banco.

## Key Decisions

- O modelo rico de noticia existe apenas no Rust em `src-tauri/src/news/`.
- A tabela `news` continua enxuta e recebe uma persistencia hibrida:
  - `tipo`, `titulo`, `temporada_id`, `rodada`, `criado_em`, `lida` usam colunas nativas.
  - `texto` guarda um envelope JSON contendo o corpo da noticia e os metadados extras.
  - `chave_dedup` guarda uma chave unica e estavel, sem JSON.
- Convencao para `rodada`:
  - `> 0`: rodada de corrida
  - `< 0`: semana de pre-temporada
  - `0`: fim de temporada e eventos gerais
- `NewsType` e `NewsImportance` novos ficam no modulo `news`, sem alterar `models/*`.

## Architecture

- `news/mod.rs`
  - Define `NewsItem`, `NewsType`, `NewsImportance`.
  - Exporta `generator`.
- `news/generator.rs`
  - Converte `MarketEvent`, `EndOfSeasonResult` e `RaceResult` em `NewsItem`.
  - Gera dedup keys estaveis e timestamps monotonicamente crescentes.
- `db/queries/news.rs`
  - Faz CRUD, batch insert, filtros e trim FIFO.
  - Faz parse do envelope JSON em `texto` ao reconstruir `NewsItem`.
- Integracoes
  - `advance_season`: gera noticias de fim de temporada.
  - `advance_market_week`: gera noticias da semana da pre-temporada.
  - `simulate_race_weekend`: gera noticias da corrida.
  - `get_news`: consulta e filtra noticias para o frontend futuro.

## Data Mapping

- `id` -> `NewsItem.id`
- `tipo` -> `NewsType` serializado como string
- `titulo` -> `NewsItem.titulo`
- `texto` -> envelope JSON com:
  - `texto`
  - `icone`
  - `semana_pretemporada`
  - `categoria_id`
  - `categoria_nome`
  - `importancia`
  - `timestamp`
  - `driver_id`
  - `team_id`
- `chave_dedup` -> chave unica da noticia
- `temporada_id` -> id da season
- `rodada` -> rodada positiva, negativa ou zero
- `criado_em` -> timestamp textual
- `lida` -> sempre `false` na criacao

## Query Strategy

- Filtro por temporada: resolver `season.numero -> season.id` antes da query.
- Filtro por tipo: comparar `tipo`.
- Filtro por semana de pre-temporada: `rodada = -semana`.
- Ordenacao principal: `criado_em DESC`, fallback por `id DESC`.
- Trim FIFO: apagar os registros mais antigos acima de 400 itens.

## Testing Strategy

- Validar mapeamento `MarketEvent -> NewsItem`.
- Validar agrupamentos de licencas e destaques de evolucao.
- Validar batch insert/load/filters/trim.
- Validar integracao:
  - `advance_season` persiste noticias de fim de temporada.
  - `advance_market_week` persiste noticias da semana.
  - `simulate_race_weekend` persiste noticias da corrida.
