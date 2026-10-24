# Plan: Titel für YouTube-Videos und HN-Posts (`plan/20261024_01_titles`)

## 1. Beteiligte Dateien

| Datei | Änderung |
|---|---|
| `migrations/010_add_title.sql` | Neu: `title`-Spalte (`TEXT NOT NULL DEFAULT ''`) + Budget-Tabelle `title_fetch_budget(day, source, count)`. |
| `src/models.rs` | `title: String` in `Summary`, `EntryRow`, `EntryDetail`. |
| `src/db.rs` | `SUMMARY_COLUMNS` + `title`; `update_title`, `update_title_for_url`, `fetch_untitled_urls` (DISTINCT, ohne leere Links), `title_budget_remaining`/`consume`. Unit-Tests. |
| `src/services/transcript.rs` | `fetch_title(url)` via `yt-dlp --no-playlist --skip-download --print title` mit denselben Cookie-/Extractor-Args. |
| `src/services/hacker_news.rs` | Keine Logikänderung nötig (Titel steckt in `HnFetchResult`); ggf. Helper `fetch_story_title` für den Backfill ohne Kommentar-Wust. |
| `src/tasks.rs` | `fetch_hn_content` liefert Titel mit (Tuple/Struct); `fetch_youtube_content` speichert Titel best-effort; neuer Backfill-Loop `title_backfill_loop` mit Tagesbudgets (YT 100/Tag, HN 1000/Tag, per Env überstimmbar). |
| `src/main.rs` | Backfill-Loop als Hintergrundtask starten. |
| `src/templates.rs` | `title`-Felder in `BrowseSummaryItem`, `EntryItem`, `EntryDetailTemplate`. |
| `templates/browse.html`, `templates/entries.html`, `templates/entry_detail.html` | Titel anzeigen, Fallback auf (gekürzten) Link. |
| `src/routes/mod.rs` | Titel an Templates übergeben. |
| `tests/integration_entries.rs`, `tests/integration_ui.rs` | Titel-Rendering + Fallback, HN-Titel-Pfad, Regression gegen `summaries.db`. |
| `tests/integration_browser.rs` | Ignorierter Fantoccini-Test: Titel in `/entries` + `/browse` + Detail sichtbar. |

## 2. Ablauf

1. Migration + Modelle + DB-Helfer + Unit-Tests (`cargo test db::`).
2. `fetch_title` (yt-dlp), HN-Titel-Weitergabe, Titel-Speichern im Pipeline-Pfad.
3. Backfill-Loop + Verdrahtung in `main.rs`.
4. Templates + Routen, `cargo fmt`, `cargo clippy`.
5. Route-/Integrationstests + Regression gegen `summaries.db`.
6. Browser-Test mit Headless-Chrome (Binaries unter `/opt/chrome-test` vorhanden).
7. Walkthrough + Aufwand.

## 3. Commit-Strategie

Conventional Commits mit Body, z. B.:

* `feat(titles): store and display YouTube/HN titles with rate-limited backfill`
* `test(titles): cover title rendering and transcript-free detail`

Keine Historien-Umschreibung.
