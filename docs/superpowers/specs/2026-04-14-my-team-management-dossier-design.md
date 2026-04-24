# My Team Management Dossier Design

**Date:** 2026-04-14

## Goal

Reformular a aba `Minha Equipe` para transmitir uma leitura clara de gestao da escuderia, com foco principal em financas e operacao da temporada, mantendo identidade esportiva suficiente para o jogador reconhecer sua dupla e a base tecnica do time.

## Chosen Direction

A direcao aprovada parte da antiga opcao `B`, mas com tres ajustes centrais:

- o `ranking da categoria` deixa de competir com o topo da tela e desce para o final como fechamento comparativo;
- o bloco financeiro deixa de ser acessorio e vira o centro da tela;
- os modulos de `dupla de pilotos` e `operacao tecnica` ficam menores, servindo como contexto e nao como protagonistas.

O resultado desejado e uma tela que pareca uma `central de gestao da equipe`, mesmo sem o jogador tomar decisoes diretas de dinheiro.

## Experience Principles

- A aba deve deixar claro que a equipe e uma operacao viva, com entradas, saidas, pressao de caixa e desempenho esportivo conectados.
- O usuario deve conseguir entender `o que entrou`, `o que saiu`, `como o caixa evoluiu` e `como a equipe se posiciona contra o grid`.
- O bloco financeiro deve parecer um `dossie de caixa`, nao apenas um conjunto de KPIs soltos.
- O ranking deve funcionar como um espelho final: primeiro o usuario entende sua operacao, depois compara essa operacao com as demais equipes.

## Layout Structure

### 1. Hero de gestao

Topo com:

- nome da equipe;
- breve leitura textual da situacao atual;
- metricas compactas de alto impacto como `posicao`, `pontos` e `estado financeiro`.

Esse hero ancora a narrativa da tela: a equipe esta viva, competitiva e inserida num contexto economico.

### 2. Coluna lateral compacta

Blocos menores para:

- `dupla de pilotos`;
- `operacao tecnica` (`nivel do carro`, `confiabilidade`, `pit crew`, `risco de pit`).

Esses blocos devem ocupar menos espaco vertical e visual do que na exploracao inicial, servindo como contexto da equipe, nao como centro do layout.

### 3. Dossie financeiro central

Bloco principal da tela, com peso visual dominante.

Ele combina duas camadas:

#### a. Extrato por categorias

Separar `rodada atual` em entradas e saidas, com linhas claras para:

- patrocinio;
- bonus de resultado;
- premio parcial;
- auxilios extraordinarios;
- salarios;
- custo operacional do evento;
- manutencao estrutural;
- investimento tecnico;
- servico da divida.

Tambem mostrar cards de resumo como:

- caixa atual;
- resultado da rodada;
- teto salarial;
- poder de gasto ou leitura equivalente.

#### b. Leitura acumulada

Mostrar acumulado da temporada via:

- linha do tempo do `caixa acumulado`;
- grafico de composicao de custos;
- pequenos resumos textuais que transformem numero em interpretacao.

## Timeline Priority

Foi aprovado que a `linha do tempo do caixa acumulado` deve ganhar mais espaco do que na primeira versao refinada.

Isso implica:

- ampliar sua largura;
- tornar o grafico mais legivel rodada a rodada;
- reduzir o peso visual dos modulos laterais para liberar espaco;
- posicionar a timeline como leitura principal do acumulado sazonal.

## Ranking Placement

O `ranking da categoria` deve ficar no final da aba.

Formato aprovado:

- tabela seca;
- foco em `Equipe`, `Dinheiro`, `Nivel do carro`, `Tipo do carro`, `Pontos`;
- leitura comparativa objetiva;
- destaque visual para a linha da equipe do jogador.

O ranking nao deve competir com o dossie financeiro no topo/meio da tela.

## Data Scope

O financeiro aprovado combina:

- `rodada atual`;
- `acumulado da temporada`.

Nao e so um resumo acumulado nem so um snapshot do ultimo evento. A tela deve mostrar os dois e deixar isso explicito na rotulagem.

## Visual Tone

- escuro, tecnico e limpo;
- mais `sala de gestao` do que `showroom`;
- graficos claros e funcionais;
- poucos efeitos decorativos;
- hierarquia visual puxada por informacao economica, nao por glamour.

## Approved Mockup Direction

Versao aprovada:

- base da `Opcao B`
- ranking no fim
- financeiro dominante
- extrato por categorias
- linha do tempo de caixa acumulado
- graficos auxiliares
- dupla de pilotos e operacao tecnica compactadas

## Implementation Notes

- A `MyTeamTab` atual ja expoe parte da leitura financeira (`cash_balance`, `debt_balance`, `spending_power`, `salary_ceiling`, `last_round_net`, `financial_state`, `season_strategy`), mas a direcao aprovada pede granularidade maior.
- O backend/spec financeiro ja define campos como `last_round_income` e `last_round_expenses`, alem de categorias aprovadas de receita e custo. Isso sustenta a evolucao natural para um dossie mais detalhado.
- O ranking no fim pode reutilizar a base visual/comportamental de comparativos ja existentes em `StandingsTab`, desde que adaptado para a linguagem de `Minha Equipe`.
