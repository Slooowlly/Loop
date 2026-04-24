# Window Controls Widget Panel Design

**Contexto:** O componente `src/components/layout/WindowControlsDrawer.jsx` já possui uma bandeja lateral glassmorphism que entra da direita para a esquerda no hover e expõe os controles de minimizar, alternar fullscreen e fechar.

**Objetivo:** Adicionar um painel secundário puramente visual, no mesmo clima glassmorphism, que apareça abaixo da bandeja principal após um pequeno atraso. Esse painel deve lembrar um módulo/painel de televisão premium, com várias opções empilhadas representadas apenas por emojis.

**Interação aprovada:** Ao receber hover, a bandeja principal abre imediatamente. Se o cursor continuar na área por cerca de `500ms`, um segundo painel vertical desce a partir da região central da bandeja principal. Quando o cursor sai, o temporizador é cancelado e o painel secundário some junto com a bandeja principal.

**Direção visual:** O painel secundário deve parecer parte do mesmo sistema visual da bandeja principal, usando blur, transparência, sombra e bordas claras suaves. Os itens internos serão cápsulas compactas empilhadas verticalmente, sem texto, com emojis centrais e brilho sutil no hover. O bloco `iRacerApp` / `v0.10` deve ganhar mais respiro para que o painel de widgets não atravesse visualmente essa área.

**Escopo atual:** Os widgets serão somente visuais nesta fase. Não haverá ações, backend novo, nem mudanças nos comandos Tauri existentes.

**Estrutura técnica:** A implementação pode continuar dentro de `src/components/layout/WindowControlsDrawer.jsx`, com um estado dedicado para a visibilidade do painel secundário e um `setTimeout` controlando o atraso de exibição. O componente precisa limpar o timer em `mouseleave` e no desmontar para evitar aparições fora de tempo.

**Validação:** Confirmar por teste de contrato do componente que o painel secundário, o atraso visual e a lista de widgets existem no código; depois rodar `npm run build` para garantir que a interface continua compilando.
