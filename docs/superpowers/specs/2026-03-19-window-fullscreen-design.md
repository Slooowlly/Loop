# Window Fullscreen Design

**Contexto:** O aplicativo Tauri abre hoje com `maximized: true` e `decorations: false`, o que deixa a janela grande, mas ainda respeitando a área reservada da barra de tarefas do Windows.

**Objetivo:** Fazer o app abrir sempre em tela cheia nativa para que a barra de tarefas deixe de interferir na visualização.

**Abordagem escolhida:** Configurar a janela principal com `fullscreen: true` em `src-tauri/tauri.conf.json` e desativar o estado inicial `maximized`. Como a UI mostra um botão de maximizar/restaurar, o drawer também será ajustado para exibir apenas minimizar e fechar, evitando um controle que não combina com o novo modo padrão.

**Fluxo da janela:** Ao iniciar o app, a janela principal já nasce em fullscreen. O Windows passa a tratar a janela como tela cheia real, cobrindo a área da taskbar enquanto o app estiver aberto e focado.

**Riscos e limites:** O comportamento exato de exibição da taskbar continua dependente do sistema operacional e das configurações do usuário, mas fullscreen nativo é a forma correta de obter esse efeito no Windows. Não haverá atalho visual para sair de tela cheia, porque o pedido é manter o app sempre nesse modo.

**Validação:** Confirmar por build que a aplicação continua compilando e revisar a configuração final da janela para garantir `fullscreen: true` e `maximized: false`.
