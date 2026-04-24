# Next Race Countdown Units Design

## Goal
Trocar o texto do topo `Proxima corrida` para uma leitura temporal mais editorial:
- meses quando a etapa ainda estiver distante
- semanas no meio do caminho
- dias na reta final

## Decision
Usar apenas o `days_until_next_event` que o frontend ja recebe, sem alterar backend nem duracao da animacao do calendario.

## Rules
- `<= 0 dias`: `Proxima corrida hoje`
- `1 dia`: `Proxima corrida amanha`
- `2 a 7 dias`: `Proxima corrida em X dias`
- `8 a 27 dias`: `Proxima corrida em X semanas`
- `28 a 55 dias`: `Proxima corrida em 1 mes`
- `56+ dias`: `Proxima corrida em X meses`

## Notes
- A contagem continua simples e curta, sem combinar unidades (`1 mes e 2 semanas`).
- A animacao de avancar calendario continua a mesma; o ganho aqui e narrativo e visual.
