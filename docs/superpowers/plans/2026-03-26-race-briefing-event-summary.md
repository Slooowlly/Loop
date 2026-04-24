# Race Briefing Event Summary Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Atualizar o `Resumo do evento` do briefing pre-corrida para o layout compacto aprovado, com publico ranqueado, cobertura/editorial e historico resumido.

**Architecture:** A mudanca fica concentrada em `NextRaceTab.jsx`, reaproveitando o contexto ja montado no briefing e adicionando alguns helpers puros para derivar ranking de publico, cobertura ao vivo e historico resumido. Os testes de UI continuam em `NextRaceTab.test.jsx`.

**Tech Stack:** React, Vitest, Testing Library, Tauri frontend store

---

## Chunk 1: Teste e implementacao do resumo refinado

### Task 1: Cobrir o novo resumo do evento

**Files:**
- Modify: `src/pages/tabs/NextRaceTab.test.jsx`
- Modify: `src/pages/tabs/NextRaceTab.jsx`
- Test: `src/pages/tabs/NextRaceTab.test.jsx`

- [ ] **Step 1: Write the failing test**

Adicionar assertions para o novo card:
- `Publico`
- ranking da etapa na temporada
- `Cobertura` com `Ao vivo` em etapa importante
- `Historico`
- subtitulo do melhor resultado
- `Horario local` com enfase no periodo do dia

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test -- src/pages/tabs/NextRaceTab.test.jsx`
Expected: FAIL nas novas expectativas do bloco `Resumo do evento`

- [ ] **Step 3: Write minimal implementation**

Atualizar o card em `NextRaceTab.jsx` para:
- trocar as linhas simples pelo layout compacto aprovado
- derivar ranking de publico via calendario da temporada atual
- derivar `Cobertura` vs `Expectativa`
- derivar um `Historico` curto com o melhor dado real disponivel

- [ ] **Step 4: Run test to verify it passes**

Run: `npm test -- src/pages/tabs/NextRaceTab.test.jsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/pages/tabs/NextRaceTab.jsx src/pages/tabs/NextRaceTab.test.jsx docs/superpowers/specs/2026-03-26-race-briefing-event-summary-design.md docs/superpowers/plans/2026-03-26-race-briefing-event-summary.md
git commit -m "feat: refine race briefing event summary"
```

### Task 2: Verificacao final

**Files:**
- Modify: `src/pages/tabs/NextRaceTab.jsx`
- Test: `src/pages/tabs/NextRaceTab.test.jsx`

- [ ] **Step 1: Run targeted verification**

Run: `npm test -- src/pages/tabs/NextRaceTab.test.jsx`
Expected: PASS

- [ ] **Step 2: Run build verification**

Run: `npm run build`
Expected: PASS

- [ ] **Step 3: Check polish**

Confirmar no componente:
- sem redundancia com `Condicoes`
- sem texto enganoso quando faltar historico por pista
- layout responsivo preservado
