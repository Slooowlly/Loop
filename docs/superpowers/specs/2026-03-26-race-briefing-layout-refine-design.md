# Refinamento do Layout do Briefing Pre-Corrida

## Objetivo

Enxugar o briefing pre-corrida para privilegiar leitura rapida e separar melhor narrativa editorial de informacoes competitivas.

## Decisoes

- Remover o bloco `Momento da etapa`.
- Transformar `Data do evento` e `Publico/interesse` em um unico card-resumo, usando linhas no estilo visual de `Condicoes`.
- Concentrar a coluna esquerda em conteudo editorial:
  - titulo da etapa
  - resumo do evento
  - previa da corrida
  - voz do box
  - condicoes
  - metas
- Manter a coluna direita apenas com:
  - favoritos ao podio
  - contexto da etapa
- Tirar o botao `Voltar` de dentro do card do topo no header, deixando-o fora do card e visivel apenas durante o briefing.

## Impacto esperado

- Menos redundancia visual.
- Leitura mais clara da parte narrativa.
- Header mais leve durante o briefing.
