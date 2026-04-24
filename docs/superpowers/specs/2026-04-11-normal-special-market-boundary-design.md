# Normal vs Special Market Boundary Design

## Goal
Separar formalmente o eixo `normal` do eixo `especial` na carreira, para que `Production` e `Endurance` existam apenas como convocacoes sazonais do meio do ano e nunca contaminem o mercado normal de fim de temporada.

## Problem Summary

### Sintoma na experiencia atual
- A `PreSeasonView` do mercado normal ainda mostra `Production` e `Endurance`.
- O mercado normal pode comunicar pilotos e equipes especiais como se fizessem parte do mesmo sistema de contratos fixos das categorias regulares.
- Isso quebra a fantasia central do especial:
  - parece que o piloto saiu da equipe regular;
  - parece que o piloto ficou indisponivel para o eixo normal;
  - parece que `Production` e `Endurance` sao categorias permanentes da carreira.

### Problema de dominio
- O contrato regular deveria permanecer intacto durante todo o ano.
- A convocacao especial deveria ser apenas um vinculo temporario paralelo.
- O mercado normal nao deveria enxergar:
  - categorias especiais;
  - lineups especiais;
  - contratos especiais;
  - historico especial como fonte principal de contexto.

## Approved Approach

- Tratar `Production` e `Endurance` como `categorias especiais de convocacao`, nao como categorias normais de mercado.
- Manter contrato regular e contrato especial como dois vinculos simultaneos com papeis diferentes.
- Fazer o mercado normal consultar apenas o eixo regular.
- Limitar a aparicao do eixo especial a:
  - `JanelaConvocacao`;
  - `BlocoEspecial`;
  - encerramento administrativo do `PosEspecial`.

## Annual Flow

### Eixo Regular
- `Fevereiro -> Agosto`: `BlocoRegular`
- `Dezembro -> Fevereiro`: mercado normal

### Eixo Especial
- `Agosto -> Setembro`: `JanelaConvocacao`
- `Setembro -> Dezembro`: `BlocoEspecial`
- `Fim de Dezembro`: `PosEspecial`

### Regra de leitura da UI
- Fora da janela e do bloco especial, a carreira deve se comportar como se `Production` e `Endurance` nao existissem.
- Durante o bloco especial, telas esportivas podem olhar para o estado especial ativo do jogador.
- O mercado normal nunca deve reutilizar a semantica do especial.

## State Model

### Contrato regular
- Continua sendo a fonte de verdade principal da carreira.
- Define:
  - equipe regular;
  - categoria regular;
  - historico normal;
  - elegibilidade do mercado normal;
  - continuidade do jogador com sua equipe principal.

### Contrato especial
- Representa uma convocacao sazonal temporaria.
- Nao substitui nem suspende o contrato regular.
- So precisa existir para:
  - ativar participacao no bloco especial;
  - definir equipe especial temporaria;
  - permitir queries do especial.

### Invariantes
- Um piloto pode ter contrato regular ativo e contrato especial ativo ao mesmo tempo.
- Aceitar convocacao especial nao abre vaga na equipe regular.
- O piloto continua pertencendo ao time regular o tempo inteiro.
- O fim do especial desmonta apenas o estado especial.
- Nao existe "transferencia de volta" ao final do especial.

## Query Boundaries

### Mercado normal
- Deve usar apenas contratos regulares e categorias regulares.
- Deve ignorar completamente:
  - contratos `Especial`;
  - `categoria_especial_ativa`;
  - grids especiais;
  - lineups especiais;
  - metadados de `Production` e `Endurance`.

### Historico para preseason e agentes livres
- A leitura de categoria anterior, ultimo time e tempo de casa do mercado normal deve usar historico regular.
- Contratos especiais expirados ou rescindidos nao podem se tornar o "ultimo contrato relevante" para o mercado normal.

### Especial
- Pode consultar:
  - contrato especial ativo;
  - `categoria_especial_ativa`;
  - grids especiais;
  - ofertas especiais do jogador.

## UI Boundaries

### PreSeasonView
- Remove `Production` e `Endurance` da navegacao e do mapeamento do mercado normal.
- Nao mostra equipes especiais.
- Nao mostra pilotos especiais como se fossem parte do grid regular.
- Nao usa movimentacoes especiais para badges, vagas ou agrupamentos.

### JanelaConvocacao
- Continua sendo o unico ponto de entrada visual para `Production` e `Endurance`.
- Mostra apenas:
  - equipes especiais;
  - classes especiais;
  - convocacoes do jogador;
  - status da participacao especial.

### Dashboard e telas de corrida
- Durante `BlocoEspecial`, o jogador pode ser resolvido pela camada especial.
- Fora de `BlocoEspecial`, o dashboard e as telas normais retornam ao contrato regular como referencia principal.

## Current Codebase Notes

### Comportamentos ja alinhados com a decisao
- A convocation ja separa ofertas especiais das propostas de mercado normais.
- O jogador nao ativa contrato especial antes de aceitar a convocacao.
- Os testes do pipeline especial ja cobrem que pilotos convocados mantem a categoria regular.
- A resolucao de `player_team` ja prefere contrato especial apenas em `BlocoEspecial`.

### Vazamentos ainda esperados
- A `PreSeasonView` ainda inclui `Production` e `Endurance` na lista do mercado normal.
- O payload de agentes livres da preseason usa o ultimo contrato expirado/rescindido sem filtrar `Regular` vs `Especial`, permitindo que o especial polua categoria e ultimo time exibidos no mercado normal.

## Responsibility Boundaries

### Backend de mercado normal
- `src-tauri/src/db/queries/contracts.rs`
- `src-tauri/src/commands/career.rs`
- `src-tauri/src/market/preseason.rs`

### Backend especial
- `src-tauri/src/convocation/pipeline.rs`
- `src-tauri/src/commands/convocation.rs`

### Frontend e estado
- `src/components/season/PreSeasonView.jsx`
- `src/components/season/PreSeasonView.test.jsx`
- `src/stores/useCareerStore.js`

## Non-Goals
- Nao transformar `Production` e `Endurance` em categorias regulares da carreira.
- Nao permitir mercado amplo durante a janela especial.
- Nao criar transferencias permanentes causadas pelo contrato especial.
- Nao misturar propostas especiais com propostas do mercado normal.

## Testing Strategy

### Backend
- Validar que `get_free_agents_for_preseason` ignora contratos especiais ao derivar categoria, ultimo time e tempo de casa.
- Validar que o mercado normal continua usando apenas contratos regulares.
- Validar que `player_team` continua especial apenas durante `BlocoEspecial`.

### Frontend
- Validar que `PreSeasonView` nao renderiza `Production` nem `Endurance`.
- Validar que o grid do mercado normal nao mostra equipes especiais.
- Validar que agrupamentos e badges do mercado normal usam apenas categorias regulares.

### Integracao
- Validar que aceitar convocacao especial nao abre vaga no time regular.
- Validar que encerrar o bloco especial limpa apenas o estado especial.
- Validar que, ao entrar no mercado normal, o jogador volta a ser apresentado exclusivamente pelo eixo regular.
