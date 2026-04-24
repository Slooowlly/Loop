# Category Race Weekday Policy Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make each category generate races on an approved weekday pattern instead of defaulting every `display_date` to Saturday.

**Architecture:** Keep the monthly phase windows exactly as they are, but split calendar generation into two rules: `season window` defines which part of the year the category may occupy, and `weekday policy` defines which weekday the category may use inside that window. Centralize the policy in `src-tauri/src/calendar/mod.rs`, then update the calendar UI fallback so the convocation preview follows the same Sunday-first special logic.

**Tech Stack:** Rust, chrono, rand, rusqlite, React, Vitest

---

## File Map

- Modify: `src-tauri/src/calendar/mod.rs`
  - Centralize weekday policy per category.
  - Resolve one stable weekday per category per season.
  - Generate `display_date` from the resolved weekday instead of hardcoding Saturday.
  - Add special Sunday-first overflow logic that can be unit-tested in isolation.
- Modify: `src/pages/tabs/CalendarTab.jsx`
  - Replace the hardcoded special fallback anchor that still assumes Saturday.
  - Keep convocation highlighting aligned with the first special race day when the real special calendar has not loaded yet.
- Test: `src/pages/tabs/CalendarTab.test.jsx`
  - Lock the convocation-week UI against the new Sunday-first special default.

## Chunk 1: Backend Weekday Policy

### Task 1: Add weekday-policy helpers and lock the approved category ranges

**Files:**
- Modify: `src-tauri/src/calendar/mod.rs`
- Test: `src-tauri/src/calendar/mod.rs`

- [ ] **Step 1: Write the failing tests**

Add focused tests with a shared prefix like `test_weekday_policy_...` for:

```rust
#[test]
fn test_weekday_policy_gt4_is_always_saturday() {
    let mut rng = StdRng::seed_from_u64(42);
    let calendar = generate_calendar_for_category_with_year("S001", 2028, "gt4", &mut rng)
        .expect("gt4 calendar");

    for entry in &calendar {
        let date = NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d").expect("date");
        assert_eq!(date.weekday(), Weekday::Sat);
    }
}

#[test]
fn test_weekday_policy_rookie_is_stable_within_monday_tuesday() {
    let mut rng = StdRng::seed_from_u64(7);
    let calendar = generate_calendar_for_category_with_year("S001", 2028, "mazda_rookie", &mut rng)
        .expect("rookie calendar");

    let weekdays: HashSet<Weekday> = calendar
        .iter()
        .map(|entry| NaiveDate::parse_from_str(&entry.display_date, "%Y-%m-%d").unwrap().weekday())
        .collect();

    assert_eq!(weekdays.len(), 1);
    assert!(matches!(weekdays.iter().next(), Some(Weekday::Mon | Weekday::Tue)));
}
```

Cover all approved ranges:
- `mazda_rookie`, `toyota_rookie` -> `Mon` or `Tue`
- `mazda_amador`, `toyota_amador`, `bmw_m2` -> `Wed` or `Thu`
- `gt4` -> `Sat`
- `gt3` -> `Sun`

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test weekday_policy`
Expected: FAIL because `display_date` still comes from `week_to_display_date(..., Weekday::Sat)` for every category.

- [ ] **Step 3: Write minimal implementation**

Add small helpers in `calendar/mod.rs`:

```rust
enum CategoryWeekdayPolicy {
    Fixed(Weekday),
    StableChoice(&'static [Weekday]),
    PreferredWithOverflow {
        preferred: Weekday,
        overflow: &'static [Weekday],
    },
}

fn weekday_policy_for_category(category_id: &str) -> CategoryWeekdayPolicy { ... }
fn resolve_stable_weekday(category_id: &str, rng: &mut impl Rng) -> Weekday { ... }
fn display_date_for_weekday(year: i32, week: i32, weekday: Weekday) -> String { ... }
```

Important implementation constraints:
- Resolve the weekday once per category generation, outside the per-round `map`.
- Preserve `week_of_year` as the ordering source of truth.
- Keep `display_date` as the only field that changes weekday semantics.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test weekday_policy`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/calendar/mod.rs
git commit -m "feat: add stable weekday policy for race calendars"
```

### Task 2: Apply the stable weekday rule to regular calendar generation

**Files:**
- Modify: `src-tauri/src/calendar/mod.rs`
- Test: `src-tauri/src/calendar/mod.rs`

- [ ] **Step 1: Write the failing tests**

Add integration-style tests that validate the complete regular output still respects both month window and stable weekday:

```rust
#[test]
fn test_weekday_policy_amador_and_bmw_stay_in_wed_thu() { ... }

#[test]
fn test_weekday_policy_gt3_is_always_sunday_inside_regular_window() { ... }
```

Also keep the existing month-window assertions and expand them to verify:
- the category remains inside `fevereiro-agosto`
- the chosen weekday does not drift between rounds

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test weekday_policy_regular`
Expected: FAIL until `generate_calendar_for_category_with_constraints` passes the resolved weekday through to `build_calendar_entry`.

- [ ] **Step 3: Write minimal implementation**

Refactor the generation path so it resolves the weekday once and threads it through:

```rust
let resolved_weekday = resolve_calendar_weekday(categoria, season_phase, week_start, week_end, total, rng)?;

let entries = ordered_tracks
    .into_iter()
    .enumerate()
    .map(|(index, (track, thematic_slot))| {
        build_calendar_entry(
            ...,
            resolved_weekday,
            ...
        )
    })
    .collect();
```

Update `build_calendar_entry` to call:

```rust
display_date: display_date_for_weekday(season_year, week_of_year, resolved_weekday),
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test weekday_policy_regular`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/calendar/mod.rs
git commit -m "feat: generate regular calendars on category weekdays"
```

### Task 3: Add Sunday-first overflow handling for special calendars

**Files:**
- Modify: `src-tauri/src/calendar/mod.rs`
- Test: `src-tauri/src/calendar/mod.rs`

- [ ] **Step 1: Write the failing tests**

Add unit tests around a helper that can be exercised with a compressed candidate set, so overflow is testable even though the real `setembro-dezembro` window usually has enough Sundays:

```rust
#[test]
fn test_special_weekday_policy_prefers_sunday_when_enough_slots_exist() { ... }

#[test]
fn test_special_weekday_policy_uses_overflow_only_after_sundays_are_exhausted() { ... }
```

The helper should accept a candidate-date list or candidate-week list in tests, for example:

```rust
let assigned = assign_special_display_dates_from_candidates(
    &candidates,
    total_rounds,
    Weekday::Sun,
    &[Weekday::Sat, Weekday::Fri],
);
```

Expected behavior:
- use every available `Sunday` first
- only then consume overflow weekdays
- never leave the allowed date pool

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test special_weekday_policy`
Expected: FAIL because special generation still uses the same generic Saturday-based display-date builder.

- [ ] **Step 3: Write minimal implementation**

Introduce a small, testable helper in `calendar/mod.rs`:

```rust
fn assign_special_display_dates_from_candidates(
    candidates: &[NaiveDate],
    total_rounds: usize,
    preferred: Weekday,
    overflow: &[Weekday],
) -> Result<Vec<NaiveDate>, String> { ... }
```

Then integrate it so `production_challenger` and `endurance`:
- prefer `Sunday`
- use overflow weekdays only when necessary
- still preserve `week_of_year` ordering and `setembro-dezembro` month boundaries

Recommended overflow order:
- `Saturday`
- `Friday`

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test special_weekday_policy`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/calendar/mod.rs
git commit -m "feat: prefer sunday dates for special calendars"
```

## Chunk 2: Calendar UI Alignment and Verification

### Task 4: Remove the remaining Saturday assumption from the calendar tab fallback

**Files:**
- Modify: `src/pages/tabs/CalendarTab.jsx`
- Test: `src/pages/tabs/CalendarTab.test.jsx`

- [ ] **Step 1: Write the failing test**

Add a focused UI test that proves the convocation fallback follows the Sunday-first special policy when the accepted offer exists but only the fallback anchor is available:

```jsx
it("anchors the convocation fallback to the second sunday of september", async () => {
  mockState.acceptedSpecialOffer = {
    id: "offer-1",
    team_name: "Team Orion",
    special_category: "endurance",
    class_name: "gt4",
  };

  render(<CalendarTab activeTab="calendar" />);

  expect(await screen.findByTestId("calendar-day-2026-09-13"))
    .toHaveAttribute("data-special-race-day", "true");
});
```

If the current fallback still assumes Saturday, the convocation window will drift one day earlier than the approved special policy.

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/pages/tabs/CalendarTab.test.jsx`
Expected: FAIL if `getFallbackFirstSpecialRaceDate` still returns the second Saturday of September.

- [ ] **Step 3: Write minimal implementation**

Update `CalendarTab.jsx` so the fallback anchor uses the same special default:

```jsx
function getFallbackFirstSpecialRaceDate(year) {
  return nthWeekdayOfMonthUtc(year, 8, 0, 2);
}
```

Keep the rest of the convocation-window math unchanged unless the tests prove another Saturday assumption is still embedded.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/pages/tabs/CalendarTab.test.jsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/pages/tabs/CalendarTab.jsx src/pages/tabs/CalendarTab.test.jsx
git commit -m "fix: align calendar fallback with special sunday policy"
```

### Task 5: Run focused verification and review residual risks

**Files:**
- Verify only: `src-tauri/src/calendar/mod.rs`
- Verify only: `src/pages/tabs/CalendarTab.jsx`
- Verify only: `src/pages/tabs/CalendarTab.test.jsx`

- [ ] **Step 1: Run focused backend tests**

Run: `cargo test weekday_policy`
Expected: PASS

- [ ] **Step 2: Run the broader calendar backend suite**

Run: `cargo test calendar::tests`
Expected: PASS, including month-window and special-calendar coverage

- [ ] **Step 3: Run focused frontend tests**

Run: `npx vitest run src/pages/tabs/CalendarTab.test.jsx`
Expected: PASS

- [ ] **Step 4: Review residual risks**

Confirm manually that:
- `display_date` consumers still behave correctly for non-Saturday races
- convocation-week highlighting still lines up with the first special race
- no helper outside `calendar/mod.rs` is manufacturing a fake Saturday date

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/calendar/mod.rs src/pages/tabs/CalendarTab.jsx src/pages/tabs/CalendarTab.test.jsx
git commit -m "feat: generate category calendars on stable race weekdays"
```
