# Database Flow Improvement Notes

These notes separate documentation improvements from runtime schema changes.

## What Changed In The Maps

- `database-network-diagram.mmd` is now the full ER-style network.
- `database-core-flow.mmd` shows the main career loop: drivers and teams sign contracts, seasons generate calendar events, race results feed standings, standings feed history.
- `database-modules-flow.mmd` shows side systems: market, special window, narrative, injuries, rivalries, and retirements.

## Runtime Flow Improvements To Consider

1. Make `calendar` the explicit race-event table and deprecate `races` if it remains unused.
2. Standardize mixed key names over time, especially `piloto_id` vs `pilot_id`, `temporada_id` vs `season_id`, and `equipe_id` vs `team_id`.
3. Add or enforce semantic relationships that are currently loose, especially news, history, special window logs, and DNF history.
4. Keep active state and immutable snapshots clearly separated:
   - active state: `drivers`, `teams`, `contracts`, `seasons`, `calendar`, `standings`
   - immutable history: `race_results`, `driver_season_archive`, `history_seasons`, `retired`
5. Treat market and special-window flows as pipelines:
   proposal or offer -> response -> contract -> assignment or entry -> log/history.

## Safe Migration Order

1. Add tests that document current behavior around `calendar`, `race_results`, and special contracts.
2. Add compatibility columns or views where name standardization is needed.
3. Backfill data.
4. Move queries to the new canonical names.
5. Only then remove old aliases or legacy tables.
