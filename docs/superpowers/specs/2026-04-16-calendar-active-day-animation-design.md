# Calendar Active Day Animation Design

**Goal**

Adicionar na aba `Calendario` uma animacao visual durante `Avancar calendario`, destacando apenas o dia ativo enquanto a data caminha ate o proximo evento.

**Context**

- O fluxo de dias ja existe na store em `startCalendarAdvance`, atualizando `calendarDisplayDate` e `displayDaysUntilNextEvent`.
- Hoje o header mostra a data andando, mas a aba `Calendario` nao reage visualmente a esse estado.
- O preview aprovado foi a variacao "Opcao C: foco com trilha superior", mais informativa e mais chamativa.

**Core Decisions**

- A animacao aparece apenas quando a aba ativa e `calendar`.
- O calendario continua estatico; quem se move e o destaque do dia ativo.
- O dia ativo ganha um tratamento dourado com brilho proprio.
- O card do mes que contem o dia ativo ganha uma trilha superior de progresso para reforcar o movimento.
- Corridas ja concluidas continuam verdes; a proxima corrida continua azul quando nao houver animacao em curso.

**UI Behavior**

- Quando `isCalendarAdvancing` estiver ativo e a aba `Calendario` estiver aberta, a data visivel em `calendarDisplayDate` vira a fonte de verdade do destaque.
- O mes que contem essa data recebe uma trilha superior preenchida conforme a animacao avanca entre a data inicial e a data alvo.
- A celula do dia ativo ganha:
  - overlay dourado;
  - leve escala;
  - sombra/brilho quente;
  - ponto escuro no canto para diferenciar do azul da proxima corrida.
- Quando a animacao termina e o briefing abre, o estado visual volta ao comportamento normal.

**Data Flow**

- `Dashboard` passa `activeTab` para `CalendarTab`.
- `CalendarTab` le `calendarDisplayDate` e `isCalendarAdvancing` da store.
- Helpers locais derivam:
  - data ativa parseada;
  - se a animacao deve estar ativa;
  - qual mes/dia esta em foco;
  - progresso da trilha no mes ativo.

**Testing**

- Criar teste de frontend para garantir que a aba destaca o dia ativo quando `activeTab="calendar"` e `isCalendarAdvancing=true`.
- Cobrir que a celula ativa recebe marcadores visuais especificos e que o mes ativo recebe a trilha superior.
