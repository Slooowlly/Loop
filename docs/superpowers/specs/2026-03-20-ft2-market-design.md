# FT-2 Market Design

**Goal:** adicionar um mercado de transferências entre temporadas que renova contratos, preenche vagas, reposiciona pilotos livres e gera propostas pendentes para o jogador sem depender do frontend.

## Contexto

O FT-1 já finaliza standings, aplica evolução, processa aposentadorias, gera rookies e cria a nova temporada. O FT-2 entra no mesmo pipeline de fim de temporada para resolver o estado contratual entre uma temporada e a próxima.

O schema atual já possui as tabelas `market` e `market_proposals`, além de `contracts`, `teams`, `licenses` e `retired`. O modelo atual representa piloto livre como `status = Ativo` com `categoria_atual = None`.

## Abordagem

O mercado será persistido e rodará no backend como parte do `run_end_of_season`. A resolução IA vs IA será feita imediatamente no backend. Apenas propostas para o jogador ficarão pendentes para consumo futuro pelo FT-4.

### Fluxo

1. Expirar contratos que não alcançam a nova temporada.
2. Avaliar pilotos em último ano e processar renovações.
3. Identificar vagas restantes nas equipes.
4. Montar o pool de pilotos disponíveis.
5. Calcular visibilidade de mercado para os disponíveis.
6. Gerar e resolver propostas IA para preencher vagas.
7. Preencher vagas restantes com rookies existentes.
8. Se ainda houver vagas, gerar `emergency rookies`.
9. Gerar propostas pendentes para o jogador.
10. Atualizar hierarquias e verificar o invariante de duas vagas por equipe.

## Componentes

### `market/proposals.rs`

Define `MarketProposal`, `ProposalStatus`, `MarketReport`, `SigningInfo` e `Vacancy`. Também concentra helpers simples de criação/atualização de proposta, evitando espalhar strings de status pelo pipeline.

### `market/evaluation.rs`

Calcula o score de desempenho da temporada e a expectativa baseada no carro da equipe. Esse score será usado na renovação.

### `market/renewal.rs`

Decide se a equipe renova, incluindo salário, duração e papel sugeridos, com ajustes de personalidade.

### `market/visibility.rs`

Calcula a visibilidade de mercado do piloto disponível. O valor influencia quais equipes o consideram.

### `market/team_ai.rs`

Recebe uma `Vacancy` e uma lista de `AvailableDriver` e gera 2-3 propostas plausíveis, respeitando tier de categoria, visibilidade e orçamento relativo.

### `market/driver_ai.rs`

Recebe uma proposta e decide aceitar ou recusar com base em salário, tier, papel, carro, reputação e personalidade.

### `market/pipeline.rs`

Orquestra o mercado completo, persiste contratos e propostas do jogador, atualiza equipes e devolve um `MarketReport`.

## Integração com FT-1

`EndOfSeasonResult` ganhará `market_report`. O `run_end_of_season` chamará o mercado depois das aposentadorias e antes do reset dos stats de temporada, enquanto os dados da temporada recém-finalizada ainda estão disponíveis para avaliação.

## Persistência

- Contratos expirados: `contracts.status = Expirado`.
- Renovações e contratações: novos registros em `contracts`.
- Equipes: `update_team_pilots` e `update_team_hierarchy`.
- Mercado: inserir um registro na tabela `market` para a pré-temporada.
- Propostas do jogador: persistidas em `market_proposals`.
- Propostas recusadas IA vs IA: não persistidas.

## Regras de segurança

- Nenhuma equipe pode terminar o mercado com menos de 2 pilotos.
- Jogador nunca assina automaticamente.
- Sem promoções/rebaixamentos no FT-2.
- Sem mudanças em `models/*` ou migrations.

## Testes

O FT-2 será implementado em TDD, com testes determinísticos usando `StdRng` para:

- avaliação,
- renovação,
- visibilidade,
- IA das equipes,
- IA dos pilotos,
- pipeline completo do mercado,
- integração com o fim de temporada.
