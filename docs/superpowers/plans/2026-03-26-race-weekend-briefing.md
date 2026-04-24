# Race Weekend Briefing Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Adicionar ao dashboard um bloco temporal com data atual da carreira e avanço do calendário, seguido de uma aba própria de briefing pré-corrida antes da simulação.

**Architecture:** O backend passa a expor um resumo temporal mais útil para a UI, incluindo data atual da carreira e distância até a próxima corrida. O frontend usa esse resumo no header, anima a passagem do tempo localmente e abre uma tela de briefing baseada na atual `NextRaceTab`, separando claramente avanço temporal, briefing e simulação.

**Tech Stack:** React, Zustand, Tauri invoke, Rust backend, Vitest, cargo test

---

## File Map

- Modify: `src/components/layout/Header.jsx`
  - substituir o CTA direto de simulação por bloco temporal + botão `Avançar calendário`
- Modify: `src/components/layout/Header.test.jsx`
  - cobrir o novo bloco temporal e o CTA
- Modify: `src/pages/Dashboard.jsx`
  - abrir a aba/tela de briefing após o avanço do calendário
- Modify: `src/pages/tabs/NextRaceTab.jsx`
  - transformar a tela atual em briefing de box/paddock
- Modify: `src/stores/useCareerStore.js`
  - adicionar estado temporal, animação local de avanço, modo briefing e CTA de exportação placeholder
- Modify: `src/utils/formatters.js`
  - helpers curtos para data compacta e texto relativo de próxima corrida, se necessário
- Modify: `src-tauri/src/models/temporal.rs`
  - enriquecer o resumo temporal retornado ao frontend
- Modify: `src-tauri/src/commands/calendar.rs`
  - devolver o resumo temporal enriquecido
- Modify: `src-tauri/src/commands/career.rs`
  - opcionalmente incluir o resumo temporal já no payload de load da carreira, se for o caminho escolhido
- Modify: `src-tauri/src/commands/career_types.rs`
  - adicionar o DTO temporal serializável consumido pelo frontend
- Test: `src/pages/tabs/NewsTab.test.jsx`
  - não editar, apenas garantir que a integração não regrediu no dashboard geral
- Create or Modify: testes Rust perto de `src-tauri/src/commands/calendar.rs` ou `src-tauri/src/commands/career.rs`
  - validar data atual derivada, distância até próxima corrida e fallback sem corrida

## Chunk 1: Backend Temporal DTO

### Task 1: Definir o contrato temporal para o frontend

**Files:**
- Modify: `src-tauri/src/models/temporal.rs`
- Modify: `src-tauri/src/commands/career_types.rs`

- [ ] **Step 1: Escrever o teste falhando para o resumo temporal enriquecido**

Cobrir:
- data atual da carreira disponível
- próxima corrida preservada
- diferença em dias até o próximo evento

- [ ] **Step 2: Rodar o teste para garantir que falha**

Run: `cargo test temporal_summary --manifest-path src-tauri/Cargo.toml`

- [ ] **Step 3: Implementar o DTO mínimo**

Adicionar um payload serializável com campos como:
- `current_display_date`
- `next_event_display_date`
- `days_until_next_event`
- `weeks_until_next_event`

- [ ] **Step 4: Rodar o teste para garantir que passa**

Run: `cargo test temporal_summary --manifest-path src-tauri/Cargo.toml`

### Task 2: Expor a data atual da carreira a partir do calendário

**Files:**
- Modify: `src-tauri/src/commands/calendar.rs`
- Modify: `src-tauri/src/db/queries/calendar.rs`

- [ ] **Step 1: Escrever teste para derivar a data atual pela semana efetiva**

Cobrir:
- temporada sem corrida concluída
- temporada com corridas concluídas
- fallback para a data da próxima corrida quando apropriado

- [ ] **Step 2: Rodar o teste e confirmar falha**

Run: `cargo test get_temporal_summary --manifest-path src-tauri/Cargo.toml`

- [ ] **Step 3: Implementar a derivação mínima**

Usar `effective_week`, `next_player_event.week_of_year` e `display_date` já existentes para calcular a data mostrada no header.

- [ ] **Step 4: Rodar os testes**

Run: `cargo test get_temporal_summary --manifest-path src-tauri/Cargo.toml`

### Task 3: Definir a estratégia de espectadores estimados

**Files:**
- Modify: `src-tauri/src/commands/career.rs`
- Modify: `src-tauri/src/commands/career_types.rs`

- [ ] **Step 1: Escrever teste para a estimativa de espectadores**

Cobrir uma resposta com `event_interest` alto e outra sem dados suficientes.

- [ ] **Step 2: Rodar o teste e confirmar falha**

Run: `cargo test career_load_next_race --manifest-path src-tauri/Cargo.toml`

- [ ] **Step 3: Implementar a estimativa**

Gerar um valor textual ou numérico baseado em `event_interest` e sinais públicos já existentes, sem persistência nova.

- [ ] **Step 4: Rodar os testes**

Run: `cargo test career_load_next_race --manifest-path src-tauri/Cargo.toml`

## Chunk 2: Store e Fluxo de Dashboard

### Task 4: Guardar o estado temporal e o modo briefing no store

**Files:**
- Modify: `src/stores/useCareerStore.js`

- [ ] **Step 1: Escrever teste ou cenário manual reprodutível para o novo fluxo do store**

Cobrir:
- carregar carreira com resumo temporal
- iniciar avanço do calendário
- finalizar avanço abrindo o briefing
- não simular automaticamente

- [ ] **Step 2: Rodar a checagem relevante**

Run: `npm test -- src/components/layout/Header.test.jsx`

- [ ] **Step 3: Implementar o estado mínimo**

Adicionar campos como:
- `temporalSummary`
- `calendarAdvanceState`
- `showRaceBriefing`
- `exportRaceWeekend` placeholder

- [ ] **Step 4: Implementar a animação local de 5 segundos**

Atualizar a data visível dia a dia até a data da corrida e abrir briefing ao fim.

- [ ] **Step 5: Rodar a checagem relevante**

Run: `npm test -- src/components/layout/Header.test.jsx`

### Task 5: Integrar o briefing no Dashboard

**Files:**
- Modify: `src/pages/Dashboard.jsx`

- [ ] **Step 1: Adicionar teste ou cenário de renderização**

Cobrir o estado `showRaceBriefing`.

- [ ] **Step 2: Rodar a checagem**

Run: `npm test -- src/components/layout/Header.test.jsx`

- [ ] **Step 3: Implementar o branch de render**

Quando o store indicar briefing ativo, renderizar `NextRaceTab` como tela de briefing.

- [ ] **Step 4: Rodar a checagem**

Run: `npm test -- src/components/layout/Header.test.jsx`

## Chunk 3: Header e Briefing UI

### Task 6: Substituir o CTA do header pelo bloco temporal

**Files:**
- Modify: `src/components/layout/Header.jsx`
- Modify: `src/components/layout/Header.test.jsx`

- [ ] **Step 1: Escrever o teste falhando do novo topo direito**

Cobrir:
- data atual visível
- texto relativo da próxima corrida
- botão `Avançar calendário`
- ausência de `simulateRace()` direto pelo header

- [ ] **Step 2: Rodar o teste e confirmar falha**

Run: `npm test -- src/components/layout/Header.test.jsx`

- [ ] **Step 3: Implementar o layout**

Manter:
- equipe à esquerda
- tabs ao centro
- bloco temporal à direita

- [ ] **Step 4: Implementar o estado de loading da animação**

O botão deve mudar de rótulo durante o avanço do calendário e ficar desabilitado.

- [ ] **Step 5: Rodar o teste**

Run: `npm test -- src/components/layout/Header.test.jsx`

### Task 7: Transformar a NextRaceTab em briefing de box/paddock

**Files:**
- Modify: `src/pages/tabs/NextRaceTab.jsx`

- [ ] **Step 1: Escrever o teste ou cenário guiado do briefing**

Cobrir presença dos blocos:
- cabeçalho do evento
- briefing narrativo
- contexto do campeonato
- metas
- botões `Simular corrida`, `Exportar`, `Voltar`

- [ ] **Step 2: Rodar a checagem existente**

Run: `npm test -- src/components/layout/Header.test.jsx`

- [ ] **Step 3: Reestruturar a aba**

Reaproveitar dados existentes de `nextRace`, `season`, standings e store, com visual de box/paddock.

- [ ] **Step 4: Implementar o botão `Voltar`**

Fechar o briefing e retornar ao dashboard sem simulação.

- [ ] **Step 5: Manter `Exportar` como placeholder**

Botão renderizado, sem ação real, com texto de “em breve”.

- [ ] **Step 6: Rodar a checagem**

Run: `npm test -- src/components/layout/Header.test.jsx`

## Chunk 4: Narrativa, Contexto e Verificação

### Task 8: Montar o texto narrativo e os blocos de contexto

**Files:**
- Modify: `src/pages/tabs/NextRaceTab.jsx`
- Optionally Modify: `src/utils/formatters.js`

- [ ] **Step 1: Implementar helpers mínimos para narrativa**

Cobrir:
- líder / perseguidor
- diferença para o líder
- etapa restante
- meta da equipe

- [ ] **Step 2: Implementar mini tabela e progresso da temporada**

Usar standings já acessíveis pelo frontend ou uma invoke adicional enxuta.

- [ ] **Step 3: Validar visualmente o layout sem overflow**

Conferir desktop e mobile largo/estreito.

- [ ] **Step 4: Rodar o build**

Run: `npm run build`

### Task 9: Verificação final

**Files:**
- Modify: se necessário, os arquivos tocados acima

- [ ] **Step 1: Rodar os testes do backend temporal**

Run: `cargo test temporal_summary --manifest-path src-tauri/Cargo.toml`

- [ ] **Step 2: Rodar os testes da área de notícias para garantir que nada regrediu**

Run: `cargo test news_tab --manifest-path src-tauri/Cargo.toml`

- [ ] **Step 3: Rodar o build do frontend**

Run: `npm run build`

- [ ] **Step 4: Revisar manualmente o fluxo**

Confirmar:
- header mostra data e texto relativo
- botão anima o avanço
- briefing abre ao final
- simular vai para resultado
- exportar não quebra

