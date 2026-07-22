# Implementierungsplan: Code-Review Fixes (Issues #1–#17)

Dieses Dokument beschreibt das Vorgehen zur Umsetzung der im Code-Review (`01_review_report.md`) identifizierten Verbesserungen. Die Fixes sind nach Priorität gruppiert und so strukturiert, dass sie von einem Subagenten schrittweise abgearbeitet werden können.

## 1. Übersicht der betroffenen Dateien

Jede Datei wird mit ihrem aktuellen Inhalt und den geplanten Änderungen zusammengefasst.

### Primäre Quelldateien (Änderungen erforderlich)

| # | Datei | Zeilen (ca.) | Aktueller Inhalt | Geplante Änderungen |
|:--|:------|:-------------|:------------------|:--------------------|
| 1 | `src/services/embedding.rs` | ~182 | `EmbeddingService` mit `cosine_similarity()`, `embed_text()`, `find_similar()`, Byte-Serialisierung. Enthält `assert!(!a.is_empty() && !b.is_empty())` in `cosine_similarity`. | **Fix #2**: `assert!` durch `if`-Guard ersetzen, der `0.0` zurückgibt statt zu paniken. |
| 2 | `src/main.rs` | ~351 | Anwendungs-Einstiegspunkt: Tracing-Init, CLI-Dispatch (`export-db`), DB-Pool-Setup, `AppState`-Erstellung, Visualisierungskomponenten-Laden, Axum-Server auf `127.0.0.1:5001`. | **Fix #3**: `std::fs::read_to_string` in `load_viz_data` durch `tokio::fs::read_to_string` ersetzen. **Fix #6**: Host/Port über Umgebungsvariablen `HOST`/`PORT` konfigurierbar machen. |
| 3 | `src/services/hacker_news.rs` | ~336 | `HackerNewsService` mit BFS-Kommentar-Crawler, Firebase-API-Client, `clean_html_to_text()`. Kompiliert Regex-Instanzen bei jedem Aufruf von `clean_html_to_text`. | **Fix #4**: Alle `Regex::new()`-Aufrufe in `clean_html_to_text` durch `std::sync::OnceLock`-gecachte Instanzen ersetzen. |
| 4 | `src/utils/url_validator.rs` | ~274 | YouTube- und HN-URL-Validierung mit `validate_youtube_url()`, `validate_hn_url()`, `parse_source_url()`. Kompiliert Regex-Patterns innerhalb von Validierungsfunktionen. | **Fix #4**: Regex-Patterns per `OnceLock` cachen. |
| 5 | `src/utils/markdown_converter.rs` | ~107 | `convert_markdown_to_youtube_format()` mit Bold-Konvertierung, Punctuation-Repositionierung, TLD-Dot-Ersetzung, Heading-Konvertierung. | **Fix #4**: Regex-Patterns per `OnceLock` cachen. **Fix #13**: Heading-Regex `r"^##\s*(.*)"` um Multiline-Flag `(?m)` erweitern. |
| 6 | `src/utils/timestamp_linker.rs` | ~155 | Timestamp-zu-YouTube-Link-Konvertierung mittels Regex `\b(?:\d{1,2}:)?[0-5]?\d:[0-5]\d\b`. | **Fix #12**: Regex verschärfen, um Zeitstempel nur in Textknoten zu matchen (nicht in HTML-Attributen/CSS). Negative Lookbehind für Zeichen wie `x` hinzufügen, um Formate wie `16:9` auszuschließen. |
| 7 | `src/routes/mod.rs` | ~266 | Axum-Handler: `index()`, `process_transcript()`, `get_generation()`, `browse_summaries()`, `search_similar()`. Enthält deutsche Fehlermeldungen und `unwrap_or_default()` bei Template-Rendering. | **Fix #10**: Deutsche Fehlermeldungen durch englische ersetzen. **Fix #11**: `unwrap_or_default()` durch explizites Error-Logging mit `tracing::error!` ersetzen. **Fix #14**: `DeduplicationService` aus dem Handler entfernen und stattdessen aus `AppState` beziehen. |
| 8 | `src/errors.rs` | ~98 | Fehler-Enums: `TranscriptError`, `SummaryError`, `EmbeddingError`, `ProcessError`, `ExportError`, `NnMapperError`. Deutsche Strings in `NnMapperError`. | **Fix #10**: Deutsche Fehlermeldungen in `NnMapperError` durch englische ersetzen. |
| 9 | `src/db.rs` | ~193 | SQLite-Zugriffsfunktionen mit hartcodierter Page-Size `20` in `fetch_browse_page`. | **Fix #15**: Page-Size als Parameter übergeben statt hartkodieren. |
| 10 | `src/state.rs` | ~216 | `AppState`-Definition, `ModelOption`, `get_default_models()`, per-Model `Mutex`-Locks. | **Fix #14**: `DeduplicationService`-Instanz als Feld in `AppState` hinzufügen. |

### Sekundäre Dateien (optional / informativ)

| # | Datei | Zeilen (ca.) | Aktueller Inhalt | Geplante Änderungen |
|:--|:------|:-------------|:------------------|:--------------------|
| 11 | `src/tasks.rs` | ~729 | Pipeline-Orchestrierung: `process_summary()`, `process_summary_inner()` (~460 Zeilen), Fallback-Ketten, RPM-Limiting, DB-Retry. | **Fix #1** (strategisch): `process_summary_inner` in Subfunktionen aufteilen: `fetch_youtube_content()`, `fetch_hn_content()`, `process_pasted_transcript()`, `run_model_pipeline()`, `finalize_and_embed()`. _Hinweis: Dieser Umbau ist umfangreich und sollte als separater Task geplant werden._ |
| 12 | `src/services/rate_limiter.rs` | ~185 | RPD-Enforcement, `check_rate_limit()` / `increment_counter()` als getrennte Operationen, DST-aware `today_la()`. | **Fix #7** (optional): `check_and_increment()` als atomare Operation einführen. _Niedrige Priorität, da die aktuelle TOCTOU-Race nur minimale Über-Limit-Fälle erlaubt._ |
| 13 | `src/services/nn_mapper.rs` | ~89 | GPU-UMAP-Projektion, `unsafe impl Send/Sync for NnMapper`. | **Fix #8** (Audit): `FittedUmap`-Internals prüfen. Falls thread-safe, Kommentar mit Safety-Begründung hinzufügen. Falls nicht, `Mutex`-Wrapper verwenden. |
| 14 | `src/services/summary.rs` | ~579 | Streaming-Summarisierung, Token-Fallback `len / 4`. | **Fix #9** (optional): Token-Fallback auf `chars().count() / 3` verbessern (bessere Approximation für Unicode-Text). |
| 15 | `src/utils/markdown_renderer.rs` | ~58 | `render_markdown_to_html()` via `pulldown-cmark`. | **Hinweis**: Für HTML-Sanitisierung (`ammonia`) muss `Cargo.toml` erweitert werden. _Niedrige Priorität, da Input nur von vertrauenswürdigem Gemini-Output stammt._ |

## 2. Vorgehensweise (nach Priorität)

### Phase 1: Quick Wins (Low Effort, High Impact)

Diese Fixes können unabhängig voneinander in beliebiger Reihenfolge umgesetzt werden.

1. **Fix #2 — `assert!`-Panic in `cosine_similarity` entfernen** (`src/services/embedding.rs`):
   - In `cosine_similarity(a: &[f32], b: &[f32])`: Den `assert!(!a.is_empty() && !b.is_empty())` durch ein `if a.is_empty() || b.is_empty() { return 0.0; }` ersetzen.
   - Bestehende Unit-Tests müssen weiterhin bestehen. Einen neuen Test `test_cosine_similarity_empty_vectors` hinzufügen.

2. **Fix #3 — Blocking I/O in Async-Kontext beheben** (`src/main.rs`):
   - In der Funktion `load_viz_data`: `std::fs::read_to_string(path)` durch `tokio::fs::read_to_string(path).await` ersetzen.
   - Falls `load_viz_data` noch nicht `async fn` ist, in eine `async fn` umwandeln und den Aufrufer anpassen.

3. **Fix #4 — Regex-Caching per `OnceLock`** (drei Dateien):
   - **`src/services/hacker_news.rs`**: In `clean_html_to_text` alle `Regex::new(...)` in statische `OnceLock<Regex>`-Variablen umwandeln:
     ```rust
     use std::sync::OnceLock;
     fn get_script_regex() -> &'static Regex {
         static RE: OnceLock<Regex> = OnceLock::new();
         RE.get_or_init(|| Regex::new(r"(?si)<script.*?</script>").unwrap())
     }
     ```
   - **`src/utils/url_validator.rs`**: Analog für alle Regex-Patterns in `validate_youtube_url` und `validate_hn_url`.
   - **`src/utils/markdown_converter.rs`**: Analog für alle Regex-Patterns in `convert_markdown_to_youtube_format`.

4. **Fix #10 — Sprachkonsistenz herstellen** (drei Dateien):
   - **`src/routes/mod.rs`**: Deutsche Fehlermeldungen (z.B. `"Fehler: Der eingegebene Wert '{}' ist weder..."`) durch englische Entsprechungen ersetzen (`"Error: The value '{}' is neither a valid YouTube URL..."`).
   - **`src/errors.rs`**: In `NnMapperError` die deutschen Strings durch englische ersetzen (z.B. `"Modell-Datei konnte nicht geladen werden"` → `"Failed to load model file"`).
   - **`src/main.rs`**: Deutsche Log-Nachrichten (z.B. `"COMPACT_DB_PATH nicht gesetzt..."`, `"Lade Visualisierungskomponenten..."`) durch englische ersetzen.

### Phase 2: Gezielte Verbesserungen (Medium Effort)

5. **Fix #6 — Host/Port konfigurierbar machen** (`src/main.rs`):
   - `std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string())` und `std::env::var("PORT").unwrap_or_else(|_| "5001".to_string())` verwenden.
   - Den hartkodierten `"127.0.0.1:5001"` String durch `format!("{}:{}", host, port)` ersetzen.

6. **Fix #11 — Template-Render-Fehler loggen** (`src/routes/mod.rs`):
   - Alle `template.render().unwrap_or_default()` durch ein Match ersetzen:
     ```rust
     match template.render() {
         Ok(html) => Html(html),
         Err(e) => {
             tracing::error!("Template render failed: {e}");
             Html("<p>Internal rendering error</p>".into())
         }
     }
     ```

7. **Fix #13 — Heading-Regex Multiline** (`src/utils/markdown_converter.rs`):
   - Die Heading-Regex von `r"^##\s*(.*)"` auf `r"(?m)^##\s*(.*)"` ändern, damit Headings nach Newlines erkannt werden.

8. **Fix #12 — Timestamp-Regex verschärfen** (`src/utils/timestamp_linker.rs`):
   - Den regulären Ausdruck so anpassen, dass nur echte Zeitstempel gematcht werden. Möglichkeit: Negative Lookbehind für Buchstaben oder den Kontext prüfen (`\b(\d{1,2}:)?[0-5]?\d:[0-5]\d\b`, aber nur wenn nicht von Nicht-Leerzeichen umgeben wie in `16:9`).
   - Alternativ: Nur Timestamps matchen, die mindestens 3 Stellen haben (z.B. `\b(?:\d{1,2}:)[0-5]\d:[0-5]\d\b` für `HH:MM:SS` oder `\b[0-5]?\d:[0-5]\d\b` mit Kontextprüfung).

9. **Fix #14 — `DeduplicationService` in `AppState` verschieben**:
   - In `src/state.rs`: `dedup_service: DeduplicationService` als Feld zu `AppState` hinzufügen, initialisiert mit `Duration::from_secs(300)`.
   - In `src/routes/mod.rs`: `DeduplicationService::new(...)` aus dem Handler entfernen und stattdessen `state.dedup_service` verwenden.

10. **Fix #15 — Page-Size parametrisieren** (`src/db.rs`):
    - `fetch_browse_page` um einen Parameter `page_size: u32` erweitern.
    - Den hartkodierten Wert `20` durch den Parameter ersetzen.
    - Aufrufer in `routes/mod.rs` und `cache.rs` anpassen.

### Phase 3: Strategische Verbesserungen (höherer Aufwand, separater Task)

Die folgenden Punkte sind als eigenständige Feature-Tasks zu planen und **nicht** Teil dieser Implementierungsrunde:

11. **Fix #1 — `process_summary_inner` aufteilen** (`src/tasks.rs`):
    - Ziel: Subfunktionen `fetch_youtube_content()`, `fetch_hn_content()`, `process_pasted_transcript()`, `run_model_pipeline()`, `finalize_and_embed()` extrahieren.
    - _Aufwand: Hoch. Eigenständiger Plan empfohlen._

12. **Fix #5 — Modellkonfigurationen externalisieren** (`src/state.rs`):
    - Ziel: `get_default_models()` aus einer JSON/YAML-Konfigurationsdatei laden.
    - _Aufwand: Mittel. Erfordert Serde-Deserialisierung und Fehlerhandling beim Startup._

13. **Fix #8 — `unsafe impl Send/Sync` in `nn_mapper.rs` auditieren**:
    - Ziel: `FittedUmap`-Internals prüfen; Safety-Kommentar oder `Mutex`-Wrapper hinzufügen.
    - _Aufwand: Niedrig, aber erfordert Expertenwissen über die `fast-umap`-Crate._

## 3. Zusammenfassung der zu ändernden Dateien

| Datei | Phase | Fix-Nummern | Art der Änderung |
|:------|:------|:------------|:-----------------|
| `src/services/embedding.rs` | 1 | #2 | Guard statt assert |
| `src/main.rs` | 1+2 | #3, #6, #10 | Async I/O, env-Konfiguration, Sprachfix |
| `src/services/hacker_news.rs` | 1 | #4 | Regex-Caching |
| `src/utils/url_validator.rs` | 1 | #4 | Regex-Caching |
| `src/utils/markdown_converter.rs` | 1+2 | #4, #13 | Regex-Caching, Multiline-Flag |
| `src/routes/mod.rs` | 1+2 | #10, #11, #14 | Sprachfix, Error-Logging, Service-Refactor |
| `src/errors.rs` | 1 | #10 | Sprachfix |
| `src/db.rs` | 2 | #15 | Page-Size-Parameter |
| `src/state.rs` | 2 | #14 | DeduplicationService-Feld |
| `src/utils/timestamp_linker.rs` | 2 | #12 | Regex-Verschärfung |
| `src/cache.rs` | 2 | #15 | Aufrufer-Anpassung für Page-Size |

## 4. Tests

Nach jeder Phase:
1. `cargo test` — alle Unit-Tests müssen bestehen.
2. `cargo clippy` — keine neuen Warnungen.
3. Manuelle Verifikation: Server starten (`cargo run`), Hauptseite aufrufen, eine URL absenden.

## 5. Report & Walkthrough

Nach Abschluss aller Änderungen wird ein `walkthrough.md` im selben Verzeichnis (`plan/20260722_02_code_review/`) erstellt, das folgende Punkte dokumentiert:
- Welche Fixes umgesetzt wurden (mit Diff-Zusammenfassung pro Datei).
- Welche Fixes bewusst ausgelassen oder auf Phase 3 verschoben wurden und warum.
- Aufgetretene Probleme oder unerwartete Erkenntnisse.
- Testergebnisse (`cargo test`-Output, `cargo clippy`-Output).
