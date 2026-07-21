# Menu Debug nas Configurações

## Objetivo

Reduzir o ruído visual da tela de Configurações, mantendo as opções de desenvolvimento acessíveis somente quando o usuário abrir explicitamente o toggle `Menu Debug`.

## Comportamento aprovado

- Idioma, salvamento automático e bandeira amarela automática permanecem sempre visíveis.
- Mensagens de status ou erro da bandeira amarela permanecem visíveis junto à configuração normal.
- Uma nova linha `Menu Debug`, com controle visual de toggle, aparece logo abaixo das configurações normais.
- O toggle começa desligado toda vez que a tela de Configurações é montada.
- O estado não é persistido em arquivo, store ou backend.
- Com o toggle desligado, não são renderizados os detalhes técnicos da macro, comando de chat, quebra ao vivo, teste do overlay de rádio, gravação de corrida, fim de temporada e rivalidades percebidas.
- Ao ligar o toggle, todo esse conteúdo reaparece no mesmo painel e na mesma ordem atual.
- Fechar o menu apenas desmonta os controles e não chama ações de desligamento no backend. Uma gravação ou demo de overlay em andamento continua ativa e volta a ser refletida quando o menu é reaberto.
- Estados visuais internos e temporários do `RivalryPerceptionPanel` podem ser reiniciados quando o componente for desmontado, pois não representam uma configuração nem uma ação ativa no backend.

## Implementação

A mudança fica restrita à camada de UI em `src/pages/Settings.jsx`. Um estado React local controla a visibilidade, uma linha com semântica acessível de `switch` altera esse estado e um bloco condicional agrupa as ferramentas existentes. Nenhuma API Tauri ou regra de negócio será modificada.

## Testes

Um teste de componente deve confirmar que:

1. `Menu Debug` aparece desligado inicialmente.
2. Os sete grupos técnicos — detalhes, chat, quebra, overlay, gravação, fim de temporada e rivalidades — não aparecem no DOM enquanto o toggle está desligado.
3. Clicar no toggle exibe os sete grupos técnicos.
4. Clicar novamente volta a ocultá-los.
5. Desmontar e montar a tela novamente restaura `aria-checked="false"` e mantém os grupos ocultos.
6. Fechar o menu não invoca comandos para desligar o overlay ou parar uma gravação ativa; ao reabrir, os controles voltam a refletir esses estados ativos.

Além do teste focado, o build e a suíte de UI devem continuar passando.
