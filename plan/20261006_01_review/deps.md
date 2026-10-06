# Dependency- und API-Recherche

## Ergebnis

Für dieses Feature wird keine Dependency eingeführt oder aktualisiert. Alle Aufgaben
(Timestamp-Regex, Preisformatierung, Retry-Anzeige, Fehlermeldungen, Copy-Button,
NULL-robuste SELECTs) lassen sich mit bestehenden Crates lösen:

- `regex`, `chrono`, `sqlx`, `askama` decken Formatierung, Zeitdarstellung,
  `COALESCE`-Abfragen und Templates ab;
- der YouTube-Copy-Button nutzt die native Browser-Clipboard-API
  (`navigator.clipboard.writeText` mit `execCommand`-Fallback), kein JS-Paket.

Bewusst nicht eingeführt: kein Zeitzonen-Crate (z. B. `chrono-tz`) für
Europe/Berlin — die Retry-Uhrzeit wird in UTC (Containerzeit) gezeigt; kein
HTML-Escaping-Helper — Askamas `{{ }}`-Escaping in `data-clipboard` genügt.

## Bestehende Abhängigkeiten (Notation `<organization>/<project>`)

Laufzeit (aus `Cargo.toml`):

- `tokio/tokio`
- `tokio/axum`
- `launchbadge/sqlx`
- `plops/gemini-rust` (Crate `gemini-rust`)
- `serde-rs/serde`, `serde-rs/json`
- `askama-rs/askama`
- `rust-lang/regex`
- `tokio-rs/tracing`, `tokio-rs/tracing-subscriber`
- `dtolnay/thiserror`, `dtolnay/anyhow`
- `chronotope/chrono`
- `tokio/tower-http`
- `seanmonstar/reqwest`
- `rust-lang/futures-rs` (Crate `futures-util`)
- `floatdrop/vtt` (Crate `vtt`)
- `pulldown-cmark/pulldown-cmark`
- `openai-rs/async-openai` (Crate `async-openai`)

Entwicklung / optional:

- `rust-lang/proptest` (Crate `proptest`, dev)
- `jgrahamc/fantoccini` (Crate `fantoccini`, dev)
- `tokio/tokio-util` (dev)
- `Stebalien/tempfile` (Crate `tempfile`, dev)
- `tokio/tower` (dev)
- `plops/fast-umap` (gepatcht via `third_party/fast-umap`, optional hinter `nn-mapper`)
- `burn-rs/burn` (Crates `burn-autodiff`, `burn-cubecl`, optional)
- `cubecl/cubecl` (optional)

## DeepWiki-Abgleich

Abfrage zu `plops/rs-summarizer` (Struktur + Frage zu `/browse`,
`generation_partial`, `timestamp_linker`, Fehlerbehandlung) bestätigt:

- `src/routes/mod.rs::browse_summaries` rendert `BrowseTemplate`; Kosten aktuell via
  `"{:.6}"|format` in `templates/browse.html`;
- `templates/generation_partial.html` zeigt `next_retry_at` roh bei `retry_wait`;
- `replace_timestamps_in_html` wird bisher nur auf `timestamps_html` angewendet;
- Fehlerkette: `GenerationStatus` → `PublicErrorCode` → `mark_error` in `src/tasks.rs`.

Keine neue externe Dependency bedeutet: keine zusätzliche DeepWiki-Abfrage oder
Usage-Example zur globalen Dependency-Dokumentation.

## Containerprogramme

Keine neuen Programme erforderlich. Vorhandenes Rust/Cargo, SQLite via `sqlx`
sowie Python 3 (nur für DB-Inspektion während der Entwicklung) reichen aus.
