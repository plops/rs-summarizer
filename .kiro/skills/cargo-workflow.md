# Cargo-Workflow: Dependency-Management

Diese Datei dokumentiert den Workflow für das Verwalten von Rust-Dependencies in rs-summarizer und seinen Workspace-Membern.

## Dependency-Format in `Cargo.toml`

Dependencies werden im folgenden Format eingetragen:

```toml
# Einfache Dependency
name = "x.y.z"

# Dependency mit Features
name = { version = "x.y.z", features = ["feature1", "feature2"] }

# Dependency mit deaktivierten Default-Features
name = { version = "x.y.z", default-features = false, features = ["feature1"] }

# Dev-Dependency
[dev-dependencies]
proptest = "1.0"
```

Versionen werden immer als exakte Minor-Version angegeben (z.B. `"1.7.1"` statt `"1"`), damit Upgrades bewusst durchgeführt werden.

## Neue Dependency hinzufügen

1. **DeepWiki-Lookup** (vor der Implementierung): Verwende das DeepWiki MCP-Tool, um Informationen über die GitHub-Repository der Dependency zu erhalten. Format: `<github-organisation>/<projekt>` (z.B. `emilk/egui`).

2. **Version ermitteln**: Aktuelle stabile Version auf [crates.io](https://crates.io) nachschlagen oder via `cargo search <crate-name>`.

3. **In `Cargo.toml` eintragen**: Dependency mit exakter Version und benötigten Features hinzufügen.

4. **`deps.md` aktualisieren**: Eine Zeile pro Dependency im Format `<github-organisation>/<projekt>` in die `deps.md`-Datei des jeweiligen Crates eintragen (siehe unten).

## `cargo upgrade` — Versionen aktualisieren

`cargo upgrade` ist Teil von `cargo-edit` und setzt die neuesten Versionsnummern in `Cargo.toml`:

```bash
# cargo-edit installieren (einmalig)
cargo install cargo-edit

# Alle Dependencies auf neueste kompatible Versionen upgraden
cargo upgrade

# Auch inkompatible (Major-Version) Upgrades erlauben
cargo upgrade --incompatible allow

# Nur eine bestimmte Dependency upgraden
cargo upgrade <crate-name>
```

Nach `cargo upgrade` immer `cargo check` und `cargo test` ausführen, um sicherzustellen dass keine Breaking Changes eingeführt wurden.

## DeepWiki MCP für Dependency-Recherche

Vor der Implementierung einer neuen Dependency das DeepWiki MCP-Tool verwenden:

```
# Beispiel: Informationen über fast-umap abrufen
mcp_deepwiki_ask_question(
    repoName="eugenehp/fast-map",
    question="How do I use the GPU backend for parametric UMAP?"
)
```

DeepWiki liefert kontextuelle Dokumentation direkt aus dem GitHub-Repository — nützlich für:
- API-Nutzung und Beispiele
- Feature-Flags und ihre Auswirkungen
- Bekannte Einschränkungen (z.B. CPU-Backend unterstützt kein `transform`)
- Kompatibilitätsprobleme zwischen Crates

## `deps.md`-Konvention

Jedes Crate im Workspace, das externe Dependencies hat, führt eine `deps.md`-Datei im Crate-Verzeichnis. Format: eine Zeile pro Dependency, `<github-organisation>/<projekt>`:

```
# deps.md — GitHub-Repositories der Dependencies
emilk/egui
emilk/egui_plot
eugenehp/fast-map
rust-ml/linfa
launchbadge/sqlx
tokio-rs/tokio
flachesis/gemini-rust
serde-rs/serde
dtolnay/thiserror
dtolnay/anyhow
```

Die `deps.md` dient als schnelle Referenz für DeepWiki-Lookups und macht die Herkunft jeder Dependency transparent.

## Workspace-Setup

Das rs-summarizer-Projekt verwendet einen Cargo-Workspace mit mehreren Members:

```toml
# rs-summarizer/Cargo.toml
[workspace]
members = [".", "viz-tool"]

[package]
name = "rs-summarizer"
# ... bestehende Felder
```

Workspace-Members teilen einen gemeinsamen `Cargo.lock` und `target/`-Verzeichnis. Jedes Member hat seine eigene `Cargo.toml` mit eigenem `[package]`-Abschnitt.

## Häufige Probleme

- **ndarray-Versionskonflikte**: Wenn mehrere Crates `ndarray` transitiv verwenden (z.B. `fast-umap` und `linfa-clustering`), können Versionskonflikte entstehen. Nach dem Hinzufügen beider Dependencies `cargo upgrade --incompatible allow` ausführen und dann `cargo check` prüfen.
- **sqlx Offline-Modus**: Bei Compile-Fehler durch fehlende Datenbank `SQLX_OFFLINE=true cargo check` verwenden.
- **WGPU-Backend**: `fast-umap` mit `features = ["gpu"]` verwendet das WGPU-Backend, das auch ohne dedizierte GPU via Software-Rasterizer läuft. Das CPU-Backend (`fit_cpu`) unterstützt kein `transform` — für Out-of-Sample-Projektion immer das GPU-Backend verwenden.
