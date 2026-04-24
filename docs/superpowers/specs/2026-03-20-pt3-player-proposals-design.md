# PT-3 Player Proposals Design

## Goal

Permitir que o jogador leia, aceite e recuse propostas persistidas em `market_proposals`, com efeitos completos no mercado de pre-temporada e bloqueio correto de `finalize_preseason`.

## Decisions

- Resolver propostas do jogador nos comandos de carreira, sem alterar a logica central de `market/preseason.rs`.
- Criar `db/queries/market_proposals.rs` para CRUD pequeno e reutilizavel.
- Enriquecer propostas no comando de leitura usando dados atuais de equipe, categoria e companheiro.
- Aceite roda como fluxo sequencial unico: rescindir contrato antigo, remover da equipe antiga, criar novo contrato, encaixar jogador na equipe destino, atualizar hierarquia e expirar propostas restantes.
- Recusa marca a proposta como `Recusada`; se o jogador ficar sem propostas e sem equipe, gerar propostas emergenciais e, como fallback final, force-place em uma equipe com vaga.
- `finalize_preseason` deve falhar se: plano incompleto, propostas pendentes, ou jogador sem equipe.

## Data Flow

1. `get_player_proposals`
   - busca propostas pendentes por `piloto_id`
   - carrega equipe, categoria e companheiro
   - monta `PlayerProposalView`

2. `respond_to_proposal`
   - valida proposta pendente
   - aplica aceite ou recusa
   - gera noticia
   - retorna `ProposalResponse`

3. `finalize_preseason`
   - valida plano
   - valida propostas pendentes
   - valida contrato ativo do jogador
   - remove `preseason_plan.json`
   - gera noticia de abertura da temporada

## Test Strategy

- Queries novas com testes focados.
- Comandos com testes de aceite, recusa, expiracao de outras propostas e bloqueios de finalizacao.
- Cobrir edge case do jogador sem equipe apos recusar tudo.
