# PT-4 Frontend da Pre-Temporada Interativa

## Objetivo

Conectar o backend de fim de temporada e pre-temporada a uma experiencia fluida no dashboard, sem criar novas rotas. O fluxo deve sair da `NextRaceTab`, passar pelo resumo de fim de temporada, entrar na pre-temporada semanal e voltar ao dashboard normal quando a nova temporada for iniciada.

## Direcao

- A store Zustand concentra o estado transitório do fluxo sazonal.
- `Dashboard.jsx` decide entre tres modos de tela: resultado de corrida, resumo de fim de temporada e pre-temporada.
- `EndOfSeasonView.jsx` mostra um resumo legivel e escaneavel com secoes expansíveis.
- `PreSeasonView.jsx` mostra progresso, feed semanal acumulado, propostas do jogador e CTA final.
- `NextRaceTab.jsx` deixa de ficar travada quando `nextRace` e `null` e passa a entrar no fluxo sazonal.

## Regras de UX

- O jogador nao perde o contexto: o feed semanal acumula no frontend conforme as semanas avancam.
- As propostas do jogador aparecem destacadas e ficam sempre acessiveis ate serem resolvidas.
- O botao de iniciar temporada so aparece quando a pre-temporada terminou e nao restam propostas pendentes.
- Loading, erros e mensagens de resposta a propostas ficam na camada de UI, sem alterar o backend.

## Compatibilidade

- Nao alterar backend.
- Reaproveitar `GlassCard`, `GlassButton`, `LoadingOverlay` e tokens existentes.
- Seguir payloads em `snake_case` vindos do Tauri.
