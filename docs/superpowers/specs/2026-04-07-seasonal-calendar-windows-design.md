# Seasonal Calendar Windows Design

## Goal
Reorganizar o ano esportivo da carreira em janelas mensais legíveis para o jogador, de modo que o calendário, a pré-temporada e os blocos especiais passem a comunicar um ciclo anual intuitivo.

## Annual Structure

### Mercado Aberto Normal
- Acontece de `dezembro` até `fevereiro`.
- É o período mais longo e mais complexo do mercado.
- Deve transmitir a sensação de negociações prolongadas, propostas, renovações e redefinição de grid.

### Bloco Regular
- As corridas normais só podem acontecer entre `fevereiro` e `agosto`.
- O dia exato de cada corrida não importa, desde que permaneça dentro dessa janela.
- A distribuição das rodadas pode continuar variando por categoria, mas nunca deve escapar da faixa mensal.

### Janela Especial
- Acontece na virada de `agosto` para `setembro`.
- É curta e administrativa.
- Existe para refletir uma seleção mais rápida dos destaques do ano regular.

### Bloco Especial
- As corridas especiais só podem acontecer entre `setembro` e `dezembro`.
- Assim como no bloco regular, o dia exato é flexível.
- A janela especial deve ser claramente separada do mercado aberto normal.

### PosEspecial
- Permanece como fechamento administrativo rápido no fim de `dezembro`.
- Não vira um bloco longo.
- Serve para desmontagem do especial e preparação da entrada no mercado aberto seguinte.

## System Translation

### Calendário
- O backend continua podendo usar `week_of_year` como unidade interna.
- Porém, os ranges deixam de ser definidos por cortes fixos como `2..40` e `41..50`.
- A nova fonte de verdade passa a ser uma janela mensal por fase do ano esportivo.

### Pré-temporada
- A data da `PreSeasonView` continua vindo do backend.
- Ela precisa ficar sempre entre `dezembro` e `fevereiro`.
- A primeira corrida regular de `fevereiro` funciona como âncora final da pré-temporada.

### Convocação Especial
- A `JanelaConvocacao` continua sendo uma fase própria.
- Sua semântica muda de “faixa de semanas arbitrária” para “transição curta entre fim de agosto e começo de setembro”.

## Responsibility Boundaries

### Fonte principal da regra temporal
- `src-tauri/src/calendar/mod.rs`
- Esse módulo deve concentrar a conversão `fase -> janela mensal`.

### Consumo da regra temporal
- `src-tauri/src/market/preseason.rs`
- `src-tauri/src/convocation/pipeline.rs`
- `src-tauri/src/db/queries/calendar.rs`
- `src/components/season/PreSeasonView.jsx`

### Regra de design
- A lógica de janela mensal não deve ser espalhada em múltiplos arquivos.
- Os outros módulos devem depender de helpers centrais do calendário.

## Compatibility
- Saves existentes não precisam de migração estrutural de banco.
- Corridas já geradas em temporadas antigas podem permanecer como estão.
- A nova regra vale para temporadas novas e para novas pré-temporadas geradas a partir dela.

## Expected Player Outcome
- O jogador entende o ano esportivo sem precisar inferir a lógica.
- O mercado normal parece mais longo, mais carregado e mais importante.
- A janela especial parece curta, objetiva e reativa ao desempenho do ano.
- As datas da UI passam a conversar com a fantasia da simulação.

## Testing
- Validar que corridas regulares sempre caem entre `fevereiro` e `agosto`.
- Validar que corridas especiais sempre caem entre `setembro` e `dezembro`.
- Validar que a `PreSeasonView` sempre mostra datas entre `dezembro` e `fevereiro`.
- Ajustar testes que hoje dependem explicitamente dos ranges `2..40` e `41..50`.
