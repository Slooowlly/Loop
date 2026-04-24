# Calendar Convocation Week Design

## Goal
Integrar a `JanelaConvocacao` diretamente ao grid da aba `Calendario`, removendo o card separado e usando uma semana real do calendário com cor própria, sem perder o marcador amarelo do dia atual.

## Approved Direction
- Remover o card `Janela de Convocação` da aba `Calendario`.
- Mostrar a janela como `7 dias reais` dentro do mês correspondente.
- Manter a semana de convocação com identidade visual própria.
- Exibir também as corridas futuras do bloco especial dentro das datas reais do calendário.
- Manter o destaque amarelo de `Hoje` acima da cor da convocação quando ambos coincidirem.

## Data Model
- O calendário principal continua vindo de `get_calendar_for_category` para a categoria regular do jogador.
- Quando existir `acceptedSpecialOffer.special_category`, a aba também carrega o calendário dessa categoria especial.
- A semana de convocação é ancorada na primeira corrida especial conhecida.
- Se ainda não houver corrida especial carregada, a UI usa um fallback determinístico do ano da temporada para manter a leitura visual da semana.

## UI Behavior
- Dias da semana de convocação recebem um estado visual exclusivo no grid.
- Corridas especiais continuam sendo células de corrida normais, mas diferenciadas como parte do bloco especial.
- O texto/card separado de convocação deixa de existir.
- A legenda do calendário passa a explicar a nova cor da convocação.

## Testing
- Cobrir o fetch do calendário especial.
- Cobrir a presença da semana de convocação no grid.
- Cobrir a remoção do card separado.
- Cobrir a prioridade visual do marcador amarelo sobre a semana de convocação.
