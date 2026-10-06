# Serielle Aufgaben: rs-summarizer-Version

Diese Aufgaben sind strikt seriell. Nach einem fehlgeschlagenen Gate zuerst die Ursache beheben. Lies zunächst `plan.md`, `prompt.txt` und `deps.md`. Bewahre fremde Änderungen an `Cargo.toml`, `Cargo.lock` und vorhandenen `#walkthrough.md#`-Dateien.

## 1. Ausgangslage und Versionsvertrag

Lies `Cargo.toml`, `src/main.rs`, `src/state.rs`, alle `AppState`-Konstruktionen und `src/routes/mod.rs`. Bestätige mit `cargo metadata --no-deps --format-version 1` die Paketversion. Führe eine zentrale Compile-Time-Konstante und `AppState.app_version` ein; aktualisiere alle Builder.

Validiere:

```bash
cargo fmt --check
cargo test --lib
```

Commit: `feat(version): establish compile-time application version`

Der Body erklärt die Entscheidung gegen Runtime-Override und nennt die real ausgeführten Tests.

## 2. Migration und Persistenz

Lies `src/models.rs`, `src/db.rs` und Migrationen 001–008. Lege `009_add_rs_summarizer_version.sql` additiv an. Ergänze `Summary`, `insert_new_summary`, Route und alle direkten Test-Inserts. Der Wert wird nur bei Erstellung geschrieben.

Ergänze Tests für frische Migration, echte Legacy-009-Anwendung, Insert/Fetch und Nichtüberschreiben beim Retry.

Validiere:

```bash
cargo fmt --check
cargo test db::
cargo test --test integration_pipeline
cargo test --test integration_ratings
```

Commit: `feat(version): persist summary generator version`

## 3. Laufzeit- und Artefaktversion anzeigen

Lies `src/templates.rs`, `templates/index.html`, `templates/browse.html` sowie Index-/Browse-Handler. Übergib die Laufzeitversion an beide vollständigen Templates; ergänze einen zugänglichen Footer. Übergib für Browse außerdem die gespeicherte Summary-Version. Bei `''` „Version unbekannt“ anzeigen.

Füge Template-/Handler-Tests für Footer, abweichende gespeicherte Version und Legacy-Anzeige hinzu.

Validiere:

```bash
cargo fmt --check
cargo test routes::
cargo test --test integration_ratings
```

Falls vorhanden, zusätzlich seriell ausführen:

```bash
TEST_BROWSER=chromium CHROMEDRIVER=/opt/archify-browser/chromedriver/chromedriver-linux64/chromedriver \
  cargo test --test integration_browser -- --ignored --test-threads=1
```

Fehlt WebDriver, präzise dokumentieren, welches Programm fehlt; den deterministischen Handler-Test dennoch ausführen.

Commit: `feat(version): display application and summary versions`

## 4. Export-Provenienz

Lies `src/commands/export_db.rs` einschließlich Tests. Ergänze das reduzierte Zielschema, Quell-SELECT und Ziel-INSERT vollständig. Der Wert darf nicht von `include_embeddings` abhängen. Füge einen Export-Roundtrip-Test hinzu.

Validiere:

```bash
cargo fmt --check
cargo test commands::export_db::
```

Commit: `feat(export): retain summary generator version`

## 5. Abschluss und Walkthrough

Führe aus:

```bash
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
git diff --check
git status --short
```

Prüfe, dass 009 vorhanden ist, frühere Migrationen unverändert sind und keine Geheimnisse/DBs/Artefakte staged sind. Erst danach `walkthrough.md` erzeugen: reale Implementierung, Migration/Legacy-Semantik, Befehle und Ergebnisse, Commits, Einschränkungen, Follow-ups und Containerprogramme (erwartet: keine).

Commit: `docs(version): document delivered version provenance`

