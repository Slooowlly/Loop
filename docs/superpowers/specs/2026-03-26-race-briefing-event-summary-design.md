# Refinamento do Resumo do Evento no Briefing

## Objetivo

Dar mais presença ao bloco `Resumo do evento` no briefing pre-corrida, mantendo a mesma linguagem visual do produto e priorizando leitura rapida.

## Direcao aprovada

- Base visual na `faixa compacta expandida`.
- Topo do bloco com:
  - `Data do evento` em destaque
  - `Horario local` ao lado, com enfase no periodo do dia
- Faixa inferior com tres informacoes editoriais:
  - `Publico`
  - `Cobertura` ou `Expectativa`
  - `Historico`

## Regras de conteudo

- `Publico` deve mostrar o numero principal e um subtitulo com o ranking relativo da etapa dentro da temporada.
- `Cobertura` mostra `Ao vivo` sem subtexto apenas em etapas importantes.
- Quando a etapa nao for importante o suficiente para transmissao, o card troca para `Expectativa` e mostra uma leitura curta da equipe.
- `Historico` deve ser curto e legivel, usando a melhor informacao real disponivel no jogo.

## Regra simples para cobertura ao vivo

Sem criar um sistema complexo novo agora, considerar `Ao vivo` quando a etapa cumprir pelo menos uma destas condicoes:

- abertura do campeonato
- final do campeonato
- evento com interesse `Evento principal`

## Fallbacks honestos

- Se nao houver historico rico por circuito disponivel no briefing, usar o historico competitivo real ja carregado para o jogador.
- O texto nao deve prometer granularidade por pista quando essa informacao nao estiver disponivel.

## Impacto esperado

- O card deixa de parecer pequeno dentro do briefing.
- O resumo ganha mais identidade editorial.
- O usuario entende melhor o peso da etapa logo no topo da coluna esquerda.
