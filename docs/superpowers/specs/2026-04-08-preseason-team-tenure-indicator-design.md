# Indicador Compacto de Permanencia no Mapeamento das Equipes

## Objetivo

Refinar o indicador visual de permanencia dos pilotos no mapeamento das equipes da pre-temporada para reduzir ruído visual. Em vez de badges grandes com texto, a interface deve mostrar apenas um contador compacto de temporadas (`1T`, `2T`, `3T`) e usar cor sutil para destacar quem acabou de chegar ao time.

## Decisao

- Mostrar o total de temporadas consecutivas do piloto na equipe como um contador compacto.
- Usar `1T` para novatos no time, mas sem badge textual `Novo`.
- Destacar novatos com acento visual leve:
  - nome com tom azul;
  - borda do slot levemente azulada.
- Manter pilotos com mais tempo em estilo neutro, com contador discreto.

## Impacto

- O grid continua comunicando continuidade e renovacao.
- A leitura fica mais leve e menos carregada visualmente.
- O backend nao precisa mudar: o campo de permanencia por piloto ja existe no payload do grid.

## Validacao

- Atualizar teste do `PreSeasonView` para esperar `1T` e `3T`.
- Confirmar que o slot de novato continua identificado sem depender de texto longo.
