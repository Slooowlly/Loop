# Desativação Temporária da Tela de Campeão

## Decisão

A tela de campeão não deve aparecer nesta versão enquanto seu conteúdo for apenas um template frontend com dados DEMO. Esta decisão substitui o fluxo descrito em `2026-07-21-season-champion-post-race-flow-design.md` até existir um pipeline real de backend.

## Alternativas consideradas

1. **Desativação completa — escolhida.** Remove todos os pontos de entrada e garante que o template não seja exposto ao jogador.
2. **Desativar apenas o gatilho automático.** Manteria o botão destrutivo nas Configurações e ainda permitiria abrir dados falsos; rejeitada.
3. **Feature flag.** Permitiria reativação rápida, mas adicionaria configuração sem uso imediato; rejeitada por YAGNI.

## Comportamento desta versão

- Concluir o debriefing da última corrida segue novamente a política pós-corrida existente e abre Notícias, sem overlay intermediário.
- `App.jsx` não monta `SeasonChampionOverlay` como host global.
- Configurações não mostra o botão “Forçar fim de temporada”.
- O store não expõe `debugForceSeasonEndChampion` nem estado de navegação pós-overlay.
- `SeasonChampionOverlay.jsx` e seu CSS permanecem no repositório, sem ponto de entrada em produção, para futura conexão com dados reais.
- `championOverlay` permanece no estado inicial como `null`. As ações dormentes ficam reduzidas ao contrato básico: `showChampionOverlay(data)` apenas define o payload (ou `{ demo: true }`) e `hideChampionOverlay()` apenas volta o campo para `null`.
- `afterCloseTab`, `pendingDashboardTab` e `consumePendingDashboardTab` são removidos por não terem consumidor nesta versão.

## Dados e persistência

Nenhuma ação desta versão simula corridas ou avança a temporada para abrir a tela. A remoção do botão destrutivo elimina o risco de alterar o save apenas para visualizar o template.

## Testes

- Teste estrutural confirma separadamente que `App.jsx` não importa/monta o host, `Settings.jsx` não renderiza nem chama o debug destrutivo e `Dashboard.jsx` não chama `showChampionOverlay` nem consome destino pós-overlay.
- Teste do Dashboard confirma que um resultado final fresco volta a navegar diretamente para Notícias.
- Testes de publicação/consumo de `pendingDashboardTab`, abertura automática e `debugForceSeasonEndChampion` são removidos ou substituídos pelo comportamento desativado.
- Os testes isolados de renderização e fechamento do componente visual podem permanecer, ajustados para o contrato básico do store, porque não criam um ponto de entrada em produção.
- Testes focados do store e Dashboard permanecem verdes após remover o estado transitório sem consumidores.
- Build confirma que os arquivos dormentes não quebram a compilação.
