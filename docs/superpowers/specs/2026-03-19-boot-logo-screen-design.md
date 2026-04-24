# Boot Logo Screen Design

**Contexto:** O app hoje começa diretamente na splash screen em `src/pages/SplashScreen.jsx`. O usuário quer uma etapa anterior, mostrando a nova logo do app antes da tela inicial.

**Objetivo:** Adicionar uma tela de boot cinematográfica com a logo do app, exibida por cerca de `2s`, antes da splash atual.

**Interação aprovada:** A abertura terá duas etapas. Primeiro, uma tela dedicada mostra apenas a logo, com entrada suave e ambiente visual coerente com o app. Após aproximadamente `2s`, essa tela navega automaticamente para a splash atual, que continua com o botão `Entrar`.

**Direção visual:** Fundo escuro com brilho sutil azul/violeta, logo centralizada, animação leve de `fade + scale`, sem botões nem texto competindo com a marca. A intenção é parecer um “boot” elegante, não uma tela de loading genérica.

**Arquitetura recomendada:** Criar uma nova página de boot e ajustar o roteamento para que `/` seja essa tela. A splash atual passa para uma rota seguinte, como `/splash`. A imagem da logo deve vir de um asset estável do frontend, separado do pipeline de ícones do Tauri.

**Assunção de UX:** Como a tela de boot precisa ficar limpa, a bandeja global de janela deve ficar oculta nas rotas de abertura (`/` e `/splash`). Nas demais rotas, ela continua disponível.

**Validação:** Confirmar que a tela de boot é a primeira rota, que ela navega automaticamente para a splash após `2s`, que a splash continua funcional e que o drawer global não aparece nas rotas de abertura. Depois, rodar `npm run build`.
