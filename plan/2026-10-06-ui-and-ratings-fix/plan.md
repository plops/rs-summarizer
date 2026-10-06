# Implementierungsplan: UI-Optimierungen, YouTube-Copy-Button, Timestamp-Fixes & Fehlerbehandlung

Datum: 2026-10-06 · Task: `plan/20261006_01_review/prompt.txt` · Instanz: https://rocketrecap.com

## 1. Ausgangslage (Kurzbefund)

- `/browse` rendert die Zusammenfassung doppelt: `summary_html` (Markdown) plus `timestamps_html`
  (YouTube-Format als HTML) im `<footer>`. Nur der Footer bekommt derzeit Timestamp-Links.
- Der Timestamp-Regex in `src/utils/timestamp_linker.rs` verlangt zweistellige Minuten
  (`[0-5]\d`), einstellige Minuten (`1:23`, `4:05`, `9:50`) werden nicht verlinkt.
- Kosten werden mit 6 Nachkommastellen gezeigt (`${"{:.6}"|format}`), nur auf `/browse`.
  `generation_partial.html` zeigt weder Modell noch Kosten, keinen Source-Link, keine
  Rating-Komponente und keinen Copy-Button.
- Fehler enden als englische Standardmeldung (`Summary generation failed unexpectedly` bzw.
  `PublicErrorCode::message()`); `mark_error` in `src/tasks.rs` speichert den
  Code-Standardtext statt einer handlungsorientierten Meldung.
- `retry_wait` rendert den rohen ISO-Zeitstempel (`next_retry_at`).
- Die Prod-DB-Kopie `summaries.db` (19.601 Zeilen, per `.gitignore` nicht versioniert)
  enthält NULL-Werte in Spalten, die `Summary` als non-null erwartet
  (`summary` 3557×, `cost` 323×, `timestamped_summary_in_youtube_format` 500× u.a.).
  `SELECT *` + striktes `FromRow` schlägt auf solchen Zeilen fehl — jede Browse-Seite mit
  einer NULL-Zeile wäre komplett leer. Die Lesezugriffe müssen NULL-robust werden
  (Abwärtskompatibilität). Daneben enthält die DB vereinzelt negative Kosten
  (Datenfehler, z. B. `-0.249167`); Anzeige nur bei `cost > 0.0` blendet sie aus.

## 2. Beteiligte Dateien und geplante Änderungen

| Datei | Änderung |
|---|---|
| `src/utils/timestamp_linker.rs` | Regex auf `\b(?:\d{1,2}:)?[0-5]?\d:[0-5]\d\b` erweitern (einstellige Minuten); Kommentar + Unit-Tests für `1:23`, `4:05`, `9:50`, `1:02:03`; bestehende Guards (kein `16:09px`, keine Ratios) und Tests bleiben. |
| `src/utils/cost_format.rs` (neu) + `src/utils/mod.rs` | `format_cost(cost: f64) -> String`: `<= 0.0` → `$0.00`; `>= 0.01` → 2 Stellen; darunter → 3 Stellen. Unit-Tests für alle drei Zweige. |
| `src/generation.rs` | `format_retry_display(next_retry_at) -> String` (plus testbaren Kern mit `now`-Parameter): Zukunft → `Wiederholung in ca. N Minute(n) (geplant um HH:MM Uhr)`; Vergangenheit/ungültig/leer → `Wiederholung geplant …` ohne Roh-ISO. Zeit in UTC (Containerzeit, kein neuer Crate für Zeitzonen). Unit-Tests. |
| `src/tasks.rs` | `user_facing_error_message(&ProcessError) -> String` (pub, testbar) mit den vier deutschen Meldungen (keine Untertitel / YouTube-Blockade / Modell ausgelastet / Transkript zu kurz), Fallback auf bisheriges `format_process_error`. `mark_error` nimmt `&ProcessError`: Code aus Rohfehler bestimmen (Logik unverändert), deutsche Meldung speichern. `process_summary` anpassen. Unit-Tests. |
| `src/db.rs` | `fetch_summary` + `fetch_browse_page`: explizite Spaltenliste mit `COALESCE` für alle historisch NULL-fähigen Spalten (Strings → `''`, Bool/Int → `0`, `cost` → `0.0`). Keine Schemaänderung. Test mit Prod-ähnlichem Legacy-Schema (nullable Spalten + NULL-Werte). |
| `src/templates.rs` | `BrowseSummaryItem`: `timestamps_html` entfernen; `cost_display: String` (leer bei `cost <= 0.0`) und `youtube_text: String` (roh, für `data-clipboard`) hinzufügen. `GenerationPartialTemplate`: `model`, `cost_display`, `original_source_link`, `youtube_text`, `rating_stats`, `retry_display` hinzufügen; `timestamps` entfernen. |
| `templates/browse.html` | `<footer>`-Duplikat entfernen; Kosten via `cost_display`; Copy-Button `📋 Für YouTube kopieren` mit `data-clipboard` + Feedback `Kopiert! ✓`; Delegations-JS (Clipboard-API mit Fallback). Sortierung (`identifier DESC`) und Rating-Include unverändert. |
| `templates/generation_partial.html` | Footer entfernen; `retry_wait` zeigt `retry_display`; `succeeded` zeigt Metadaten (Modell, Kosten), Source-Link, Copy-Button und `rating_partial.html`-Include. Poll-Logik (`hx-trigger`) unverändert. |
| `templates/index.html` | Delegations-JS für `.yt-copy-btn` (einmalig, greift auch für per HTMX nachgeladene Partials). |
| `templates/rating_partial.html` | Unverändert (wird in `generation_partial` direkt inkludiert, Felder heißen passend `identifier`/`rating_stats`). |
| `src/routes/mod.rs` | `browse_summaries`: `summary_html` mit `replace_timestamps_in_html` verlinken, `cost_display`/`youtube_text` befüllen. `render_generation_partial(app, id, client_ip)` mit Rating-Stats, Metadaten, `retry_display`. `get_generation`/`retry_generation` nehmen Client-IP entgegen (`ConnectInfo` + `HeaderMap`, wie `browse`). `process_transcript`-Mehrfachreihenfolge unverändert. Tests in `mod tests` aktualisieren/erweitern. |
| `src/services/summary.rs` | Keine Änderung (Hetzner-Key-Fallback bleibt wie gefordert). |
| `tests/integration_ui.rs` (neu) | `GET /browse`: kein Artikel-`<footer>`, Copy-Button vorhanden, Kosten formatiert. `POST /generations/{id}` succeeded: Source-Link, Rating, Copy-Button. `retry_wait`: freundliche Zeit, kein ISO-Rohwert. Regression: Prod-ähnliche Legacy-DB mit NULLs rendert fehlerfrei; echte `summaries.db`-Kopie wird zusätzlich genutzt, wenn vorhanden (sonst synthetische Fixture). |
| `plan/2026-10-06-ui-and-ratings-fix/plan.md` | Diese Datei. |
| `plan/2026-10-06-ui-and-ratings-fix/deps.md` | Dependency-Recherche (keine neuen Crates). |
| `plan/2026-10-06-ui-and-ratings-fix/plan_effort.md` | Token-/Aufwandsdokumentation (am Ende). |
| `plan/20261006_01_review/walkthrough.md` | Deutscher Walkthrough mit Mermaid-Diagrammen (am Ende). |

## 3. Ablauf (Implementierung und Verifikation)

1. Baseline: `cargo test --lib` (Kompilations- + Teststand sichern).
2. `timestamp_linker.rs`: Regex + Tests → `cargo test timestamp`.
3. `cost_format.rs`: Helper + Tests → `cargo test cost`.
4. `generation.rs`: Retry-Format + Tests → `cargo test retry`.
5. `tasks.rs`: Fehlermeldungen + `mark_error`-Umbau + Tests → `cargo test tasks`.
6. `db.rs`: NULL-robuste SELECTs + Legacy-Schema-Test → `cargo test db`.
7. `templates.rs` + Templates + `routes/mod.rs` → `cargo test routes`, neuer `tests/integration_ui.rs`.
8. Regression: `tests/integration_ui.rs` gegen echte `summaries.db`-Kopie ausführen
   (read-only, keine Migration); zusätzlich relevante Bestands-Suites
   (`integration_ratings`, `integration_pipeline` soweit ohne Netz lauffähig).
9. `cargo fmt` + `cargo clippy --all-targets --all-features` (null Warnings).
10. Docs: `plan_effort.md`, `walkthrough.md` (Deutsch, Mermaid, Begriffserklärungen,
    Architekturentscheidungen/Abweichungen, Learnings, Dockerfile-Pakete).
11. Review des Diffs, dann Commits nach Strategie (Abschnitt 4).

Verifikationsziele pro Schritt: jeweils fokussierte Tests grün; am Ende Gesamtlauf der
betroffenen Suiten + Clippy/Fmt sauber.

## 4. Commit-Strategie (Conventional Commits)

Kleine, einzeln nachvollziehbare Commits mit aussagekräftigem Body (was + warum):

- `fix(timestamps): link single-digit minute timestamps`
- `feat(ui): format costs with 2 or 3 decimals`
- `feat(ui): show friendly retry time instead of raw ISO timestamp`
- `fix(errors): add actionable German user-facing error messages`
- `fix(db): tolerate NULLs in legacy summary rows`
- `feat(ui): deduplicate browse rendering and add YouTube copy button`
- `feat(ui): show source, rating and metadata in generation partial`
- `test(ui): cover browse, generation partial and legacy DB regression`
- `docs(plan): add implementation plan, walkthrough and effort notes`

Reihenfolge = Implementierungsreihenfolge; Docs-Commit zuletzt. Kein Commit enthält
`summaries.db` (gitignoriert) oder den Hetzner-Key (bleibt unverändert im Code).
