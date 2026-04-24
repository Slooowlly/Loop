# News Tab Modular Text Pipeline Design

## Goal
Substituir o pipeline textual antigo da `Central de Noticias` por um pipeline modular, orientado por intenção, compatível com a estrutura visual atual do leitor principal e da coluna `Leituras do recorte`.

## Context
Hoje a `NewsTab` consome `NewsItem` legados gerados no backend. O snapshot da aba transforma cada item em `NewsTabStory`, usando:

- `item.titulo` -> `story.title`
- `item.texto` -> `story.body_text`
- `build_story_excerpt(item.texto)` -> `story.summary`

Esse fluxo ainda depende de textos narrativos longos pensados para uma UI anterior. A UI nova organiza a leitura em blocos curtos com intenção clara, então os textos antigos passaram a soar desalinhados com a estrutura dos cards.

## Approved Direction
Manter a estrutura visual do leitor principal com tres blocos, mas trocar completamente a origem textual. Em vez de depender de `titulo + texto longo`, cada story da `NewsTab` passara a ser montada como uma composicao modular:

- `headline`
- `deck` ou resumo curto
- `blocks[]`
- `meta`

Os labels dos blocos continuarao estaveis por tipo, enquanto o texto dentro deles tera variacoes.

## Type System
O novo pipeline cobre seis tipos editoriais:

1. `Corrida`
2. `Incidente`
3. `Piloto`
4. `Equipe`
5. `Mercado`
6. `Estrutural`

`Estrutural` cobre promocao, rebaixamento, aposentadoria e mudancas amplas de hierarquia.

## Block Model
Cada tipo usa tres blocos fixos por historia:

- `Corrida`: `Resumo`, `Impacto`, `Leitura`
- `Incidente`: `Ocorrido`, `Consequencia`, `Estado`
- `Piloto`: `Momento`, `Pressao`, `Sinal`
- `Equipe`: `Movimento`, `Resposta`, `Panorama`
- `Mercado`: `Movimento`, `Impacto`, `Proximo passo`
- `Estrutural`: `Mudanca`, `Efeito`, `Panorama`

Na UI, esses blocos continuam aparecendo no mesmo layout de tres secoes do card principal. O frontend deixa de inferir texto e passa apenas a renderizar o contrato pronto do backend.

## Variation Strategy
Cada tipo tera oito variacoes textuais.

Isso significa:

- 6 tipos
- 8 variacoes por tipo
- 48 conjuntos de copy modular

As variacoes mudam a formulacao do texto, mas nao o nome dos blocos. Assim, a interface segue coerente enquanto o texto ganha variedade suficiente para nao parecer repetitivo.

## Backend Contract
O contrato atual de `NewsTabStory` precisa evoluir de um modelo herdado para um modelo modular.

Contrato alvo:

- `id`
- `icon`
- `headline`
- `deck`
- `blocks: Vec<NewsTabStoryBlock>`
- `news_type`
- `importance`
- `importance_label`
- `category_label`
- `meta_label`
- `time_label`
- `entity_label`
- `driver_label`
- `team_label`
- `race_label`
- `accent_tone`
- `driver_id`
- `team_id`
- `round`

Novo subobjeto:

- `NewsTabStoryBlock`
  - `label`
  - `text`

Compatibilidade:

- `title` e `summary` podem ser mantidos temporariamente como alias de transicao se isso reduzir risco no frontend.
- `body_text` deve deixar de ser a fonte principal da renderizacao da aba.

## Generation Rules
O backend deve parar de empurrar a `NewsTab` para o texto legado puro e passar a montar stories a partir de um classificador editorial.

Passos logicos:

1. Classificar cada `NewsItem` em um dos seis tipos editoriais.
2. Extrair fatos-base do item:
   - tipo
   - importancia
   - categoria
   - rodada
   - piloto
   - equipe
   - contexto temporal
3. Escolher uma das oito variacoes daquele tipo.
4. Preencher `headline`, `deck` e os tres blocos modulares.
5. Entregar o resultado como `NewsTabStory`.

## Frontend Rendering
O frontend deixa de fazer inferencias textuais como:

- excerpt automatico para resumo
- bloco sintetico de `Por que importa`

Em vez disso:

- o card principal renderiza `headline`
- o texto curto de apoio renderiza `deck`
- os tres blocos renderizam `story.blocks`

A lateral `Leituras do recorte` pode continuar mostrando titulo curto + resumo curto, mas deve ler esses campos do novo contrato, nao do texto narrativo antigo.

## Migration Strategy
Para reduzir risco, a migracao deve ser incremental:

1. Adicionar o novo contrato modular ao backend.
2. Montar stories modulares na `NewsTab` sem remover imediatamente o legado.
3. Adaptar o frontend para preferir `blocks`.
4. Validar snapshot, filtros e troca local de stories.
5. Remover dependencia do excerpt e do texto sintetico no frontend.

Os geradores legados de `NewsItem` podem continuar existindo para outras areas do produto. O alvo desta mudanca e o pipeline da `NewsTab`, nao o sistema global de noticias como um todo.

## Testing
Cobertura minima esperada:

- contrato do snapshot novo no backend
- classificacao correta por tipo editorial
- selecao de bloco correto por tipo
- variacao escolhida dentro do intervalo valido
- renderizacao do frontend com `blocks`
- clique na lateral trocando apenas a story aberta
- fallback seguro quando algum campo estiver ausente

## Out of Scope
Ficam fora deste corte:

- reescrever o sistema global de armazenamento de `NewsItem`
- mudar a logica dos filtros de categoria/contexto
- reestruturar a aba `NextRaceTab`
- criar um CMS de copy externo

## Result
Ao final, a `Central de Noticias` deixa de parecer uma tela nova alimentada por texto antigo. O leitor principal e a lateral passam a usar um contrato textual nascido para a estrutura atual: modular, curto, intencional e variado.
