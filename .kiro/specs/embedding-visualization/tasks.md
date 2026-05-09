# Implementation Plan: embedding-visualization

## Overview

Implementierung der vollständigen Embedding-Visualisierungs-Pipeline in Rust. Das Feature besteht aus sechs Komponenten: Cargo-Skill-Dokumentation, DB_Exporter CLI, Viz_Tool Desktop-GUI (egui), Cluster_Titler (Gemini API), NN_Mapper (parametric UMAP) und Web_Viz (HTMX-Integration). Die Implementierung folgt der Reihenfolge: Infrastruktur → Datenpipeline → GUI → Web-Integration.

## Tasks

- [X] 1. Cargo-Workflow-Skill und Workspace-Setup
  - Erstelle `.kiro/skills/cargo-workflow.md` mit Dokumentation für Dependency-Management (Format, `cargo upgrade`, DeepWiki-Lookups, `deps.md`-Konvention)
  - Erweitere `rs-summarizer/Cargo.toml` um `[workspace]` mit `members = [".", "viz-tool"]` (bestehender `[package]`-Abschnitt bleibt erhalten)
  - Erstelle `viz-tool/Cargo.toml` mit `[package]`-Abschnitt (name = "viz-tool", edition = "2021") und allen benötigten Dependencies: `eframe`, `egui_plot`, `fast-umap` mit `features = ["gpu"]`, `linfa-clustering`, `sqlx` mit SQLite-Feature, `tokio`, `gemini-rust`, `serde`/`serde_json`, `thiserror`, `anyhow`
  - Erstelle `viz-tool/deps.md` mit allen GitHub-Repositories der Dependencies (eine Zeile pro Dependency im Format `<org>/<projekt>`)
  - Erstelle leere Verzeichnisstruktur `viz-tool/src/` mit Platzhalter-`main.rs`
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_

- [X] 2. Fehler-Typen und Embedding-Serialisierung
  - [X] 2.1 Erstelle `viz-tool/src/errors.rs` mit vollständigem `VizError`-Enum (Database, Io, Umap, Dbscan, Api, DimensionMismatch, InvalidBlobLength, BlobTooShort, ModelLoadError, NoEmbeddings, UmapNotComputed, InsufficientPoints)
    - Verwende `thiserror::Error` mit deutschen Fehlermeldungen wie im Design
    - _Requirements: 12.4, 12.5_

  - [X] 2.2 Erstelle `viz-tool/src/embedding.rs` mit `bytes_to_embedding`, `embedding_to_bytes` und `bytes_to_embedding_truncated`
    - `bytes_to_embedding(bytes: &[u8]) -> Vec<f32>` — Little-Endian f32-Deserialisierung (analog zu `src/services/embedding.rs`)
    - `embedding_to_bytes(embedding: &[f32]) -> Vec<u8>` — Little-Endian f32-Serialisierung
    - `bytes_to_embedding_truncated(bytes: &[u8], dim: usize) -> Result<Vec<f32>, VizError>` — prüft Länge (Vielfaches von 4, mind. `dim * 4` Bytes), gibt `VizError::InvalidBlobLength` bzw. `VizError::BlobTooShort` zurück
    - _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5_

  - [X]* 2.3 Schreibe Property-Test für Embedding Round-Trip (Property 1)
    - **Property 1: Embedding Round-Trip**
    - **Validates: Requirements 12.1, 12.2**
    - `proptest!` in `viz-tool/src/embedding.rs`: `values in prop::collection::vec(prop::num::f32::NORMAL, 1..=3072)` → `embedding_to_bytes` → `bytes_to_embedding` → bit-exakter Vergleich

  - [X]* 2.4 Schreibe Property-Test für Embedding Truncation (Property 2)
    - **Property 2: Embedding Truncation**
    - **Validates: Requirements 12.3, 4.4**
    - `proptest!` in `viz-tool/src/embedding.rs`: `values in vec(f32::NORMAL, 768..=3072), dim in 1..=768` → `bytes_to_embedding_truncated` → Länge == dim, erste dim Elemente bit-exakt

  - [X]* 2.5 Schreibe Unit-Tests für Fehlerbehandlung in `embedding.rs`
    - `test_bytes_to_embedding_empty` — leerer BLOB → leerer Vec
    - `test_invalid_blob_length` — BLOB-Länge kein Vielfaches von 4 → `VizError::InvalidBlobLength`
    - `test_blob_too_short` — BLOB kürzer als `embedding_dim * 4` → `VizError::BlobTooShort`
    - _Requirements: 12.4, 12.5_

- [X] 3. DB_Exporter CLI in rs-summarizer
  - [ ] 3.1 Erstelle `src/commands/export_db.rs` mit `ExportDbArgs`-Struct und `run_export`-Funktion
    - `ExportDbArgs { source: PathBuf, output: PathBuf }`
    - `pub async fn run_export(args: ExportDbArgs) -> anyhow::Result<()>`
    - Implementiere alle Validierungsschritte: SourceNotFound, OutputExists, OutputDirMissing
    - Öffne Source_DB read-only, erstelle Compact_DB mit WAL-Mode
    - Erstelle Schema (ohne `transcript`-Feld, mit allen anderen Feldern aus Requirement 2.2)
    - Kopiere Zeilen mit `WHERE embedding IS NOT NULL AND summary_done = 1`
    - Prüfe auf mindestens eine exportierte Zeile (NoQualifyingRows)
    - Gib Zeilenanzahl und Dateigröße auf stdout aus
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9, 2.10, 2.11_

  - [ ] 3.2 Erstelle `src/commands/mod.rs` und füge `ExportError`-Enum in `src/errors.rs` hinzu
    - `ExportError` mit Varianten: SourceNotFound, OutputExists, OutputDirMissing, NoQualifyingRows, Database, Io
    - Exportiere `commands`-Modul aus `src/lib.rs`
    - _Requirements: 2.7, 2.8, 2.10, 2.11_

  - [ ] 3.3 Erweitere `src/main.rs` um CLI-Argument-Auswertung für `export-db`
    - Werte `std::env::args()` aus bevor der axum-Server gestartet wird
    - Parse `export-db --source <pfad> --output <pfad>` manuell (kein clap)
    - Rufe `run_export` auf und beende den Prozess; starte Server nur wenn kein `export-db`-Argument vorhanden
    - _Requirements: 2.1_

  - [ ]* 2.6 Schreibe Property-Tests für Export-Korrektheit (Properties 3, 4, 5)
    - **Property 3: Export Field Correctness** — Validates: Requirements 2.2, 2.3
    - **Property 4: Export Filter Correctness** — Validates: Requirements 2.4
    - **Property 5: Source DB Immutability** — Validates: Requirements 2.5
    - Temporäre SQLite-Datenbanken in `tempfile::TempDir`, proptest mit variablen Zeilen-Kombinationen
    - _Hinweis: Als `proptest!` in `src/commands/export_db.rs` oder separater Testdatei_

  - [ ]* 3.4 Schreibe Unit-Tests für `export_db.rs`
    - `test_output_exists_error`, `test_no_qualifying_rows_error`, `test_source_not_found`, `test_output_dir_missing`, `test_wal_mode_enabled`
    - _Requirements: 2.7, 2.8, 2.9, 2.10, 2.11_

- [X] 4. Checkpoint — Cargo check und Tests
  - Führe `cargo check --workspace` aus, stelle sicher dass Workspace-Setup und alle bisherigen Module kompilieren.
  - Stelle sicher dass alle Tests in `src/` weiterhin bestehen.

- [X] 5. Viz_Tool — Datenmodelle und DataLoader
make sure to also look at /home/kiel/stage/rs-summarizer/.kiro/specs/embedding-visualization/design.md and use deepwiki mcp to ask how to use dependencies listed in /home/kiel/stage/rs-summarizer/viz-tool/deps.md
  - [ ] 5.1 Erstelle `viz-tool/src/data_loader.rs` mit `EmbeddingPoint`-Struct, `LoadResult`-Struct und `load_compact_db`-Funktion
    - `EmbeddingPoint { identifier: i64, original_source_link, summary, model, embedding_model, timestamped_summary, embedding: Vec<f32> }`
    - `LoadResult { points: Vec<EmbeddingPoint>, skipped_invalid_length: usize, skipped_too_short: usize }`
    - `pub async fn load_compact_db(path: &Path, embedding_dim: usize) -> Result<LoadResult, VizError>`
    - Verwende `sqlx` mit `SqliteConnectOptions` (read-only), lese alle Zeilen mit `embedding IS NOT NULL`
    - Deserialisiere BLOBs via `bytes_to_embedding_truncated`, überspringe ungültige BLOBs mit `eprintln!`-Warnung
    - _Requirements: 4.3, 4.4, 4.5, 4.7, 4.8, 4.9, 12.3, 12.4, 12.5, 12.6_

  - [ ]* 5.2 Schreibe Property-Test für Valid BLOB Count (Property 8)
    - **Property 8: Valid BLOB Count**
    - **Validates: Requirements 12.6, 4.6**
    - Generiere Mischung aus gültigen und ungültigen BLOBs, prüfe dass `points.len()` exakt der Anzahl gültiger BLOBs entspricht

- [X] 6. Viz_Tool — UmapEngine
make sure to also look at /home/kiel/stage/rs-summarizer/.kiro/specs/embedding-visualization/design.md and use deepwiki mcp to ask how to use dependencies listed in /home/kiel/stage/rs-summarizer/viz-tool/deps.md
  - [ ] 6.1 Erstelle `viz-tool/src/umap_engine.rs` mit `UmapParams`-Struct, `compute_umap`- und `fit_parametric_umap`-Funktionen
    - `UmapParams { n_components, n_neighbors, min_dist, n_epochs: usize (Standard: 200) }`
    - `pub fn compute_umap(embeddings: &[Vec<f32>], params: UmapParams) -> Result<Vec<Vec<f32>>, VizError>`
    - Verwende WGPU-Backend (`Umap::new(config).fit(data, None)`) — kein CPU-Backend
    - Prüfe `n_neighbors < embeddings.len()` vor dem Aufruf → `VizError::InsufficientPoints`
    - `pub fn fit_parametric_umap(embeddings: &[Vec<f32>], params: UmapParams) -> Result<FittedUmap, VizError>`
    - _Requirements: 5.1, 5.2, 5.3, 5.7, 5.8, 5.9, 3.4_

- [X] 7. Viz_Tool — DbscanEngine
make sure to also look at /home/kiel/stage/rs-summarizer/.kiro/specs/embedding-visualization/design.md and use deepwiki mcp to ask how to use dependencies listed in /home/kiel/stage/rs-summarizer/viz-tool/deps.md
  - [ ] 7.1 Erstelle `viz-tool/src/dbscan_engine.rs` mit `DbscanParams`-Struct und `compute_dbscan`-Funktion
    - `DbscanParams { eps: f64, min_samples: usize }`
    - `pub fn compute_dbscan(embeddings_4d: &[[f32; 4]], params: DbscanParams) -> Result<Vec<i32>, VizError>`
    - Konvertiere `&[[f32; 4]]` zu `ndarray::Array2<f64>` (linfa erwartet f64)
    - Führe Clustering durch via `Dbscan::params(min_samples).tolerance(eps).transform(&dataset)`
    - Konvertiere `Array1<Option<usize>>`: `None` → `-1`, `Some(id)` → `id as i32`
    - _Requirements: 6.1, 6.2, 6.3, 6.7, 6.8_

- [X] 8. Viz_Tool — ClusterTitler
make sure to also look at /home/kiel/stage/rs-summarizer/.kiro/specs/embedding-visualization/design.md and use deepwiki mcp to ask how to use dependencies listed in /home/kiel/stage/rs-summarizer/viz-tool/deps.md
  - [ ] 8.1 Erstelle `viz-tool/src/cluster_titler.rs` mit `extract_abstract_block`, `generate_titles`, `save_titles`, `load_titles`
    - `pub fn extract_abstract_block(summary: &str) -> Option<String>` — sucht `**Abstract**:` (case-insensitive), extrahiert Text bis zum ersten Timestamp-Marker (`\n.*\d+:\d{2}`)
    - `pub async fn generate_titles(points, labels, api_key, model_name) -> Result<HashMap<i32, String>, VizError>`
    - Batching-Logik: Prompts akkumulieren bis 20.000 Wörter, dann separater API-Aufruf
    - Verwende `Gemini::with_model(&api_key, Model::Custom(format!("models/{}", model_name)))` mit `with_response_mime_type("application/json")`
    - JSON-Schema: `[{"id": <cluster_id>, "title": "<titel>"}]`
    - `pub fn save_titles(titles: &HashMap<i32, String>, path: &Path) -> Result<(), VizError>`
    - `pub fn load_titles(path: &Path) -> Result<HashMap<i32, String>, VizError>`
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7, 8.8, 8.9_

  - [ ]* 8.2 Schreibe Property-Test für Abstract Block Extraction (Property 6)
    - **Property 6: Abstract Block Extraction**
    - **Validates: Requirements 8.2**
    - `proptest!` in `cluster_titler.rs`: generiere `abstract_text` + `suffix` + Timestamp-Suffix, prüfe dass Block den Abstract-Text enthält und keinen Timestamp

  - [ ]* 8.3 Schreibe Unit-Tests für `cluster_titler.rs`
    - `test_extract_abstract_no_marker` — kein `**Abstract**:` → `None`
    - `test_extract_abstract_no_timestamp` — kein Timestamp → gesamter Text nach Marker
    - `test_extract_abstract_case_insensitive` — `abstract:` (lowercase) wird erkannt
    - _Requirements: 8.2_

- [X] 9. Viz_Tool — NnMapper
make sure to also look at /home/kiel/stage/rs-summarizer/.kiro/specs/embedding-visualization/design.md and use deepwiki mcp to ask how to use dependencies listed in /home/kiel/stage/rs-summarizer/viz-tool/deps.md
  - [ ] 9.1 Erstelle `viz-tool/src/nn_mapper.rs` mit `NnMapper`-Struct (train, project, save, load)
    - `NnMapper { fitted: FittedUmap, embedding_dim: usize }`
    - `pub fn train(embeddings, embedding_dim, params) -> Result<Self, VizError>` — ruft `fit_parametric_umap` auf
    - `pub fn project(&self, embedding: &[f32]) -> Result<(f32, f32), VizError>` — prüft Dimension, ruft `self.fitted.transform(...)` auf
    - `pub fn save(&self, path: &Path) -> Result<(), VizError>` — `FittedUmap::save(path)` + Sidecar-JSON (`_nn_mapper_config.json`) mit `UmapConfig` und `embedding_dim`
    - `pub fn load(path: &Path, embedding_dim: usize) -> Result<Self, VizError>` — liest Sidecar-JSON, ruft `FittedUmap::load(path, config, input_size, device)` auf
    - _Requirements: 9.1, 9.4, 9.5, 9.6, 9.7, 9.8, 9.9_

  - [ ]* 9.2 Schreibe Unit-Test für Dimension-Mismatch
    - `test_project_dimension_mismatch` — falsches Embedding → `VizError::DimensionMismatch`
    - _Requirements: 9.8_

- [X] 10. Viz_Tool — VizApp (egui GUI)
make sure to also look at /home/kiel/stage/rs-summarizer/.kiro/specs/embedding-visualization/design.md and use deepwiki mcp to ask how to use dependencies listed in /home/kiel/stage/rs-summarizer/viz-tool/deps.md
  - [ ] 10.1 Erstelle `viz-tool/src/app.rs` mit `VizApp`-Struct, `AppStatus`-Enum, `ComputeResult`-Enum und `eframe::App`-Implementierung
    - Implementiere alle State-Felder aus dem Design: `db_path`, `points`, `embedding_dim`, UMAP-Parameter, DBSCAN-Parameter, `cluster_titles`, `nn_mapper`, `status`, `error_message`, `skipped_blobs`, `compute_tx`/`compute_rx`
    - Implementiere `update`-Methode: empfange `ComputeResult` via `compute_rx.try_recv()`, aktualisiere State, rufe `ctx.request_repaint()` auf
    - _Requirements: 4.6, 5.6, 6.8, 7.6_

  - [ ] 10.2 Implementiere GUI-Layout in `VizApp::update`: linkes Control-Panel + rechter Scatter-Plot
    - Linkes Panel: Status-Banner, UMAP-4D-Parameter (Schieberegler n_neighbors 2–200, min_dist 0.0–1.0), UMAP-2D-Parameter, "Berechnen"-Button (deaktiviert während Berechnung), Fortschrittsindikator
    - DBSCAN-Panel: eps-Eingabe (> 0.0, rot bei ungültig), min_samples-Eingabe (≥ 1), "Clustern"-Button (deaktiviert wenn kein 4D-UMAP), Cluster-Anzahl-Anzeige
    - Buttons: "Cluster-Titel generieren", "NN-Mapper trainieren" (deaktiviert wenn kein 2D-UMAP)
    - Fehlermeldungen in rotem Banner
    - _Requirements: 5.4, 5.5, 5.6, 6.4, 6.5, 6.6, 6.9, 7.6, 8.1, 9.2, 9.3_

  - [ ] 10.3 Implementiere Scatter-Plot mit `egui_plot` in `VizApp::update`
    - Erstelle pro Cluster ein separates `Points`-Item mit Cluster-Titel als Name (für Hover-Tooltip)
    - `PlotPoints` aus `[f64; 2]` (f32-Koordinaten konvertieren)
    - Einfärbung nach Cluster-Label (zyklische Farbpalette, Rauschen = Grau)
    - `label_formatter` für Hover-Tooltip mit `original_source_link` und Summary
    - Cluster-Titel am Zentroid (arithmetisches Mittel der 2D-Koordinaten)
    - Zoom und Pan via egui_plot-Standard
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6_

  - [ ] 10.4 Implementiere Hintergrundberechnungen via `std::thread::spawn` + `mpsc::channel`
    - Laden: `thread::spawn` → `load_compact_db` → `ComputeResult::LoadDone`
    - UMAP: `thread::spawn` → `compute_umap` (4D + 2D) → `ComputeResult::UmapDone`
    - DBSCAN: `thread::spawn` → `compute_dbscan` → `ComputeResult::DbscanDone`
    - Cluster-Titel: `thread::spawn` + tokio-Runtime → `generate_titles` → `ComputeResult::TitlesDone`
    - NN-Mapper-Training: `thread::spawn` → `NnMapper::train` → `ComputeResult::NnMapperDone`
    - Alle Fehler als `ComputeResult::Error(String)` zurücksenden
    - _Requirements: 5.5, 5.6, 6.5, 8.1, 9.3_

  - [ ] 10.5 Implementiere Auto-Laden von gespeicherten Artefakten beim Start
    - Beim Laden einer Compact_DB: prüfe ob `<stem>_cluster_titles.json` existiert → lade Titel automatisch
    - Beim Laden einer Compact_DB: prüfe ob `<stem>_nn_mapper.bin` existiert → lade NnMapper automatisch
    - Speichere Titel nach Generierung in `<stem>_cluster_titles.json`
    - Speichere NN-Mapper nach Training in `<stem>_nn_mapper.bin`
    - _Requirements: 8.7, 8.8, 9.4, 9.5, 9.6_

  - [ ] 10.6 Erstelle `viz-tool/src/main.rs` mit `eframe::run_native` und CLI-Argument-Parsing
    - Parse optionalen Pfad-Parameter aus `std::env::args()` (kein clap)
    - Parse optionalen `--embedding-dim <n>`-Parameter (Standard: 768)
    - Wenn Pfad angegeben: direkt laden ohne Datei-Dialog
    - Wenn kein Pfad: nativen Datei-Öffnen-Dialog anzeigen (gefiltert auf `.db`)
    - _Requirements: 4.1, 4.2_

- [ ] 11. Checkpoint — Viz_Tool kompiliert und Basis-Tests bestehen
  - Führe `cargo check --workspace` aus.
  - Führe `cargo test --package viz-tool` aus, stelle sicher dass alle Unit- und Property-Tests bestehen.

- [ ] 12. Web_Viz — AppState-Erweiterung und NnMapper-Service
make sure to also look at /home/kiel/stage/rs-summarizer/.kiro/specs/embedding-visualization/design.md and use deepwiki mcp to ask how to use dependencies  listed in /home/kiel/stage/rs-summarizer/viz-tool/deps.md
  - [ ] 12.1 Erstelle `src/services/nn_mapper.rs` mit `NnMapper`-Struct (load, project) für den Web-Server
    - `NnMapper { fitted: FittedUmap, embedding_dim: usize }`
    - `pub fn load(model_path: &Path) -> Result<Self, NnMapperError>` — liest Sidecar-JSON, lädt `FittedUmap`
    - `pub fn project(&self, embedding: &[f32]) -> Result<(f32, f32), NnMapperError>` — ruft `self.fitted.transform(...)` auf
    - Füge `NnMapperError`-Enum zu `src/errors.rs` hinzu
    - _Requirements: 9.9, 10.1, 10.2_

  - [ ] 12.2 Erstelle `VizData`-Struct in `src/models.rs` und erweitere `AppState` in `src/state.rs`
    - `VizData { points_2d: Vec<(i64, f32, f32)>, cluster_labels: HashMap<i64, i32>, cluster_titles: HashMap<i32, String>, cluster_centroids: HashMap<i32, (f32, f32)> }`
    - Erweitere `AppState` um `nn_mapper: Option<Arc<NnMapper>>` und `viz_data: Option<Arc<VizData>>`
    - _Requirements: 10.1, 10.3, 10.4, 11.1_

  - [ ] 12.3 Implementiere Laden von `VizData` und `NnMapper` beim Server-Start in `src/main.rs`
    - Lese `COMPACT_DB_PATH`-Umgebungsvariable
    - Falls gesetzt: lade `VizData` aus Compact_DB + `_cluster_titles.json`, lade `NnMapper` aus `_nn_mapper.bin`
    - Falls Dateien fehlen: `viz_data: None`, `nn_mapper: None` (kein Fehler)
    - _Requirements: 10.8, 10.9, 11.6_

- [ ] 13. Web_Viz — Routen und SVG-Rendering
make sure to also look at /home/kiel/stage/rs-summarizer/.kiro/specs/embedding-visualization/design.md and use deepwiki mcp to ask how to use dependencies  listed in /home/kiel/stage/rs-summarizer/viz-tool/deps.md
  - [ ] 13.1 Erstelle `src/routes/viz.rs` mit `viz_map`- und `viz_search_map`-Routen und `find_k_nearest_2d`-Hilfsfunktion
    - `pub async fn viz_map(State(app): State<AppState>, Path(identifier): Path<i64>) -> impl IntoResponse`
    - `pub async fn viz_search_map(State(app): State<AppState>, Form(query): Form<SearchForm>) -> impl IntoResponse`
    - `pub fn find_k_nearest_2d(points: &[(f32, f32)], query: (f32, f32), k: usize) -> Vec<(f32, f32)>` — sortiert nach aufsteigender euklidischer Distanz
    - Rendere inline SVG: `<circle>`-Elemente für Punkte, `<text>`-Elemente für Cluster-Titel an Zentroiden
    - Aktuelles Video hervorgehoben (größer, anderer Farbton), 5 nächste Nachbarn hervorgehoben
    - Falls `nn_mapper: None` oder kein Embedding: leere Response
    - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.7, 10.8, 10.9, 11.1, 11.2, 11.3, 11.4, 11.5, 11.6, 11.7_

  - [ ]* 13.2 Schreibe Property-Test für Nearest-Neighbor Correctness (Property 7)
    - **Property 7: Nearest-Neighbor Correctness**
    - **Validates: Requirements 10.7**
    - `proptest!` in `src/routes/viz.rs` oder `src/utils/viz_utils.rs`: generiere Punkte + Query-Punkt + k, prüfe Sortierung und Vollständigkeit

  - [ ] 13.3 Registriere Viz-Routen im axum-Router in `src/lib.rs` oder `src/routes/mod.rs`
    - Füge `GET /viz/map/:identifier` und `POST /viz/search-map` zum Router hinzu
    - _Requirements: 10.1, 11.1_

  - [ ] 13.4 Erstelle/erweitere Askama-Templates für Web_Viz-Integration
    - Füge Karten-Abschnitt in Summary-Detail-Template ein (HTMX-Partial via `hx-get="/viz/map/{identifier}"`)
    - Füge Visualisierungs-Abschnitt in Suchergebnis-Template ein
    - Klickbare Punkte (SVG `<a>`-Elemente) und Tooltips (`<title>`-Elemente)
    - Liste der 5 nächsten Nachbarn als klickbare Links unterhalb der Karte
    - _Requirements: 10.5, 10.6, 10.7, 11.4, 11.5_

- [ ] 14. Finale Checkpoint — Alle Tests bestehen
  - Führe `cargo test --workspace` aus, stelle sicher dass alle Unit-, Property- und Integrationstests bestehen.
  - Führe `cargo check --workspace` aus, stelle sicher dass keine Compiler-Fehler vorhanden sind.

## Notes

- Tasks mit `*` sind optional und können für ein schnelleres MVP übersprungen werden
- Jeder Task referenziert spezifische Requirements für Traceability
- Das WGPU-Backend (`features = ["gpu"]`) wird für fast-umap verwendet — läuft auch ohne dedizierte GPU via Software-Rasterizer
- `linfa` erwartet `Array2<f64>` und gibt `Array1<Option<usize>>` zurück (`None` = Rauschen → `-1`)
- `FittedUmap::save/load` aus fast-umap v1.4.0+ — kein externes `bincode` nötig
- Sidecar-JSON (`_nn_mapper_config.json`) speichert `UmapConfig` + `embedding_dim` für `FittedUmap::load`
- Property-Tests laufen mit `proptest` (bereits in `[dev-dependencies]` des Hauptprojekts; auch in `viz-tool/Cargo.toml` eintragen)
- Hintergrundberechnungen im Viz_Tool folgen dem Pattern aus `rs_las_ctl`: `std::thread::spawn` + `mpsc::channel` + `ctx.request_repaint()`

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["2.1", "3.2"] },
    { "id": 1, "tasks": ["2.2", "3.1"] },
    { "id": 2, "tasks": ["2.3", "2.4", "2.5", "3.3", "5.1"] },
    { "id": 3, "tasks": ["2.6", "3.4", "5.2", "6.1", "7.1"] },
    { "id": 4, "tasks": ["8.1", "9.1"] },
    { "id": 5, "tasks": ["8.2", "8.3", "9.2", "10.1"] },
    { "id": 6, "tasks": ["10.2", "10.3", "10.4", "12.1"] },
    { "id": 7, "tasks": ["10.5", "10.6", "12.2"] },
    { "id": 8, "tasks": ["12.3"] },
    { "id": 9, "tasks": ["13.1"] },
    { "id": 10, "tasks": ["13.2", "13.3", "13.4"] }
  ]
}
```
