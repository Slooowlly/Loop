# Driver Detail Drawer Design

## Context

The current driver detail experience opens a centered modal with a dark full-screen overlay. Functionally it loads the right data, but interaction-wise it hides too much of the standings table and makes the feature feel broken when the user wants to keep scanning names while the detail view is open.

## Chosen Direction

Replace the centered modal with a fixed right-side drawer. The drawer keeps the same backend contract and most of the existing content structure, but changes presentation:

- The panel is fixed to the right edge of the viewport.
- It slides in from right to left.
- The backdrop becomes subtle instead of fully obscuring the page.
- The standings area remains visible behind it.
- On wide screens, the standings layout shifts left with extra right padding so pilot names remain readable.

## Interaction Design

- Opening the drawer keeps the current table context in place.
- Clicking another driver swaps the drawer content in place.
- Closing still works with `ESC`, click-outside, and the close button.
- The selected row should have a stronger visual state so the relationship between table and drawer is obvious.

## Scope

In scope:

- Update `DriverDetailModal.jsx` behavior and layout into a right drawer.
- Update `StandingsTab.jsx` layout to reserve room for the drawer on larger screens.
- Add drawer-specific animations in `index.css`.
- Add focused frontend tests for the new interaction contract.

Out of scope:

- Backend payload changes.
- Team drawer or editing flows.
- Unrelated standings refactors.
