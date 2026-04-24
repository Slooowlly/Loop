# PT-1 Weekly Market Design

## Goal

Refatorar o mercado de transferências para funcionar como uma pré-temporada semanal e persistente, em vez de resolver todas as movimentações em uma única chamada.

## Approach

Criar um novo orquestrador em `src-tauri/src/market/preseason.rs` responsável por duas etapas distintas:

1. `initialize_preseason()` planeja todos os eventos da pré-temporada para a temporada recém-criada, sem executar efeitos no banco.
2. `advance_week()` executa apenas os eventos planejados para a semana atual e devolve um `WeekResult` serializável para o frontend futuro.

O `market/pipeline.rs` existente permanece intacto como referência e fallback de lógica, mas deixa de ser chamado pelo pipeline de fim de temporada.

## Data Model

O estado novo é composto por:

- `PreSeasonState`: semana atual, total de semanas, fase e status de conclusão.
- `PreSeasonPlan`: estado + eventos planejados + semanas já executadas.
- `PlannedEvent` e `PendingAction`: representação serializável do que será executado em cada semana.
- `WeekResult` e `MarketEvent`: feed semanal de acontecimentos.

O plano será persistido como `preseason_plan.json` dentro da pasta do save. Isso evita migration nova e permite retomar a pré-temporada após fechar o app.

## Execution Order

`run_end_of_season()` passa a executar:

1. standings
2. licenças
3. evolução
4. aposentadorias
5. promoção/rebaixamento
6. criação da nova temporada
7. reset de stats
8. geração do calendário
9. inicialização da pré-temporada semanal

O mercado deixa de acontecer dentro de `run_end_of_season()`.

## Weekly Phases

As fases planejadas são:

- `ContractExpiry`
- `Transfers`
- `PlayerProposals`
- `RookiePlacement`
- `Finalization`
- `Complete`

As propostas do jogador são geradas e persistidas no PT-1, mas não bloqueiam o avanço das semanas. O bloqueio para iniciar a temporada seguinte fica para o PT-3.

## Commands

O backend expõe:

- `advance_season`: cria a nova temporada e inicializa a pré-temporada.
- `advance_market_week`: executa a semana atual do plano.
- `get_preseason_state`: retorna o estado atual da pré-temporada.
- `finalize_preseason`: encerra a pré-temporada apenas se o plano estiver completo.

## Testing

Cobrir:

- criação do plano
- transição de fases
- execução de semanas
- persistência JSON
- efeitos reais no banco para expiração, renovação, transferência e rookies
- invariantes de equipes preenchidas ao final
