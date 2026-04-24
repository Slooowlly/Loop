# Window Drawer Exit Confirmation Design

## Goal
Simplificar a bandeja lateral para deixar apenas a acao `Home` e exigir confirmacao sempre que o jogador tentar:
- voltar ao menu principal
- fechar o app pelo `X`

## Decision
Usar um unico modal para os dois fluxos, com texto neutro e opcoes:
- `Salvar e sair`
- `Sair sem salvar`
- `Cancelar`

## Drawer
- Remover `Configuracoes`
- Remover `Carregar save`
- Manter apenas `Home`

## Confirmation Copy
- Titulo: `Deseja sair da carreira agora?`
- Corpo: `Voce pode salvar antes de voltar ao menu principal ou fechar o jogo.`

## Behavior
- `Home` sempre abre o modal
- `X` sempre abre o modal
- `Salvar e sair` executa `flushSave()` e depois conclui a acao
- `Sair sem salvar` conclui a acao sem `flushSave()`
- `Cancelar` fecha o modal sem sair
