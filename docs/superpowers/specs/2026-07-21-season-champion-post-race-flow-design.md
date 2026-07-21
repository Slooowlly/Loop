# Fluxo Pós-Corrida da Tela de Campeão

## Objetivo

Exibir a tela global de campeão imediatamente após o jogador concluir o debriefing da última corrida do calendário. A Home deve permanecer visível por trás do overlay e, quando o overlay for fechado, a navegação deve seguir para Notícias.

## Sequência aprovada

1. A última corrida termina e abre o debriefing normalmente.
2. O jogador seleciona “Continuar” no debriefing.
3. O Dashboard fecha o resultado, seleciona a aba Home (`standings`) e abre `SeasonChampionOverlay`.
4. O overlay usa os dados DEMO existentes enquanto o pipeline real não estiver disponível.
5. Ao fechar o overlay por “Continuar”, Esc, botão Fechar ou backdrop, o Dashboard seleciona a aba Notícias (`news`).

## Detecção do gatilho

O fluxo usa os sinais existentes `resultIsFresh` e `lastRaceWasFinale`. Assim, só uma corrida recém-finalizada cujo `thematic_slot` represente o final do calendário dispara a celebração. Reabrir o resultado salvo de uma corrida antiga não abre o overlay.

Corridas comuns mantêm a política atual de destino pós-corrida em `resolvePostRaceLanding`. O final de campeonato deixa de navegar diretamente para Notícias: primeiro passa por Home e pelo overlay.

## Arquitetura

### Dashboard

`Dashboard.jsx` permanece responsável pela aba local ativa. No fechamento de um debriefing final, ele cancela qualquer avaliação de leitura de notícias, seleciona Home, solicita a abertura do overlay com destino pós-fechamento `news` e fecha o resultado.

O Dashboard também observa `pendingDashboardTab`, uma solicitação única de troca de aba exposta pelo store. Ao recebê-la, atualiza `activeTab` e chama `consumePendingDashboardTab()`. Essa ação lê o destino atual, limpa o campo sincronamente e o devolve, impedindo repetição em renderizações posteriores.

### Store

`useCareerStore.js` continua sendo a fonte do estado global do overlay. `showChampionOverlay(data)` aceita no próprio objeto um `afterCloseTab` opcional. Ao abrir uma nova instância, limpa qualquer solicitação obsoleta. `hideChampionOverlay()` fecha o overlay e copia `championOverlay.afterCloseTab` para `pendingDashboardTab`, ou mantém esse campo nulo quando não há destino. `consumePendingDashboardTab()` devolve o destino e limpa o campo sincronamente.

O contrato é transitório e restrito à UI:

- `championOverlay: null | { demo?: boolean, afterCloseTab?: string, ...dadosReais }`;
- `pendingDashboardTab: null | string`;
- `showChampionOverlay(data?)` abre uma única instância e limpa pedidos anteriores;
- `hideChampionOverlay()` publica no máximo um destino referente à instância fechada;
- `consumePendingDashboardTab()` lê e limpa o pedido;
- `clearCareer()` restaura ambos para `null` por meio de `initialState`.

O atalho de debug não fornece destino. Portanto, fechar uma prévia aberta pelas Configurações não muda a aba atual.

### Overlay global

`SeasonChampionOverlay.jsx` mantém um único caminho de fechamento para todos os controles. Esse caminho chama `hideChampionOverlay`; a regra de destino fica no store, sem callbacks React armazenados em estado e sem eventos globais do navegador.

No Esc, o overlay registra o listener em fase de captura e chama `preventDefault()` e `stopImmediatePropagation()` antes de fechar. Isso dá precedência à camada de campeão e impede que o mesmo Esc abra o `PauseMenu` ou feche outra camada por baixo. Botão “Continuar”, botão “Fechar” e backdrop chamam o mesmo fechamento, sem comportamento alternativo.

## Casos de erro e limites

- Se o resultado não for fresco, o comportamento de reabertura existente permanece inalterado.
- Se não for a última corrida, a política adaptativa Home/Notícias existente permanece inalterada.
- Se o overlay não tiver destino pós-fechamento, fechá-lo apenas remove a camada visual.
- A navegação para Notícias não inicia a avaliação de leitura usada nas corridas comuns, preservando a regra atual do final de campeonato.
- O avanço irreversível da temporada não faz parte desta mudança; o gatilho ocorre depois da corrida e antes do avanço manual para a pré-temporada.
- Uma nova abertura substitui qualquer destino antigo; uma troca ou limpeza de carreira remove overlay e solicitação pendente.

## Testes

- Teste do Dashboard: debriefing fresco da última corrida abre o overlay sobre Home, em vez de navegar diretamente para Notícias.
- Teste do Dashboard: solicitação pós-overlay `news` troca a aba e é consumida.
- Teste do store: fechar overlay com destino publica uma única solicitação de navegação.
- Teste do store: fechar overlay de debug sem destino não solicita navegação.
- Teste do overlay: Continuar, Fechar, backdrop e Esc usam o mesmo fechamento; Esc bloqueia a propagação antes de fechar.
- Teste de integração com o menu de pausa: Esc sobre o overlay não abre o `PauseMenu` e solicita Notícias uma única vez.
- Suítes focadas de `Dashboard` e `useCareerStore`, seguidas pela suíte de UI completa e build.
