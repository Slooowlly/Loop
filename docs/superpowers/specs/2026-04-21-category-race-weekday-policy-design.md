# Category Race Weekday Policy Design

## Goal
Dar identidade temporal para cada série do calendário da carreira, substituindo a regra implícita de "todas as corridas no sábado" por uma política estável de dia da semana por categoria.

## Context
- O projeto já separa o ano esportivo em janelas mensais:
  - bloco regular entre `fevereiro` e `agosto`
  - bloco especial entre `setembro` e `dezembro`
- Hoje o backend ainda ancora a geração visual no `sábado`, mesmo quando a fantasia esportiva da categoria sugeriria outro ritmo.
- O objetivo aqui não é introduzir treino, classificação ou fim de semana multi-dia.
- Cada rodada continua sendo um evento único com `display_date` próprio.

## Approved Weekday Policy

### Bloco Regular
- `mazda_rookie` e `toyota_rookie` usam um dia estável entre `segunda` e `terça`.
- `mazda_amador`, `toyota_amador` e `bmw_m2` usam um dia estável entre `quarta` e `quinta`.
- `gt4` acontece sempre no `sábado`.
- `gt3` acontece sempre no `domingo`.

### Bloco Especial
- `production_challenger` e `endurance` tentam usar `domingo` como dia principal.
- Se a janela especial não tiver domingos suficientes para acomodar todas as rodadas, o sistema pode usar dias alternativos.
- Esse overflow não cria conflito narrativo com `gt3`, porque o bloco especial não convive com corridas regulares no mesmo calendário ativo.

## Stability Rule
- O dia da semana é escolhido uma vez por categoria na geração da temporada.
- Depois de escolhido, ele vale para todas as rodadas daquela categoria naquela temporada.
- A categoria não pode alternar de dia entre rodadas dentro da mesma temporada.
- Em temporadas diferentes, categorias com faixa permitida podem cair em dias diferentes dentro da própria faixa.

### Examples
- Em uma temporada, `mazda_rookie` pode ficar toda na `segunda` e `toyota_rookie` toda na `terça`.
- Em outra temporada, isso pode inverter.
- `gt4` e `gt3` não variam: continuam sempre em `sábado` e `domingo`.

## Special Overflow Policy
- A prioridade de geração do bloco especial é:
  1. `domingo`
  2. dias alternativos apenas quando os domingos forem insuficientes
- O overflow existe para garantir encaixe de `production_challenger` e `endurance` dentro da janela `setembro-dezembro`.
- O sistema não precisa evitar o mesmo dia da semana do `gt3`, porque as fases são separadas.
- O overflow deve preservar a leitura de campeonato premium; ele é exceção operacional, não comportamento normal.

## System Translation

### Separação de responsabilidades
- `season window` continua definindo em quais meses a categoria pode correr.
- `weekday policy` passa a definir em qual dia da semana a categoria corre dentro dessa janela.

### Backend
- A geração do calendário precisa deixar de tratar `sábado` como padrão universal de `display_date`.
- O gerador deve resolver:
  - a janela mensal da fase
  - o dia permitido da categoria
  - a data visual final de cada rodada
- `week_of_year` continua sendo a base de ordenação temporal.
- `display_date` continua sendo a data narrativa consumida pela UI, pelas notícias e pelos resumos temporais.

### Frontend
- O frontend deve continuar lendo `display_date` como fonte de verdade.
- Nenhuma tela deve presumir que corrida regular ou especial cai sempre no sábado.
- Calendário, header, próxima corrida, briefing e notícias devem refletir o dia real calculado no backend.

## UX Outcome
- O jogador passa a reconhecer a hierarquia das séries pelo ritmo semanal.
- Rookies parecem categorias de base que vivem no começo da semana.
- Amador e BMW ocupam o meio da semana, sugerindo um passo acima da base.
- `gt4` ganha cara de evento de sábado.
- `gt3` vira o grande evento dominical do bloco regular.
- O bloco especial continua premium, mesmo com overflow eventual.

## Compatibility
- Saves existentes não exigem migração estrutural.
- Calendários já persistidos podem permanecer como estão.
- A nova política vale para temporadas geradas depois da mudança.

## Invariants
- Cada categoria regular usa um único dia estável por temporada.
- `mazda_rookie` e `toyota_rookie` nunca saem de `segunda-terça`.
- `mazda_amador`, `toyota_amador` e `bmw_m2` nunca saem de `quarta-quinta`.
- `gt4` nunca sai de `sábado`.
- `gt3` nunca sai de `domingo`.
- Corridas regulares nunca saem da janela `fevereiro-agosto`.
- Corridas especiais nunca saem da janela `setembro-dezembro`.
- O fallback especial só entra quando os domingos da janela forem insuficientes.
- A UI sempre mostra a `display_date` real, sem hardcode de sábado.

## Testing
- Validar que cada categoria respeita sua faixa aprovada de dia da semana.
- Validar que categorias com faixa variável escolhem um dia estável por temporada.
- Validar que `gt4` sempre gera `sábado`.
- Validar que `gt3` sempre gera `domingo`.
- Validar que especiais tentam `domingo` primeiro.
- Validar que o overflow especial só aparece quando necessário.
- Validar que a janela mensal continua correta para blocos regular e especial.
- Ajustar testes de calendário e UI que ainda assumem sábado como data padrão.
