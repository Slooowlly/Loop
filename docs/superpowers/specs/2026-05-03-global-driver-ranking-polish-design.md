# Global Driver Ranking Polish Design

## Goal

Lapidar o ranking mundial de pilotos para parecer responsivo, mais informativo e mais facil de usar, mantendo a tela atual como base.

## User Experience

Quando o usuario der duplo clique em um piloto na classificacao, a aba global deve abrir imediatamente com uma tela de carregamento. Essa tela deve mostrar que o ranking mundial esta sendo calculado, para evitar a sensacao de travamento enquanto o backend prepara o payload.

Depois do carregamento, o topo continua mostrando o piloto em foco. Clicar em qualquer piloto da tabela passa a focar esse piloto no card superior e destaca sua linha. O piloto do usuario tambem deve ter uma enfase permanente propria, com um marcador visual como `Voce`, para ser encontrado rapidamente mesmo quando outro piloto estiver em foco.

## Ranking Rows

A coluna `Equipe/Categoria` deve mostrar as duas informacoes sempre que possivel:

- Piloto ativo com equipe: `Nome da Equipe / Mazda Rookie`
- Piloto livre com categoria conhecida: `Livre / Mazda Rookie`
- Piloto livre sem categoria conhecida: `Livre`
- Piloto aposentado: `Aposentado / GT3`

O status deve continuar com os tres estados atuais: `Ativo`, `Livre` e `Aposentado`. Para aposentados, o texto visivel deve ser `Aposentado ha X anos`. Ao passar o mouse no status, o tooltip deve mostrar `Aposentado em YYYY`.

## New Data

O backend deve incluir no payload global:

- `salario_anual`
- `ano_inicio_carreira`
- `anos_carreira`
- `temporada_aposentadoria`
- `anos_aposentado`

Para pilotos ativos, o salario vem do contrato ativo. Para pilotos livres e aposentados sem salario conhecido, a UI deve mostrar `-`.

`anos_carreira` deve ser calculado a partir do ano atual da carreira quando disponivel. Para aposentados, deve preferir o snapshot historico caso exista; se nao houver dado suficiente, mostrar `-`.

## Ranking Eligibility

Pilotos sem historico competitivo relevante nao devem aparecer no ranking mundial. Um piloto deve ser excluido quando todos estes campos estiverem zerados ou ausentes:

- indice historico
- corridas
- pontos
- titulos
- vitorias
- podios
- poles
- DNFs

Lesoes nao devem manter um piloto no ranking se todo o resto competitivo estiver zerado.

## UI Columns

A tabela deve continuar compacta, mas ganhar as informacoes pedidas:

- Idade
- Carreira
- Salario

Essas colunas devem usar formatos curtos, por exemplo `25`, `9 anos` e `$250k` ou formato monetario ja usado no projeto.

## Error And Loading States

A tela de carregamento deve aparecer tanto na entrada inicial da aba quanto em recarregamentos do payload. O erro atual pode continuar como alerta textual, mas sem apagar o botao de voltar.

## Tests

Cobertura esperada:

- loading aparece antes do payload resolver;
- pilotos sem historico competitivo sao filtrados;
- `Equipe/Categoria` mostra equipe e categoria juntas;
- aposentado mostra `Aposentado ha X anos` e tooltip com `Aposentado em YYYY`;
- clique em uma linha altera o piloto em foco;
- piloto do usuario recebe destaque proprio mesmo sem estar em foco;
- idade, carreira e salario aparecem na tabela.
