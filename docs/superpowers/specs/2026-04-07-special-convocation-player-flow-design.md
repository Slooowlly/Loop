# Special Convocation Player Flow Design

## Goal
Fechar o fluxo jogável de `Production` e `Endurance` de ponta a ponta, com reaproveitamento da UI atual, para que a fase especial deixe de ser apenas estrutural e vire uma parte coerente da carreira do jogador.

## Player-Facing Flow

### Janela de Convocação
- A `JanelaConvocacao` reaproveita a experiência visual da tela hoje usada para o fechamento de temporada.
- Nessa fase, a interface mostra apenas `Production` e `Endurance`.
- O foco da tela é:
  - classes e equipes especiais;
  - grids montados;
  - convocações recebidas pelo jogador.

### Decisão do jogador
- O jogador pode:
  - aceitar uma convocação especial;
  - recusar convocações individualmente;
  - ou simplesmente não entrar em nenhuma.
- Essa é a única decisão ativa do jogador na janela especial.
- Não existe mercado amplo nem renegociação livre nessa fase.

### Se o jogador aceitar
- O jogador passa a disputar integralmente o bloco especial.
- O dashboard normal e as telas que já dependem de categoria passam a olhar para `categoria_especial_ativa`.
- Não existe categoria regular rodando em paralelo durante o bloco especial.

### Se o jogador não entrar
- O jogador fica fora do bloco especial.
- Ao avançar, o jogo resolve rapidamente todas as corridas especiais em lote.
- A carreira é levada direto para o mercado normal de `dezembro/janeiro`.

## Backend State Model

### Convocação do grid
- O pipeline atual continua responsável por montar os grids completos de `Production` e `Endurance`.
- Isso permanece sendo a base esportiva do bloco especial.

### Convocações do jogador
- Além do grid completo, o sistema precisa produzir convocações específicas para o jogador.
- Essas convocações não devem ser misturadas com as propostas do mercado normal da pré-temporada.
- Elas precisam registrar:
  - equipe;
  - categoria especial;
  - classe;
  - papel;
  - status da convocação (`pendente`, `aceita`, `recusada`, `expirada`).

### Entrada no bloco especial
- Se o jogador aceitar uma convocação:
  - recebe um contrato especial ativo;
  - recebe `categoria_especial_ativa`;
  - passa a ter uma equipe especial ativa para o bloco.
- Se o jogador não aceitar nenhuma:
  - não recebe `categoria_especial_ativa`;
  - não entra em fluxo jogável do especial.

### Saída do bloco especial
- No fim do bloco:
  - contratos especiais expiram;
  - `categoria_especial_ativa` é limpa;
  - lineups especiais são desmontados;
  - a carreira retorna ao eixo normal de fim de ano.

## Simulation Rules

### Jogador dentro do especial
- O fluxo de `próxima corrida` precisa simular o bloco especial corrida a corrida para o jogador.
- A categoria ativa do jogador passa a ser a categoria especial aceita.
- As telas de standings, calendário, equipe e pilotos passam a refletir essa categoria.

### Jogador fora do especial
- O fluxo de avanço não deve obrigar o jogador a acompanhar o bloco especial.
- Um comando de simulação em lote resolve as corridas especiais pendentes.
- Depois disso, o sistema executa `encerrar_bloco_especial` e `run_pos_especial`.

## UI Reuse Strategy

### Janela de Convocação
- A nova UI não nasce como uma tela totalmente nova de mercado.
- O reaproveitamento prioritário é:
  - layout e densidade da experiência de `nextseason`;
  - estrutura de cards/listas que já apresentam equipes e pilotos.

### Dashboard e telas já existentes
- O dashboard normal continua sendo a entrada principal.
- Quando `categoria_especial_ativa` existir:
  - standings;
  - calendário;
  - equipe;
  - telas de piloto
  passam a resolver dados com base nela.

### Regra visual
- O jogador não enxerga campeonatos paralelos.
- Ou está no especial, ou está fora dele.

## Command Surface

### Novos comportamentos esperados
- Buscar estado da janela de convocação com foco no jogador.
- Buscar convocações especiais recebidas pelo jogador.
- Responder a uma convocação especial.
- Confirmar início do bloco especial.
- Simular em lote o bloco especial quando o jogador ficou fora.

### Ajuste importante
- A store já tenta chamar `simulate_special_block`.
- Esse comportamento precisa existir de verdade no backend, ou a store deve ser alinhada a um novo comando real com o mesmo papel.

## Responsibility Boundaries

### Backend esportivo
- `src-tauri/src/convocation/pipeline.rs`
- `src-tauri/src/commands/convocation.rs`
- `src-tauri/src/commands/race.rs`
- `src-tauri/src/commands/career.rs`
- `src-tauri/src/db/queries/contracts.rs`
- `src-tauri/src/db/queries/drivers.rs`

### Estado e fluxo de frontend
- `src/stores/useCareerStore.js`
- `src/pages/Dashboard.jsx`
- componente de convocação reaproveitando a base visual da transição de temporada

## Non-Goals
- Não criar um mercado especial completo.
- Não manter campeonato regular em paralelo.
- Não abrir uma aba nova de mercado só para especiais.

## Testing
- Validar que o jogador pode receber e responder convocações especiais.
- Validar que aceitar uma convocação ativa `categoria_especial_ativa`.
- Validar que o dashboard passa a consumir a categoria especial ativa.
- Validar que ficar fora do especial dispara simulação rápida até o mercado normal.
- Validar que `run_pos_especial` limpa corretamente o estado especial do jogador.
