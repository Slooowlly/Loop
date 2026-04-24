# Compact Team Tenure Indicator Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace large tenure badges in the preseason team mapping with a compact season counter and subtle newcomer highlight.

**Architecture:** Keep the existing backend tenure payload and only refine the React presentation layer. Update the focused UI test to lock the compact `nT` format and remove dependency on the old `Novo` badge text.

**Tech Stack:** React, Vitest, Testing Library

---

### Task 1: Refine Team Slot Visuals

**Files:**
- Modify: `src/components/season/PreSeasonView.jsx`
- Test: `src/components/season/PreSeasonView.test.jsx`

- [ ] **Step 1: Update the failing test expectation**

Change the mapping test to expect compact counters like `1T` and `3T` instead of the previous badge text.

- [ ] **Step 2: Run the focused UI test**

Run: `npm test -- src/components/season/PreSeasonView.test.jsx`

- [ ] **Step 3: Replace the badge rendering with compact tenure text**

Render a compact season counter in each occupied slot and apply subtle newcomer styling when tenure is `1`.

- [ ] **Step 4: Run the focused UI test again**

Run: `npm test -- src/components/season/PreSeasonView.test.jsx`

- [ ] **Step 5: Sanity-check visual behavior**

Confirm that empty slots, veterans, and newcomers still read clearly in the component markup.
