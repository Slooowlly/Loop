# Main Menu Entry Actions Design

## Objective
Alinhar os dois primeiros botões do menu inicial com nomes e ações explícitas antes de entrar no jogo.

## Current Behavior
O botão `ENTRAR` decide automaticamente entre abrir a tela de saves ou ir para configurações.
O botão `NOVA CARREIRA` abre diretamente o fluxo de criação.
Essa combinação deixa o primeiro botão com uma ação menos previsível do que o rótulo sugere.

## Approved Change
O menu inicial passa a exibir os botões na seguinte ordem:

1. `NOVA CARREIRA`
2. `CARREGAR SAVE`
3. `CONFIGURACOES`

As ações ficam explícitas:

- `NOVA CARREIRA` navega para `/new-career`
- `CARREGAR SAVE` navega para `/load-save`
- `CONFIGURACOES` permanece navegando para `/settings`

## Impact
O `MainMenu` deixa de depender da checagem automática de saves para o primeiro clique.
A tela `LoadSave` continua responsável por lidar com o caso em que não existam saves, exibindo o CTA para criar uma nova carreira.

## Validation
Validar que o build do frontend continua passando e que o menu renderiza os botões na ordem aprovada.
