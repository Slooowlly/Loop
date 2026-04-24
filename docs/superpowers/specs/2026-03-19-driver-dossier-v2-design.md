# Driver Dossier V2 Design

## Context

The current driver drawer works as a detailed profile, but it is still too close to the first-pass modal conversion. It exposes identity, personality, tags, season stats, career stats, and contract details, yet it does not feel like a true career-management dossier.

The next iteration should evolve the driver drawer into a modular dossier that supports competitive reading, recent-form analysis, career storytelling, and future strategic systems such as market value, reputation, relationships, and health.

This work must avoid inventing fake gameplay data. The design should clearly separate:

- data that already exists and only needs to be reorganized;
- data that can be derived safely from existing history;
- data that needs structural support now but may remain optional or empty until a dedicated system exists.

## Goals

- Turn the driver drawer into a modular dossier instead of a flat detail sheet.
- Improve visual scanning by making country, license, personality, qualities, and defects easier to read.
- Replace points-heavy stats with racing-centric metrics that fit the fantasy of a motorsport career simulator.
- Introduce a recent-form section that makes the driver feel alive between races.
- Prepare structural space for market, relationships, reputation, and health without forcing placeholder-heavy UI.

## Non-Goals

- Build the full market simulation in this iteration.
- Implement a full social/narrative engine for relationships, reputation, or lore.
- Rewrite persistence across the entire data model before the dossier can evolve.
- Add editing flows or team-detail drill-down from this drawer.

## Chosen Direction

Keep `get_driver_detail` as the frontend entry point, but evolve its payload from a mostly flat response into a modular dossier. The drawer remains a single right-side surface, but each visual section reads from its own backend block.

The dossier should be able to render only the sections that have meaningful data. Blocks that are structurally prepared but not yet populated should stay hidden instead of rendering empty placeholders.

## Information Architecture

### Profile

Purpose: immediate identity and role recognition.

Fields:

- driver name
- prominent country flag and nationality
- age
- team name and role
- player badge
- status badge
- license badge (`Rookie`, `Amador`, `Pro`, `Super Pro`, `Elite`)
- optional contextual badges such as rookie, champion, championship leader

### Competitive Snapshot

Purpose: reveal what kind of driver this is at a glance.

Fields:

- primary and secondary personality
- motivation
- visible qualities
- visible defects
- optional derived best/worst attribute callouts

Layout direction:

- personality, qualities, and defects should share the same horizontal region on larger screens;
- thin vertical dividers should separate the groups;
- the section should read like a compact scouting report, not three disconnected lists.

### Performance

Purpose: present results in a racing-native way instead of championship-point bookkeeping.

The dossier should stop emphasizing points in the drawer.

Primary metrics:

- wins
- podiums
- top 10 finishes
- finishes outside top 10
- poles
- fastest laps
- hat-tricks
- races
- DNFs

These should exist for both season and career views.

### Current Form

Purpose: show momentum instead of raw totals.

Fields:

- last five race results
- recent average finishing position
- trend indicator (`↗`, `→`, `↘`)
- optional color-coded form status (`good`, `neutral`, `poor`)

### Career Path

Purpose: make the profile feel like an actual career, not just a stats card.

Initial scope:

- debut season
- debut team
- current category tenure
- previous team changes when reconstructable
- title markers when available
- promotion/relegation markers when available

The design intentionally starts with a basic timeline and summary blocks. It should not require a full event-history engine before shipping.

### Contract And Market

Purpose: bridge current contract facts with future management strategy.

Phase 1 fields:

- current team
- role
- salary
- term
- seasons remaining
- contract status

Prepared optional fields:

- release clause
- renewal interest
- team renewal interest
- market value
- estimated market salary
- interested teams
- transfer probability
- team compatibility
- ambition for change

### Relationships

Purpose: create narrative and internal-team tension.

Prepared optional fields:

- relationship with team principal
- engineer
- teammate
- mechanics
- fans
- media
- sponsors
- main rival
- conflict history

### Reputation

Purpose: frame the public story around the driver.

Prepared optional fields:

- popularity
- media sentiment
- public image
- archetype / narrative label

### Health

Purpose: support future risk and fatigue storytelling.

Prepared optional fields:

- general health
- current injury
- physical condition
- mental condition
- stress
- fatigue risk
- active treatment

## Data Strategy

### Data Already Available

These can be reorganized immediately:

- nationality, age, status, team, role
- personalities
- motivation
- visible tags
- season and career totals for races, wins, podiums, poles, DNFs
- contract data
- `ultimos_resultados`
- `historico_circuitos`
- active season and standings context

### Safely Derivable Data

These can be computed now without inventing new simulation systems:

- license badge from current category tier and/or experience
- top 10 and outside top 10 from race history
- recent average finish and trend from the last five results
- form label and color
- simple badges such as player, rookie, defending champion, current championship leader
- basic trajectory milestones such as debut season, team tenure, category tenure, and first known team
- simple career achievements derived from existing stats or stored race history

### Structurally Prepared But Potentially Empty

These should exist as optional dossier blocks or optional fields, but may remain absent until dedicated systems are implemented:

- market value and transfer interest
- relationship scores
- reputation metrics
- health metrics
- deep lore or biography

## Backend Design

`get_driver_detail` remains the public command, but internally it should assemble a dossier with section-level structs. The response should be explicit enough that the frontend does not need to derive complex behavior itself.

Recommended command-level structure:

- `DriverProfileBlock`
- `DriverCompetitiveBlock`
- `DriverPerformanceBlock`
- `DriverFormBlock`
- `DriverCareerPathBlock`
- `DriverContractMarketBlock`
- `DriverRelationshipsBlock`
- `DriverReputationBlock`
- `DriverHealthBlock`

Compatibility note:

The existing drawer already consumes flat fields. The implementation may either:

- replace the flat response entirely and update the frontend in one pass; or
- keep a small compatibility layer during migration and remove it once the new drawer is complete.

Preferred direction: one-pass migration to the modular shape, because the old flat contract is already stretching past its ideal lifespan.

## Frontend Design

The right drawer stays, but the internal composition changes.

### Header

- larger flag presentation
- name + license badge on the same line
- team role and team name below
- status and auxiliary badges nearby

### Competitive Row

- one horizontal band containing personality, qualities, and defects
- visual dividers between the groups
- each group collapses naturally on smaller widths

### Performance Area

- season and career should become mirrored stat clusters
- points should be removed from the primary grid
- stat cards should be grouped semantically instead of remaining a generic numeric matrix

### Form Area

- compact strip or cards for the last five finishes
- average finish and trend should remain visible without scrolling deep

### Timeline Area

- vertical timeline or stacked cards
- only render milestones that exist

### Optional Strategic Areas

- contract and market
- relationships
- reputation
- health

These sections should render only when the block contains meaningful content.

## Rollout Strategy

### Phase 1

Ship real upgrades backed by existing or safely derived data:

- profile upgrades including visible flag and license badge
- competitive snapshot layout rewrite
- performance metrics without points
- current form
- basic career path
- improved contract section

### Phase 2

Extend the command and drawer with optional structural blocks:

- market
- relationships
- reputation
- health

These may start as hidden-until-populated sections.

### Phase 3

When dedicated systems exist, populate the structural blocks with persistent gameplay data.

## Testing Strategy

Backend tests should validate:

- modular dossier assembly for contracted and free drivers
- recent form calculations
- top 10 / outside top 10 / fastest-lap / hat-trick derivations where history supports them
- license derivation rules
- omission of empty optional sections

Frontend tests should validate:

- new header composition with visible flag and license badge
- combined competitive row rendering
- removal of points from the performance grids
- rendering of recent-form indicators
- conditional rendering of optional sections
- preservation of drawer open/close behavior and portal layering
