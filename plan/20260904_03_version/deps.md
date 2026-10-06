# Dependency- und API-Recherche

## Ergebnis

Für dieses Feature wird keine Dependency eingeführt oder aktualisiert. Die Anwendung kann die vom Rust-Compiler bereitgestellte Manifestvariable direkt verwenden:

```rust
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
```

Das benötigt keine Laufzeitdatei, keine neue Crate und keine Netzwerk-/Gemini-Anfrage. Vorhandenes `sqlx`, Askama und SQLite werden nur über ihre bestehenden Pfade erweitert.

## DeepWiki-Abgleich

Die Anfrage zu `plops/rs-summarizer` bestätigt:

- `src/db.rs::init_db` führt eingebettete `migrations/` aus;
- `src/models.rs::Summary`, `insert_new_summary` und die `SELECT *`-Ladepfade bilden die Persistenzkette;
- `AppState` und Askama-Templates bilden die UI-Kette;
- `src/db.rs` enthält bereits ein reales Legacy-Migrationsmuster, das für 009 wiederverwendet wird.

Keine neue externe Dependency bedeutet: keine GitHub-Organisation, DeepWiki-Dependency-Abfrage oder Usage-Example zur globalen Dependency-Dokumentation.

## Containerprogramme

Keine neuen Programme erforderlich. Vorhandenes Rust/Cargo, SQLite via `sqlx` sowie gegebenenfalls Chromium/ChromeDriver reichen aus.

