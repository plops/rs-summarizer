# Implementierungsplan: sichtbare und persistierte rs-summarizer-Version

**Projekt:** `plops/rs-summarizer`  
**Ziel:** Die aus `Cargo.toml` gebaute Anwendungsversion ist auf allen vollständigen HTML-Seiten sichtbar und wird unveränderlich mit jeder neu angelegten Zusammenfassung gespeichert.

## Befund und Entscheidungen

Die Paketversion ist aktuell `1.7.6` in `Cargo.toml`; `cargo metadata` bestätigt dieselbe Paketversion. SQLite wird ausschließlich über `db::init_db` und eingebettete SQLx-Migrationen initialisiert. Neue Zusammenfassungen entstehen im Pfad `routes::process_transcript` → `db::insert_new_summary`; `Summary` wird mit `SELECT *` geladen. Askama rendert die zwei vollständigen Seiten `index.html` und `browse.html`.

1. Die Laufzeitversion stammt ausschließlich aus `env!("CARGO_PKG_VERSION")`. Es gibt keine zweite, manuell gepflegte Versionsquelle und kein Laufzeitlesen von `Cargo.toml`.
2. Die neue Spalte heißt `rs_summarizer_version`, damit sie nicht mit Modell-, Schema- oder API-Version verwechselt wird.
3. Migration `009_add_rs_summarizer_version.sql` ist additiv mit `TEXT NOT NULL DEFAULT ''`. Leer bedeutet bei Altbestand ausdrücklich „unbekannt“; die Migration darf historische Erzeugerversionen nicht erfinden.
4. Bei der Anlage wird die Version des laufenden Binaries gespeichert. Retry und spätere Zustandsupdates überschreiben sie nicht.
5. Start- und Browse-Seite zeigen einen semantischen Footer `rs-summarizer v{{ app_version }}`. Browse zeigt zusätzlich die gespeicherte Erzeugerversion der einzelnen Zusammenfassung.
6. Der reduzierte `export-db`-Export behält die Spalte; sonst ginge die gewünschte Provenienz beim offiziellen Export verloren.

## Zusätzliche sinnvolle Entscheidungen

Die Kernanforderungen sind ausreichend. Vor späterer Erweiterung sollte entschieden werden, ob die Version auch als Health-/JSON-Metadatum, mit Git-Commit/Buildzeit oder in geschätzten historischen Daten verfügbar sein soll. Empfehlung: keine historischen Werte schätzen und deduplizierte vorhandene Zusammenfassungen nicht auf eine neuere Version umschreiben.

## Datei-Kontext

| Datei | Zweck |
| --- | --- |
| `Cargo.toml`, `Cargo.lock` | Autoritative Paketversion; nur bei absichtlichem Release-Bump ändern. |
| `src/main.rs`, `src/state.rs` | Compile-Time-Version zentral definieren und als clonbaren `AppState.app_version: &'static str` bereitstellen. |
| `src/models.rs` | `Summary` wird per `sqlx::FromRow` aus allen Spalten abgebildet. |
| `src/db.rs` | Migrationen, Insert-Signatur sowie DB-/Legacy-Migrations-Tests. |
| `migrations/001_initial.sql`, `007_add_generation_lifecycle.sql`, `008_quarantine_legacy_queued_generations.sql` | Schema- und additive-Migrationskonventionen; niemals rückwirkend editieren. |
| `src/routes/mod.rs` | Übergibt Version beim Insert und baut die Index-/Browse-View-Modelle. |
| `src/templates.rs`, `templates/index.html`, `templates/browse.html` | Typisierte Askama-Kontexte und zugängliche Anzeige. |
| `src/commands/export_db.rs` | Reduziertes Export-Schema, SELECT-/INSERT-Spalten und Tests. |
| `tests/integration_ratings.rs`, `tests/integration_pipeline.rs`, `tests/integration_browser.rs` | Externe `AppState`-Builder und Insert-Aufrufer. |
| `plan/20260904_03_version/deps.md`, `task.md` | Recherche bzw. serialisierte Ausführung. |

DeepWiki für `plops/rs-summarizer` bestätigt diesen Pfad und nennt die vorhandene Glossary-/Thinking-Level-Feldführung als Präzedenzfall. Bei Abweichungen ist der lokale Quellcode maßgeblich.

## Umsetzung

### 1. Versionsquelle und Zustand

Definiere eine zentrale öffentliche Konstante, etwa `pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");`. Ergänze `AppState` um `app_version: &'static str`; initialisiere sie in `main.rs` und allen Test-Buildern. Kein Environment-Override: DB-Provenienz muss dem gebauten Artefakt entsprechen.

### 2. Additive Migration und Persistenz

Lege ausschließlich `migrations/009_add_rs_summarizer_version.sql` an:

```sql
ALTER TABLE summaries
    ADD COLUMN rs_summarizer_version TEXT NOT NULL DEFAULT '';
```

Erweitere `Summary`. Ergänze `insert_new_summary` um einen expliziten Versionsparameter und die INSERT-Spalte/bind; `process_transcript` übergibt `app.app_version`. Direkte Test-Aufrufer erhalten einen festen Wert. Die bestehende `SELECT *`-Ladelogik bleibt, aber `FromRow` beweist die neue Spalte.

Teste frische In-Memory-Migration, echte vor-009-zu-009-Migration (Altzeile bleibt leer), Insert/Fetch-Roundtrip und dass Retry die Version nicht verändert.

### 3. UI und Provenienz

Erweitere `IndexTemplate` und `BrowseTemplate` um `app_version`; `BrowseSummaryItem` erhält `rs_summarizer_version`. Befülle die Werte aus `AppState` bzw. `Summary`. Füge in beiden vollständigen HTML-Dokumenten einen sichtbaren `<footer><small>…</small></footer>` ein. In Browse zeigt eine Karte bei leerem Altwert „Version unbekannt“, nie die aktuelle App-Version als Ersatz.

Teste Askama-Rendering sowie Browse-Handler auf laufende Footer-Version, abweichende gespeicherte Version und Legacy-Anzeige.

### 4. Export

Ergänze in `src/commands/export_db.rs` die Spalte im Ziel-Schema, Quell-SELECT, Ziel-INSERT und Bind-Reihenfolge. Sie bleibt unabhängig von `include_embeddings` erhalten. Ein Export-Roundtrip-Test liest sie aus der erzeugten SQLite-Datei zurück.

### 5. Abschluss

Die bereits vorhandenen Änderungen von `Cargo.toml`/`Cargo.lock` (1.7.5 → 1.7.6) sind fremde Worktree-Änderungen und nur bei einem ausdrücklich vereinbarten Release-Bump zu committen. `walkthrough.md` erst nach realer Implementierung, grünen Gates und Commits erstellen.

## Tests und Abnahmekriterien

| Bereich | Nachweis |
| --- | --- |
| Versionsquelle | Zentraler Wert entspricht `env!("CARGO_PKG_VERSION")`; kein Manifest-Laufzeitlesen. |
| Frische DB | Alle Migrationen installieren 009; neue Zeile speichert die übergebene Version. |
| Upgrade | Reales vor-009-Schema akzeptiert 009; historische Werte bleiben `''`. |
| Unveränderlichkeit | Retry/Generation-Updates verändern das Feld nicht. |
| UI | `/` und `/browse` enthalten die laufende Version; Browse zeigt gespeicherte bzw. unbekannte Version. |
| Export | Die Spalte und ihr Wert bleiben im kompakten Export erhalten. |
| Regression | Alle bisherigen Test-Builder bauen mit dem erweiterten Vertrag. |

Nach Implementierung ausführen:

```bash
cargo fmt --check
cargo test db::
cargo test routes::
cargo test commands::export_db::
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
git diff --check
git status --short
```

Wenn der vorhandene WebDriver-Stack verfügbar ist:

```bash
TEST_BROWSER=chromium CHROMEDRIVER=/opt/archify-browser/chromedriver/chromedriver-linux64/chromedriver \
  cargo test --test integration_browser -- --ignored --test-threads=1
```

Kein Gemini-Live-Test: Das Feature ruft keinen Provider auf. Den Schlüssel aus `/workspace/src/.env` weder laden noch loggen.

## Commits

Conventional Commits mit ausführlichem Body: Motivation, Kompatibilität, geänderte Pfade und tatsächlich ausgeführte Tests. Keine Schlüssel, Datenbanken, Tarballs oder fremden Worktree-Änderungen committen.

1. `feat(version): persist summary generator version` — Migration 009, Legacy-Semantik, Insert-Quelle und Migration/Roundtrip-Tests.
2. `feat(version): display application and summary versions` — Footer, Trennung Laufzeit-/Erzeugerversion und UI-Tests.
3. `feat(export): retain summary generator version` — Export-Provenienz und Roundtrip-Test.
4. `docs(version): document delivered version provenance` — erst nach allen Gates; reale Ergebnisse, Learnings, Erweiterungen und Containerprogramme.

Ein atomarer Commit `feat(version): expose and persist application version` ist zulässig, falls Schritte 1–3 nicht separat releasebar sind; sein Body muss alle Aspekte und die komplette Testevidenz enthalten.

