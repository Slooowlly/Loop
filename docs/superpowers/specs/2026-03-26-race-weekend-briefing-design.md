# Race Weekend Briefing Design

**Date:** 2026-03-26

## Goal

Dar visibilidade clara da passagem do tempo no dashboard e substituir o atalho direto de simular corrida por um fluxo mais narrativo:

1. topo direito com data atual da carreira e CTA de avanço temporal
2. animação de avanço dia a dia até a próxima corrida
3. abertura de uma aba própria de briefing pré-corrida
4. ações separadas para `Simular corrida`, `Exportar` e `Voltar`

## Contexto Atual

- O dashboard já renderiza um header global em [Header.jsx](/c:/Users/rodri/OneDrive/Área%20de%20Trabalho/Jogos/iRacerApp%20V1/src/components/layout/Header.jsx).
- O store global já expõe `season`, `nextRace`, `simulateRace()` e estados de carregamento em [useCareerStore.js](/c:/Users/rodri/OneDrive/Área%20de%20Trabalho/Jogos/iRacerApp%20V1/src/stores/useCareerStore.js).
- Já existe uma tela de `Próxima corrida` em [NextRaceTab.jsx](/c:/Users/rodri/OneDrive/Área%20de%20Trabalho/Jogos/iRacerApp%20V1/src/pages/tabs/NextRaceTab.jsx), que hoje funciona mais como staging area de simulação.
- O backend já conhece a próxima corrida e a data do evento via `next_race.display_date` em [career.rs](/c:/Users/rodri/OneDrive/Área%20de%20Trabalho/Jogos/iRacerApp%20V1/src-tauri/src/commands/career.rs).
- O backend também já possui um resumo temporal da temporada em [temporal.rs](/c:/Users/rodri/OneDrive/Área%20de%20Trabalho/Jogos/iRacerApp%20V1/src-tauri/src/models/temporal.rs) e [calendar.rs](/c:/Users/rodri/OneDrive/Área%20de%20Trabalho/Jogos/iRacerApp%20V1/src-tauri/src/commands/calendar.rs).

## UX Aprovada

### 1. Bloco temporal no topo direito

O canto superior direito do dashboard passa a mostrar:

- `Data DD/MM/AAAA`
- subtítulo adaptativo do próximo evento:
  - `Próxima corrida hoje`
  - `Próxima corrida amanhã`
  - `Próxima corrida em 3 dias`
  - `Próxima corrida em 1 semana`
- botão principal `Avançar calendário`

Esse botão não simula a corrida. Ele apenas avança o calendário até a data do próximo evento do jogador.

### 2. Animação de avanço do tempo

Ao clicar em `Avançar calendário`:

- a UI entra num estado bloqueado temporário
- a data atual da carreira anima dia a dia até alcançar a data da corrida
- a animação dura cerca de 5 segundos
- o texto relativo do próximo evento acompanha o avanço
- ao final, a UI abre a aba própria de briefing

Essa animação é visual e narrativa. Ela não precisa persistir cada dia no backend. O estado persistente relevante continua sendo “estamos na próxima corrida”.

### 3. Aba própria de briefing pré-corrida

Após o avanço temporal, o dashboard abre uma aba/tela própria de briefing em estilo box/paddock.

Hierarquia visual aprovada:

- cabeçalho do evento
  - etapa atual, ex.: `Etapa 5 de 20`
  - nome do circuito
  - data do evento
  - clima com ícone e temperatura
  - público/espectadores
- bloco de briefing / prévia da corrida
  - texto narrativo contextual
  - metas da equipe, pessoal e de campeonato
- sobre o grid / adversários
  - top 5 favoritos ou pilotos em destaque
- contexto do campeonato
  - mini tabela top 5
  - diferença para líder e para o perseguidor imediato
  - barra de progresso da temporada
- rodapé de ação
  - `Simular corrida`
  - `Exportar`
  - `Voltar`

### 4. Ações finais

- `Simular corrida` leva ao fluxo já existente de resultado
- `Exportar` aparece no layout, mas sem ação real no v1
- `Voltar` retorna ao dashboard sem simular

## Estratégia de Dados

### Data atual da carreira

O frontend não deve inventar a data atual a partir de `nextRace`. O backend deve expor um resumo temporal pronto, com:

- data atual da carreira
- data da próxima corrida
- diferença em dias até o próximo evento

Isso evita divergência entre o que o calendário conhece e o que a UI mostra.

### Espectadores

Não foi localizado um campo persistido de espectadores no payload atual. Porém o projeto já tem:

- `event_interest`
- sinais de presença pública / visibilidade

No v1, o bloco de espectadores deve usar uma **estimativa derivada** desses sinais, para produzir uma linha como:

- `Expectativa de 85 mil espectadores`

Se no futuro houver um campo dedicado de público, a UI pode trocar a fonte sem mudar o layout.

### Briefing narrativo

O texto de briefing deve priorizar sinais já existentes ou facilmente deriváveis:

- posição do jogador no campeonato
- diferença para o líder
- progresso da temporada
- interesse do evento
- fase/rodada atual
- dados de standings e resultados recentes

Itens mais sofisticados, como histórico por circuito altamente específico ou cenários completos condicionais, podem entrar com fallback enxuto no v1.

## Escopo do V1

Entra no v1:

- bloco temporal fixo no header
- CTA `Avançar calendário`
- animação visual de avanço dia a dia
- abertura automática da aba de briefing ao fim da animação
- tela de briefing com identidade de box/paddock
- botão `Simular corrida`
- botão `Exportar` como placeholder
- estimativa de espectadores baseada em sinais já existentes

Fica para depois:

- previsão hora a hora
- temperatura do asfalto
- direção do vento
- exportação real
- cenários avançados de campeonato altamente condicionais

## Decisões de Estrutura

- O fluxo será incorporado à estrutura atual do dashboard, sem criar rota externa nova.
- A atual [NextRaceTab.jsx](/c:/Users/rodri/OneDrive/Área%20de%20Trabalho/Jogos/iRacerApp%20V1/src/pages/tabs/NextRaceTab.jsx) é a melhor base para virar a nova tela de briefing.
- O estado global de carreira precisará ganhar um estado intermediário para “briefing aberto” e outro para “animação de avanço em andamento”.
- O header deixará de disparar `simulateRace()` diretamente.

