# Team Finance Economy Design

**Date:** 2026-04-13

## Goal

Transformar o `budget` de um rating abstrato em um sistema financeiro real, com caixa acumulado, dívida, estados financeiros, estratégias de investimento e impacto estrutural forte na competitividade das equipes, sem exigir microgestão do jogador.

## Player Experience

- O jogador não gerencia dinheiro diretamente.
- O sistema é visível e legível: a equipe do jogador e, depois, todas as equipes poderão expor informações financeiras.
- O dinheiro precisa gerar histórias de longo prazo:
  - equipes que poupam para subir no futuro
  - equipes que fazem all-in para evitar rebaixamento
  - equipes que sobem cedo demais, quebram e caem
  - equipes rebaixadas que usam o auxílio para se reconstruir

## Core Model

O centro do sistema deixa de ser `budget` e passa a ser:

- `cash_balance`
- `debt_balance`
- `financial_state`
- `season_strategy`
- `last_round_income`
- `last_round_expenses`
- `last_round_net`
- `parachute_payment_remaining`

`budget` pode continuar existindo temporariamente como leitura derivada/compatibilidade, mas deixa de ser a fonte real das decisões.

## Revenue Model

Entradas por rodada:

- `sponsorship_income`
  - depende de categoria, prestígio, forma recente e economia global
- `result_bonus`
  - depende do resultado da rodada e de overperformance
- `partial_prize_income`
  - adiantamento leve baseado na posição/projeção no campeonato
- `aid_income`
  - parachute payment, empréstimo emergencial, resgate excepcional

## Expense Model

Saídas por rodada:

- `salary_expense`
- `event_operations_cost`
- `structural_maintenance_cost`
- `technical_investment_cost`
- `debt_service_cost`
- `crisis_cost`

Na offseason entram custos maiores:

- projeto do carro da próxima temporada
- retenção/contratação de pilotos
- recuperação ou expansão de estrutura
- pagamento extraordinário de dívida

## Financial States

As equipes operam em 6 estados:

- `elite`
- `healthy`
- `stable`
- `pressured`
- `crisis`
- `collapse`

Cada estado altera:

- horizonte de planejamento
- tolerância à dívida
- agressividade esportiva
- prioridade de gasto
- disposição para especialização do carro
- prioridade de retenção de pilotos

Leituras aprovadas:

- `elite`: pensa em título e consistência
- `healthy`: investe com confiança
- `stable`: escolhe onde gastar
- `pressured`: precisa de resultado em breve, mas ainda tem margem
- `crisis`: aposta agora ou afunda devagar
- `collapse`: sobreviver é mais importante que competir

## Financial Health Score

O estado financeiro não depende só de caixa bruto. Ele é derivado de um score composto:

- posição de caixa
- estabilidade de receita
- pressão de dívida
- força estrutural
- resultados recentes

Isso evita leituras simplistas como “rico = saudável” ou “pobre = colapso”.

## How Money Becomes Performance

O dinheiro não melhora diretamente o carro. Ele compra capacidade de investimento, e o investimento é convertido em efeito com eficiência desigual.

Princípios:

- dinheiro define capacidade
- gestão define eficiência
- engenharia define conversão técnica
- pilotos definem extração em pista
- contexto define risco

Trilhas de impacto:

### Short term

- manutenção
- confiabilidade
- ajustes pequenos de performance
- qualidade operacional

### Medium term

- sustentação de upgrades
- retenção técnica
- projeto do carro atual/próximo

### Long term

- `engineering`
- `facilities`
- prestígio
- resiliência institucional

## Anti-Snowball Controls

O sistema precisa de freios para impedir dominância puramente financeira:

- retornos decrescentes no investimento
- custo alto para sustentar elite
- inflação competitiva
- variação técnica limitada
- economia global
- ambição custa caro

Também são necessários contrapesos não financeiros embutidos no modelo atual:

- `engineering`
- `facilities`
- `morale`
- `prestige`

Esses quatro atributos formam a eficiência de gestão derivada, sem criar um campo novo explícito.

## Debt And Crisis

Caixa negativo é permitido.

Consequências:

- dívida real com juros
- perda de liberdade estratégica
- piora do risco
- eventos fortes em estágios graves

Eventos aprovados:

- congelamento de upgrades
- venda de ativos
- saída de staff
- empréstimo emergencial
- investidor de resgate
- descoberta técnica rara
- reestruturação agressiva

## Promotion, Relegation, And Aid

Promoção:

- aumenta teto de receita
- aumenta custo operacional
- aumenta exigência competitiva
- pode punir equipes que sobem sem caixa

Rebaixamento:

- reduz exposição e teto esportivo
- reduz custos base
- concede `parachute_payment`
- pode salvar equipes quebradas

O auxílio de rebaixamento é intencionalmente forte o bastante para dar chance de reconstrução, mas não deve apagar má gestão.

## Global Economy

O save passa a ter uma camada macroeconômica:

- `boom`
- `neutral`
- `recession`

Ela afeta:

- receitas de patrocínio
- premiação
- salários
- custo operacional
- custo de desenvolvimento

Em recessão, receitas caem, mas custos também recuam para preservar competitividade relativa.

## Team Strategies

Além do estado financeiro, a IA escolhe uma postura de temporada/rodada:

- `expansion`
- `balanced`
- `austerity`
- `all_in`
- `survival`

Essas posturas distribuem investimento entre:

- carro
- confiabilidade/manutenção
- estrutura
- mercado
- reserva de caixa
- serviço da dívida

## Backend Architecture

O sistema deve ser introduzido em camadas:

- persistência nova no modelo `Team`
- novo módulo financeiro dedicado
- integração com fechamento de rodada
- integração com preseason/offseason
- integração com promoção/rebaixamento
- exposição de leitura ao frontend

Estrutura sugerida:

- `src-tauri/src/finance/mod.rs`
- `src-tauri/src/finance/economy.rs`
- `src-tauri/src/finance/cashflow.rs`
- `src-tauri/src/finance/state.rs`
- `src-tauri/src/finance/events.rs`

## Implementation Phases

1. caixa, dívida e fluxo por rodada
2. estados financeiros e estratégias da IA
3. impacto em carro, confiabilidade e estrutura
4. eventos fortes de crise/resgate
5. economia global
6. UI financeira detalhada para equipes

## Non-Goals For First Delivery

- microgestão manual de orçamento pelo jogador
- contratos detalhados de patrocinadores individuais
- sistema bancário profundo com múltiplas instituições
- UI comparativa completa de todas as equipes

Esses pontos podem vir depois, mas não devem bloquear a primeira versão do sistema.
