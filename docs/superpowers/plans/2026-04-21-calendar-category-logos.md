# Calendar Category Logos Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Substituir `R1/R2/R3` na celula principal do calendario por uma logo da categoria, mantendo a rodada apenas no tooltip.

**Architecture:** A mudanca fica concentrada em `src/pages/tabs/CalendarTab.jsx`, onde a celula principal de corrida ja combina imagem da pista, badges e tooltip. As logos passam a ser servidas como assets publicos em `/categorias/...`, com fallback seguro quando algum arquivo nao estiver mapeado. Os testes de interface em `CalendarTab.test.jsx` cobrem a nova apresentacao da celula sem alterar o comportamento do tooltip nem dos indicadores de outras categorias.

**Tech Stack:** React, Vite, Vitest, Testing Library, assets estaticos em `public/`

---

## File Structure

- Modify: `src/pages/tabs/CalendarTab.jsx`
  Responsibility: declarar o mapa de logos por categoria e atualizar a renderizacao da `DayCell`.
- Modify: `src/pages/tabs/CalendarTab.test.jsx`
  Responsibility: validar a nova celula principal com logo, o desaparecimento de `R{rodada}` na celula e a preservacao da rodada no tooltip.
- Create: `public/categorias/MX5 ROOKIE.png`
  Responsibility: asset estatico da categoria `mazda_rookie`.
- Create: `public/categorias/GR ROOKIE.png`
  Responsibility: asset estatico da categoria `toyota_rookie`.
- Create: `public/categorias/MX5 CUP.png`
  Responsibility: asset estatico da categoria `mazda_amador`.
- Create: `public/categorias/GR CUP.png`
  Responsibility: asset estatico da categoria `toyota_amador`.
- Create: `public/categorias/PRODUCTION.png`
  Responsibility: asset estatico da categoria `production_challenger`.
- Create: `public/categorias/GT4.png`
  Responsibility: asset estatico da categoria `gt4`.
- Create: `public/categorias/GT3.png`
  Responsibility: asset estatico da categoria `gt3`.
- Create: `public/categorias/ENDURANCE.png`
  Responsibility: asset estatico da categoria `endurance`.

## Chunk 1: Asset mapping and regression tests

### Task 1: Add the failing UI test for the new day cell

**Files:**
- Modify: `src/pages/tabs/CalendarTab.test.jsx`
- Verify: `src/pages/tabs/CalendarTab.test.jsx`

- [ ] **Step 1: Write the failing test**

```jsx
it("renders the main race day with a category logo instead of the round label", async () => {
  render(<CalendarTab activeTab="calendar" />);

  const raceDay = await screen.findByTestId("calendar-day-2026-03-12");
  expect(within(raceDay).queryByText("R1")).not.toBeInTheDocument();

  const logo = within(raceDay).getByAltText("Mazda MX-5 Rookie Cup");
  expect(logo).toHaveAttribute("src", "/categorias/MX5%20ROOKIE.png");
});
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run: `npx vitest run src/pages/tabs/CalendarTab.test.jsx -t "renders the main race day with a category logo instead of the round label"`
Expected: FAIL because the cell still renders `R1` and no category logo exists.

- [ ] **Step 3: Add public category assets**

Copiar os arquivos aprovados de `image/categorias/` para `public/categorias/`, preservando os nomes de arquivo.

- [ ] **Step 4: Add category logo mapping and minimal DayCell rendering**

```jsx
const CATEGORY_LOGOS = {
  mazda_rookie: "/categorias/MX5%20ROOKIE.png",
  toyota_rookie: "/categorias/GR%20ROOKIE.png",
  mazda_amador: "/categorias/MX5%20CUP.png",
  toyota_amador: "/categorias/GR%20CUP.png",
  bmw_m2: "/categorias/PRODUCTION.png",
  production_challenger: "/categorias/PRODUCTION.png",
  gt4: "/categorias/GT4.png",
  gt3: "/categorias/GT3.png",
  endurance: "/categorias/ENDURANCE.png",
};
```

Renderizar a logo central apenas quando `race` existir, sem alterar tooltip nem dots secundarios.

- [ ] **Step 5: Run the focused test to verify it passes**

Run: `npx vitest run src/pages/tabs/CalendarTab.test.jsx -t "renders the main race day with a category logo instead of the round label"`
Expected: PASS

- [ ] **Step 6: Commit the chunk**

```bash
git add public/categorias src/pages/tabs/CalendarTab.jsx src/pages/tabs/CalendarTab.test.jsx
git commit -m "feat: show category logos in calendar race cells"
```

## Chunk 2: Visual polish and regression coverage

### Task 2: Move the day number into a badge and preserve tooltip behavior

**Files:**
- Modify: `src/pages/tabs/CalendarTab.jsx`
- Modify: `src/pages/tabs/CalendarTab.test.jsx`
- Verify: `src/pages/tabs/CalendarTab.test.jsx`

- [ ] **Step 1: Write the failing regression test for tooltip preservation**

```jsx
it("keeps the round details in the tooltip after replacing the cell label", async () => {
  render(<CalendarTab activeTab="calendar" />);

  const raceDay = await screen.findByTestId("calendar-day-2026-03-12");
  fireEvent.mouseEnter(raceDay);

  const tooltip = await screen.findByTestId("calendar-tooltip");
  expect(within(tooltip).getByText("R1")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the focused test to verify it fails if tooltip behavior regresses**

Run: `npx vitest run src/pages/tabs/CalendarTab.test.jsx -t "keeps the round details in the tooltip after replacing the cell label"`
Expected: PASS before refactor or FAIL only if the tooltip was accidentally changed. If it already passes, keep it as a guard and proceed.

- [ ] **Step 3: Apply the visual layout update**

Implementar na `DayCell`:

- remover o texto `R{race.rodada}` da celula principal;
- mover o numero do dia para um badge no canto superior direito;
- renderizar a logo como elemento central com `object-contain`;
- manter `Hoje`, `Esp`, status e dots secundarios em suas posicoes compatveis.

- [ ] **Step 4: Run the focused calendar test file**

Run: `npx vitest run src/pages/tabs/CalendarTab.test.jsx`
Expected: PASS with all calendar tests green.

- [ ] **Step 5: Review the affected diff**

Run: `git diff -- src/pages/tabs/CalendarTab.jsx src/pages/tabs/CalendarTab.test.jsx public/categorias`
Expected: only the planned asset and calendar-cell changes.

- [ ] **Step 6: Commit the polish**

```bash
git add public/categorias src/pages/tabs/CalendarTab.jsx src/pages/tabs/CalendarTab.test.jsx
git commit -m "test: cover calendar category logo presentation"
```

## Chunk 3: Final verification

### Task 3: Verify the finished change before handoff

**Files:**
- Verify: `src/pages/tabs/CalendarTab.jsx`
- Verify: `src/pages/tabs/CalendarTab.test.jsx`
- Verify: `public/categorias/*`

- [ ] **Step 1: Run final focused verification**

Run: `npx vitest run src/pages/tabs/CalendarTab.test.jsx`
Expected: PASS

- [ ] **Step 2: Re-read the spec and verify requirements against the implementation**

Checklist:

- corrida principal mostra logo da categoria;
- celula principal nao mostra mais `R{rodada}`;
- tooltip continua mostrando `R{rodada}`;
- outras categorias continuam com dots e tooltip;
- badge do dia continua legivel.

- [ ] **Step 3: Summarize any residual risk**

Registrar se alguma categoria ainda depende de fallback visual ou se algum logo precisa ajuste de proporcao.
