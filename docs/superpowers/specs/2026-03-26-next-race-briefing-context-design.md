# Next Race Briefing Context Design

**Goal:** Enriquecer a previa da proxima corrida com tres blocos narrativos faltantes: historico enxuto do jogador na pista, rival principal pronto para consumo e noticias automaticamente ligadas a etapa.

**Why:** A tela ja consegue montar uma boa leitura de campeonato e condicoes, mas ainda carece de memoria especifica do circuito e de ganchos narrativos de paddock. Esses tres blocos melhoram a sensacao de continuidade sem exigir que o frontend reconstrua regra de negocio.

## Scope

- Adicionar um bloco novo ao payload de `load_career`, ao lado de `next_race`.
- Centralizar no backend a selecao e o resumo dos dados da previa.
- Atualizar a `NextRaceTab` para consumir os novos campos com fallbacks seguros.
- Cobrir o contrato novo com testes Rust e os estados principais com testes React.

## Data Model

Adicionar `next_race_briefing` em `CareerData`.

### `next_race_briefing.track_history`

- `has_data: bool`
- `starts: i32`
- `best_finish: Option<i32>`
- `last_finish: Option<i32>`
- `dnfs: i32`
- `last_visit_season: Option<i32>`
- `last_visit_round: Option<i32>`

Resumo enxuto. O backend deve calcular esse bloco a partir de resultados persistidos, usando `race_results` e `calendar.track_name` como fonte principal. `track_dnf_history` pode complementar a narrativa, mas nao deve ser a fonte unica.

### `next_race_briefing.primary_rival`

- `driver_id: String`
- `driver_name: String`
- `championship_position: i32`
- `gap_points: i32`
- `is_ahead: bool`
- `rivalry_label: Option<String>`

Base principal: rival direto no campeonato. Quando houver rivalidade registrada para o jogador, o backend pode preencher `rivalry_label` com uma leitura curta do nivel atual, sem substituir o rival esportivo se a melhor leitura continuar sendo a tabela.

### `next_race_briefing.weekend_stories`

Lista de ate 3 itens:

- `id: String`
- `icon: String`
- `title: String`
- `summary: String`
- `importance: String`

Selecao feita no backend com noticias da temporada atual, filtradas pela categoria da proxima corrida e pela `rodada` da proxima etapa. Prioridade editorial sugerida:

1. `Rivalidade`
2. `Hierarquia`
3. `Corrida`
4. `Incidente`
5. `FramingSazonal`

## Backend Design

### Contract

Expandir `CareerData` em `src-tauri/src/commands/career_types.rs` com structs dedicadas a previa:

- `NextRaceBriefingSummary`
- `TrackHistorySummary`
- `PrimaryRivalSummary`
- `BriefingStorySummary`

### Assembly

Em `src-tauri/src/commands/career.rs`, no fluxo de `load_career`:

- Se nao houver `next_race`, retornar `next_race_briefing: None`.
- Se houver `next_race`, montar o bloco novo a partir de helper functions pequenas e focadas.

Helpers sugeridos:

- `build_next_race_briefing_summary(...)`
- `build_track_history_summary(...)`
- `build_primary_rival_summary(...)`
- `build_weekend_story_summaries(...)`

### Track history source

Calcular por consultas a `race_results` + `calendar`:

- total de largadas do jogador naquela pista
- melhor chegada valida
- ultimo resultado naquela pista
- total de DNFs naquela pista
- ultima visita por temporada/rodada

Comparacao por `track_name`, seguindo o padrao ja usado em outras partes narrativas do projeto.

### Rival source

Regra principal:

- se o jogador lidera o campeonato, rival = P2
- caso contrario, rival = piloto imediatamente a frente

Enriquecimento opcional:

- buscar rivalidades do jogador
- se a rivalidade envolver o rival esportivo, preencher `rivalry_label`
- se nao envolver, manter o rival esportivo e deixar `rivalry_label` vazio

Isso preserva clareza competitiva e evita trocar o rival da tabela por um antagonista menos relevante para a etapa.

### Story source

Reaproveitar consulta de noticias existente e filtrar:

- mesma temporada
- mesma categoria da prova
- mesma `rodada` da proxima etapa

Ordenacao:

- maior importancia primeiro
- depois prioridade editorial por tipo
- depois timestamp mais recente

Resumo:

- usar `texto` truncado ou primeira frase curta para `summary`

## Frontend Design

`src/stores/useCareerStore.js`

- armazenar `nextRaceBriefing` vindo de `load_career`
- limpar esse campo quando necessario nos fluxos que ja limpam `nextRace`

`src/pages/tabs/NextRaceTab.jsx`

- trocar o card atual de "Historico" para usar o resumo de pista, com fallback para o historico generico ja existente se o backend ainda nao tiver dados
- usar `primary_rival` para copy contextual quando existir
- renderizar um bloco curto de noticias da etapa, sem transformar a tela em uma segunda NewsTab

Comportamento esperado:

- sem historico na pista: texto neutro, sem destaque vazio
- sem rival pronto: continua a heuristica atual
- sem noticias da etapa: bloco some ou mostra frase breve de ausencia de manchetes

## Error Handling

- O bloco novo deve ser opcional no payload.
- Falhas ao montar noticias ou rivalidade nao devem impedir `load_career`.
- Se uma subconsulta falhar, a previa continua carregando com campos vazios ou `None`.

## Testing

### Rust

- teste de contrato com `CareerData`
- teste de historico por pista com multiplas visitas
- teste de rival principal para lider e para P2/P3
- teste de noticias da etapa filtrando por categoria e rodada

### Frontend

- teste exibindo historico de pista vindo do backend
- teste exibindo rival principal vindo do payload
- teste exibindo noticias da etapa
- teste de fallback quando `next_race_briefing` vier ausente

## Non-Goals

- nao transformar a previa em uma timeline completa de noticias
- nao expor ainda o perfil completo de rivalidade
- nao reestruturar a NewsTab
- nao criar copy pesada no frontend
