# Calendar Category Logos Design

**Goal:** Reforcar a identidade visual do calendario substituindo `R1/R2/R3` na celula principal de corrida por uma logo da categoria, mantendo a rodada disponivel no tooltip.

## Contexto

O calendario em `src/pages/tabs/CalendarTab.jsx` hoje usa a celula de corrida para mostrar:

- fundo com a imagem da pista;
- numero do dia centralizado;
- `R{rodada}` abaixo do dia;
- badges auxiliares como `Hoje`, `Esp` e o indicador de status.

O usuario quer que a informacao principal do dia deixe de ser a rodada e passe a ser a categoria da corrida. A rodada continua relevante, mas pode ficar apenas no tooltip porque a leitura do calendario deve priorizar identidade visual.

Essa mudanca vale apenas para a corrida principal daquele dia. Os indicadores de outras categorias devem permanecer como estao hoje, porque existe um trabalho paralelo para distribuir melhor as corridas entre os dias.

## Objetivo de UX

Cada dia com corrida principal deve ficar imediatamente reconhecivel pela categoria, sem sacrificar a leitura do numero do dia nem o detalhamento do tooltip.

Resultado esperado:

- a logo da categoria vira o foco visual da celula;
- o dia continua visivel, mas como selo discreto;
- `R1/R2/R3` desaparece da celula;
- tooltip preserva rodada, pista, duracao, clima e status;
- comportamento atual de hover, tooltip e outras categorias continua intacto.

## Solucao Visual

Para celulas com corrida principal:

- remover o texto `R{rodada}` da area central;
- renderizar a logo da categoria no centro da celula, por cima do fundo da pista;
- mover o numero do dia para um selo pequeno no canto superior direito;
- manter `Hoje` no canto superior esquerdo;
- manter `Esp` no canto inferior esquerdo para corridas especiais;
- manter os dots das outras categorias no canto inferior direito quando existirem.

Diretrizes visuais:

- a logo central deve ter destaque, mas sem colidir com os badges de canto;
- o selo do dia precisa ter fundo e contraste suficientes para pistas claras e escuras;
- em telas pequenas, a logo deve respeitar um tamanho maximo para nao poluir a celula;
- o estado `Hoje` continua podendo sobrepor a celula com maior destaque visual, sem esconder a logo completamente.

## Solucao Tecnica

### Assets

Servir as logos como assets publicos em `/categorias/...`, seguindo o mesmo padrao ja usado por `/tracks/...`.

As imagens fornecidas em `image/categorias` devem ser disponibilizadas em `public/categorias` com nomes estaveis.

### Mapeamento

Criar um mapa `CATEGORY_LOGOS` no frontend, inicialmente no proprio `CalendarTab.jsx`, com entradas para:

- `mazda_rookie`
- `toyota_rookie`
- `mazda_amador`
- `toyota_amador`
- `bmw_m2`
- `production_challenger`
- `gt4`
- `gt3`
- `endurance`

Cada categoria aponta para o arquivo correspondente em `/categorias/...`.

### Renderizacao

Na `DayCell`:

- quando `race` existir, buscar `CATEGORY_LOGOS[race.categoria]`;
- se houver logo mapeada, renderizar a imagem como elemento central da celula;
- se nao houver logo, manter a celula funcional com fundo da pista + selo do dia, sem quebrar layout;
- nao alterar o conteudo do tooltip, apenas a apresentacao da celula.

## Regras e Limites

- Apenas a corrida principal do dia recebe a nova apresentacao.
- Os marcadores de outras categorias nao entram nesta mudanca.
- O tooltip continua sendo a fonte da rodada.
- A implementacao deve preservar o comportamento atual de hover, tooltip e destaque do dia atual.
- A mudanca deve ser incremental e localizada para reduzir risco em um arquivo que ja concentra bastante logica visual.

## Testes

Cobrir ao menos estes cenarios em `src/pages/tabs/CalendarTab.test.jsx`:

- celula principal nao renderiza mais `R1/R2/R3`;
- celula principal renderiza a logo da categoria esperada;
- tooltip da corrida principal continua exibindo `R{rodada}`;
- dias com outras categorias continuam mostrando os indicadores secundarios como antes.

## Fora de Escopo

- alterar a logica de distribuicao de corridas entre datas;
- redesenhar tooltip;
- substituir os dots de outras categorias por logos;
- refatorar estruturalmente o calendario alem do necessario para a nova celula.
