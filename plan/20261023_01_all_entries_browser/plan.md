# Plan: Datenbank-Browser für alle Einträge (`plan/20261023_01_all_entries_browser`)

## 1. Beteiligte Dateien

| Datei | Änderung |
|---|---|
| `src/models.rs` | Neue Rows `EntryRow` (leicht: id, model, link, timestamp, cost) und `EntryDetail` (wie Row plus `summary`, `timestamped_summary_in_youtube_format`, `rs_summarizer_version`, aber **ohne** `transcript`/Embeddings). `BrowseParams` wird für `/entries?page=` wiederverwendet. |
| `src/db.rs` | `fetch_entries_page(db, page, 50)` mit expliziter leichter `SELECT`-Projektion (`ORDER BY identifier DESC`); `fetch_entry_detail(db, id)` mit Detail-Projektion ohne `transcript`/Embeddings. Unit-Tests dazu. |
| `src/templates.rs` | `EntryItem` (inkl. gekürztem `short_link`, `cost_display`), `EntriesTemplate` (`templates/entries.html`), `EntryDetailTemplate` (`templates/entry_detail.html`). |
| `templates/entries.html` | Neu: Tabelle (ID, Link, Anfrage-Datetime, Modell, Kosten), 50 Zeilen/Seite, Vor/Zurück, pro Eintrag `<details>` mit HTMX-Lazy-Load (`hx-get="/entries/{id}/detail"`, Trigger `toggle once`), Copy-Button-Skript wie auf `/browse`. |
| `templates/entry_detail.html` | Neu: Fragment mit Summary-HTML (Zeitstempel verlinkt), Modell/Kosten, Source-Link, Rating-Partial, YouTube-Copy-Button. Kein Transkript. |
| `src/routes/mod.rs` | `entries_page` (`GET /entries`) und `entry_detail` (`GET /entries/{identifier}/detail`, 404 bei unbekannter ID). Helfer (`youtube_clipboard_text`, `cost_display`, Timestamp-Linker) werden wiederverwendet. |
| `src/lib.rs` | Routen `/entries` und `/entries/{identifier}/detail` registrieren. |
| `templates/index.html` | Navigationslink auf `/entries`. |
| `tests/integration_entries.rs` | Neu: Übersicht ohne Summary/Transkript, Detail mit Summary aber ohne Transkript, ungültige `page`/ID, Regression gegen `summaries.db`. |
| `tests/integration_browser.rs` | Neuer ignorierter Fantoccini-Test: `/entries` im Headless-Browser öffnen, Metadaten prüfen, Details aufklappen, Summary sichtbar, Transkript abwesend. |
| `plan/20261023_01_all_entries_browser/` | `prompt.txt` (Quelle), `plan.md` (diese Datei), `walkthrough.md`, `plan_effort.md`. |

## 2. Ablauf

1. Modelle + DB-Funktionen + Unit-Tests schreiben, `cargo test db::` prüfen.
2. Templates + Routen + Verdrahtung, `cargo fmt`, `cargo clippy`.
3. Route-/Integrationstests schreiben und laufen lassen (`cargo test --test integration_entries`), Regression gegen `summaries.db`.
4. Browser-Test schreiben; WebDriver + Headless-Browser installieren (falls nötig); `cargo test --test integration_browser -- --ignored` für den neuen Test (plus Smoke der bestehenden Browse-Tests nur falls Zeit).
5. Gegen echte `summaries.db` per `curl` prüfen (19700+ Zeilen paginieren, Detail ohne Transkript).
6. Walkthrough + Aufwand notieren.

## 3. Commit-Strategie

Conventional Commits mit aussagekräftigem Body, z. B.:

* `feat(entries): add lightweight all-entries browser with lazy details`
* `test(entries): cover metadata-only overview and transcript-free detail`

Jeder Commit: Kurztitel (imperativ, max. ~72 Zeichen), Leerzeile, Body mit
Was/Warum, keine Historien-Umschreibung.
