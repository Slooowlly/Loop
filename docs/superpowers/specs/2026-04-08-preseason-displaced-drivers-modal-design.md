# Preseason Displaced Drivers Modal Design

**Context**

The end-of-preseason modal in `src/components/season/PreSeasonView.jsx` currently shows displaced veteran drivers in a single flat list. Users need clearer context about where each driver came from and more room to scan the list.

**Goals**

- Group "pilotos sem vaga" by their previous category.
- Make each group clearly labeled with category name and count.
- Mention the driver's category explicitly inside the driver row.
- Increase the modal size so grouped content remains easy to read.

**Non-goals**

- No changes to preseason business rules.
- No changes to how free agents are generated.
- No restructuring of unrelated `PreSeasonView` sections.

**Chosen Approach**

Reuse the existing category helpers already present in `PreSeasonView.jsx`:

- `subcatLabel(...)` for readable category names
- `subcatColor(...)` for category accents
- `FREE_AGENT_ORDER` for stable display ordering

Create a modal-specific grouped view derived from `displacedVeterans`, then update only the modal rendering to:

- render a section per category
- show category heading plus total drivers in that category
- add a small category badge or line in each driver card
- enlarge modal width and scroll area height

**Data Flow**

- Source: `preseasonFreeAgents`
- Existing filter: `displacedVeterans = preseasonFreeAgents.filter((d) => !d.is_rookie)`
- New derived structure: `displacedVeteransByCategory`
- Render order: same category ordering already used for free agents

**Testing**

Add a focused component test in `src/components/season/PreSeasonView.test.jsx` that proves:

- the confirmation flow opens the displaced-drivers modal when preseason is complete
- drivers are grouped by category headings
- category text is shown for grouped content

Visual size changes will be verified through rendered class assertions rather than snapshot-heavy tests.

**Risks**

- `PreSeasonView.jsx` is already large, so changes should stay localized.
- Some drivers may have missing `categoria`; fallback handling should keep them under `outras`.

**Notes**

This design was approved in chat. I am not creating a git commit for the spec because the working tree already contains unrelated user changes and no commit was requested.
