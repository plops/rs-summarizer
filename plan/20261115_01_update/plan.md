# Implementierungsplan: Modell-Update, Architektur-Fixes, Titel-Backfill-Analyse, CHANGELOG & Release-Workflow

## 1. Ziel & Überblick

Dieses Dokument instruiert einen Implementierungs-Agenten, `rs-summarizer` (Produktivinstanz: `https://rocketrecap.com`) umfassend zu aktualisieren, definierte Architektur-Schwachstellen zu beheben, das Produktions-Backup bezüglich des Titel-Backfills auszuwerten, ein englischsprachiges `CHANGELOG.md` einzuführen, den GitHub-Release-Workflow anzupassen und die neue Version (`1.11.0`) nach dem aktualisierten Release-Protokoll zu veröffentlichen.

---

## 2. Rahmenbedingungen & strikte Regeln

* **Auftraggeber:** wol pumba (`wolpumba@gmail.com`)
* **Umgebung:** Ubuntu-Docker-Container (`/workspace/src/cl-cl-generator/example/05_dockerfile_meta/source01/examples/03_ai_env/Dockerfile`). Fehlende CLI-Pakete (z. B. `zstd`, `sqlite3`) selbstständig nachinstallieren und im Walkthrough Abschnitt 4 dokumentieren.
* **Sprache & Stil:** Idiomatisches Rust (Edition 2024).
* **Qualitätsgates (ohne Warnings!):**
  * `cargo fmt --check`
  * `cargo clippy --all-targets --all-features -- -D warnings`
  * `cargo test`
* **Abhängigkeiten:** Keine unnötigen neuen Crates einführen. Falls unumgänglich, `deps.md` in der Notation `<organization>/<project>` pflegen.
* **Kontext:** DeepWiki MCP `plops/rs-summarizer` falls verfügbar nutzen.
* **Geheimnisse & Backups (STRIKT!):**
  * `/workspace/src/rs-summarizer/summaries_20261115.db.zst` niemals verändern oder committen. Temporäre Dateien nach `/tmp/` entpacken und am Ende aufräumen.
  * `/workspace/src/rs-summarizer/hetzner_token` (ggf. auch `/workspace/src/rs-summarizer/hetzner_token` prüfen) enthält einen produktiven Hetzner-API-Key. Du darfst ihn **niemals** ausgeben, in Logs/Dateien/Plan/Walkthrough schreiben oder einchecken! Falls du ihn für einen Live-Smoke-Test gegen Hetzner nutzt, übergib ihn ausschließlich blind per Shell-Substitution:
    `HETZNER_API_KEY="$(cat /workspace/src/rs-summarizer/hetzner_token)" ...`
  * Den alten, ungültigen Fallback-Key in `src/services/summary.rs` stehen lassen (keine Git-History-Bereinigung).
* **Bewusst NICHT ändern:**
  * Keinen HTML-Sanitizer (`ammonia` o. ä.) für `|safe` einführen (Prototyp-Entscheidung).
  * Embedding-Modell auf `gemini-embedding-001` (3072 Dimensionen) belassen.
  * Kein SQLite-Chunk-Batching oder Neuarchitektur von `find_similar` vornehmen.
  * `extension/` nicht anfassen.

---

## 3. Pflicht-Vorab-Inspektion (vor der ersten Code-Änderung!)

1. **Submodule/Patches prüfen:** Falls `third_party/fast-umap` nicht gepatcht ist, `./scripts/apply_submodule_patches.sh` (mit `SKIP_COMMIT=1`) prüfen, damit `--all-features` kompiliert.
2. **`gemini-rust` Enum prüfen:** Suche im lokalen Cargo-Cache (`~/.cargo/registry/src/**/gemini-rust-2.0.0/src/`) nach `InteractionThinkingLevel` und `InteractionBuilder`:
   * Prüfe, ob `InteractionThinkingLevel` die Variante `XHigh` bereits hat oder nur `Minimal, Low, Medium, High`.
   * Prüfe, ob `InteractionBuilder` ein `raw`/JSON-Feld akzeptiert oder ein konkretes `CreateInteractionRequest`-Struct baut.
   * *Ergebnis bestimmt Schritt 6.3 (`xhigh`-Versand).*
3. **Produktions-Backup entpacken (read-only Inspektion):**
   ```bash
   zstd -d -c /workspace/src/rs-summarizer/summaries_20261115.db.zst > /tmp/summaries_20261115.db
   ```
   Prüfe exakt das Produktiv-Schema und Legacy-Spalten (wichtig für Migration `011`!):
   ```bash
   sqlite3 -readonly /tmp/summaries_20261115.db ".schema summaries"
   sqlite3 -readonly /tmp/summaries_20261115.db ".schema summary_ratings"
   sqlite3 -readonly /tmp/summaries_20261115.db "SELECT name, sql FROM sqlite_master WHERE type='index' AND tbl_name='summaries';"
   ```
4. **Git-Historie für `CHANGELOG.md`-Rückschau inspizieren:**
   ```bash
   git tag -l --sort=-v:refname
   git log --oneline --decorate -n 80
   ls -1 plan/
   ```
5. **Alle Vorkommen alter Modellnamen suchen:**
   ```bash
   rg -n "gemini-3\.7|gemini-3\.5-flash|gemini-2\.5-flash|gemini-3\.|startsWith\('gemini-3\.'\)|MetadataCache" src/ templates/ tests/ doc/ config/
   ```

---

## 4. Arbeitspaket 1: Backup-Auswertung (YouTube-Titel-Backfill)

Ziel: Prüfen, ob die Datenbank nach und nach mit YouTube-Videotiteln aufgefüllt wird (ältere Einträge hatten ursprünglich keine Titel).

Führe auf `/tmp/summaries_20261115.db` (`sqlite3 -readonly`) folgende Abfragen aus und protokolliere alle exakten Zahlen für `walkthrough.md` und die Abschlussantwort:

```sql
-- 1. Gesamtdeckung YouTube-Zeilen & eindeutige YouTube-URLs
SELECT
    COUNT(*) AS total_yt_rows,
    SUM(CASE WHEN COALESCE(title, '') != '' THEN 1 ELSE 0 END) AS titled_yt_rows,
    SUM(CASE WHEN COALESCE(title, '') = '' THEN 1 ELSE 0 END) AS untitled_yt_rows,
    ROUND(100.0 * SUM(CASE WHEN COALESCE(title, '') != '' THEN 1 ELSE 0 END) / COUNT(*), 2) AS pct_rows_titled,
    COUNT(DISTINCT original_source_link) AS distinct_yt_urls,
    COUNT(DISTINCT CASE WHEN COALESCE(title, '') != '' THEN original_source_link END) AS distinct_titled_yt_urls,
    COUNT(DISTINCT CASE WHEN COALESCE(title, '') = '' THEN original_source_link END) AS distinct_untitled_yt_urls
FROM summaries
WHERE original_source_link LIKE '%youtube.com%'
   OR original_source_link LIKE '%youtu.be%';

-- 2. Tagesverlauf aus title_fetch_budget (zeigt direkt den langsamen Backfill pro Tag)
SELECT day, source, count
FROM title_fetch_budget
ORDER BY day ASC, source ASC;

-- 3. Summen pro Quelle im Budget
SELECT
    source,
    COUNT(*) AS active_days,
    SUM(count) AS total_attempts,
    MIN(day) AS first_day,
    MAX(day) AS last_day
FROM title_fetch_budget
GROUP BY source;

-- 4. Verteilung betitelter vs. unbetitelter YouTube-Einträge nach Monat
-- (zeigt, ob alte Monate bereits schrittweise aufgefüllt werden)
SELECT
    substr(COALESCE(summary_timestamp_start, ''), 1, 7) AS month,
    COUNT(*) AS total_rows,
    SUM(CASE WHEN COALESCE(title, '') != '' THEN 1 ELSE 0 END) AS titled_rows,
    COUNT(DISTINCT CASE WHEN COALESCE(title, '') != '' THEN original_source_link END) AS distinct_titled_urls
FROM summaries
WHERE original_source_link LIKE '%youtube.com%'
   OR original_source_link LIKE '%youtu.be%'
GROUP BY month
ORDER BY month ASC;

-- 5. Aufschlüsselung nach rs_summarizer_version
-- (zeigt insbesondere betitelte Alt-Einträge mit leerer Version '' oder < 1.9.x!)
SELECT
    COALESCE(rs_summarizer_version, '') AS version,
    COUNT(*) AS total_yt_rows,
    SUM(CASE WHEN COALESCE(title, '') != '' THEN 1 ELSE 0 END) AS titled_yt_rows,
    COUNT(DISTINCT CASE WHEN COALESCE(title, '') != '' THEN original_source_link END) AS distinct_titled_yt_urls
FROM summaries
WHERE original_source_link LIKE '%youtube.com%'
   OR original_source_link LIKE '%youtu.be%'
GROUP BY version
ORDER BY version ASC;
```

Berechne daraus zusätzlich, wie viele Tage der Backfill beim Tageslimit (`100` YouTube-URLs/Tag) voraussichtlich noch für die verbleibenden `distinct_untitled_yt_urls` benötigt.

---

## 5. Arbeitspaket 2: Ziel-Spezifikation der Modelle, Thinking-Levels & Fallbacks

### 5.1 Exakte Modell-Konfiguration (`config/models.json` & `src/state.rs`)

Folgende Modelle müssen **genau** konfiguriert werden (veraltete `gemini-3.7-flash`, `gemini-3.5-flash` und `gemini-2.5-flash` werden komplett entfernt):

| Reihenfolge | `name` | Input ($/1M) | Output ($/1M) | `context_window` | `rpm_limit` | `tpm_limit` | `rpd_limit` | `architecture` | Thinking-Unterstützung |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :--- | :--- |
| 1 | `auto` | 0.0 | 0.0 | 1\_048\_576 | 15 | 250\_000 | 500 | `Gemini` | wie `gemini-3.5-flash-lite` (`minimal..high`, Default `high`) |
| 2 | `gemini-3.5-flash-lite` | 0.30 | 2.50 | 1\_048\_576 | 15 | 250\_000 | 500 | `Gemini` | `minimal, low, medium, high` (Default `high`) |
| 3 | `gemini-3.1-flash-lite` | 0.25 | 1.50 | 1\_048\_576 | 15 | 250\_000 | 500 | `Gemini` | `minimal, low, medium, high` (Default `high`) |
| 4 | `gemini-4-hydrogen` | 1.50 | 7.50 | 1\_048\_576 | 5 | 250\_000 | 20 | `Gemini` | `low, medium, high, xhigh` (**kein** `minimal`, Default `xhigh`) |
| 5 | `gemini-3.8-flash` | 0.75 | 3.75 | 1\_048\_576 | 5 | 250\_000 | 20 | `Gemini` | `low, medium, high` (**kein** `minimal`, Default `high`) |
| 6 | `gemini-3.6-flash` | 0.75 | 3.75 | 1\_048\_576 | 5 | 250\_000 | 20 | `Gemini` | `minimal, low, medium, high` (Default `high`) |
| 7 | `gemini-3-flash-preview` | 0.50 | 3.00 | 1\_048\_576 | 5 | 250\_000 | 20 | `Gemini` | `minimal, low, medium, high` (Default `high`) |
| 8 | `gemini-2.5-flash-lite` | 0.10 | 0.40 | 1\_048\_576 | 10 | 250\_000 | 20 | `Gemini` | nur `auto` (Interactions API lehnt `thinking_level` bei 2.5 ab) |
| 9 | `gemma-4-31b-it` | 0.07 | 0.34 | 262\_144 | 30 | 16\_000 | 14\_400 | `Gemma` | nur `auto` |
| 10 | `gemma-4-26b-a4b-it` | 0.07 | 0.34 | 262\_144 | 30 | 16\_000 | 14\_400 | `Gemma` | nur `auto` |
| 11 | `hetzner-qwen-3.8-27b` | 0.0 | 0.0 | 262\_144 | 60 | 0 | 14\_400 | `Hetzner` | nur `auto` |
| 12 | `hetzner-qwen-3.6-35b` | 0.0 | 0.0 | 262\_144 | 60 | 0 | 14\_400 | `Hetzner` | nur `auto` |

*Hinweis:* `ModelOption.tpm_limit: u32` mit `#[serde(default)]` versehen (`0` = kein TPM-Limit). `ModelOption::is_expensive()` bzw. klare Trennung zwischen teuren und günstigen Modellen über den Preis (oder explizit in den Fallback-Ketten) festlegen.

### 5.2 `AUTO_MODEL` & strikte Fallback-Regeln („teure Modelle nur explizit“)

* **`AUTO_MODEL`:** `"gemini-3.5-flash-lite"` (neuestes 500-RPD-Modell).
* **Invariante:** Ein günstiges/kostenloses Modell darf **niemals** auf ein teureres Modell zurückfallen. Kein Modell darf auf ein strikt teureres Modell zurückfallen. `gemma-4-*` steht wegen des engen 16K-TPM-Limits in **keiner** automatischen Fallback-Kette.
* **Fallback-Ketten in `src/tasks.rs` (`get_fallback_chain`):**
  * `gemini-4-hydrogen`: `["gemini-4-hydrogen", "gemini-3.8-flash", "gemini-3.6-flash", "gemini-3-flash-preview", "gemini-3.5-flash-lite", "gemini-3.1-flash-lite", "gemini-2.5-flash-lite", "hetzner-qwen-3.8-27b", "hetzner-qwen-3.6-35b"]`
  * `gemini-3.8-flash`: `["gemini-3.8-flash", "gemini-3.6-flash", "gemini-3-flash-preview", "gemini-3.5-flash-lite", "gemini-3.1-flash-lite", "gemini-2.5-flash-lite", "hetzner-qwen-3.8-27b", "hetzner-qwen-3.6-35b"]`
  * `gemini-3.6-flash`: `["gemini-3.6-flash", "gemini-3.8-flash", "gemini-3-flash-preview", "gemini-3.5-flash-lite", "gemini-3.1-flash-lite", "gemini-2.5-flash-lite", "hetzner-qwen-3.8-27b", "hetzner-qwen-3.6-35b"]`
  * `gemini-3-flash-preview`: `["gemini-3-flash-preview", "gemini-3.5-flash-lite", "gemini-3.1-flash-lite", "gemini-2.5-flash-lite", "hetzner-qwen-3.8-27b", "hetzner-qwen-3.6-35b"]`
  * `gemini-3.5-flash-lite`: `["gemini-3.5-flash-lite", "gemini-3.1-flash-lite", "gemini-2.5-flash-lite", "hetzner-qwen-3.8-27b", "hetzner-qwen-3.6-35b"]`
  * `gemini-3.1-flash-lite`: `["gemini-3.1-flash-lite", "gemini-3.5-flash-lite", "gemini-2.5-flash-lite", "hetzner-qwen-3.8-27b", "hetzner-qwen-3.6-35b"]`
  * `gemini-2.5-flash-lite`: `["gemini-2.5-flash-lite", "gemini-3.5-flash-lite", "gemini-3.1-flash-lite", "hetzner-qwen-3.8-27b", "hetzner-qwen-3.6-35b"]`
  * `hetzner-qwen-3.8-27b`: `["hetzner-qwen-3.8-27b", "hetzner-qwen-3.6-35b", "gemini-3.5-flash-lite", "gemini-3.1-flash-lite", "gemini-2.5-flash-lite"]`
  * `hetzner-qwen-3.6-35b`: `["hetzner-qwen-3.6-35b", "hetzner-qwen-3.8-27b", "gemini-3.5-flash-lite", "gemini-3.1-flash-lite", "gemini-2.5-flash-lite"]`
  * `gemma-4-31b-it`, `gemma-4-26b-a4b-it`, sonstige: `vec![other]`

---

## 6. Beteiligte Dateien & geplante Änderungen im Detail

### 6.1 `config/models.json` und `src/state.rs`
* `ModelOption` um `pub tpm_limit: u32` (`#[serde(default)]`) erweitern.
* Methoden / Capability-Helpers auf `ModelOption` (oder freie Funktionen in `state.rs` / `models.rs`) definieren, um String-Prefix-Raten wie `starts_with("gemini-3.")` komplett abzuschaffen:
  * `supported_thinking_levels(model_name: &str) -> &'static [ThinkingPreference]`
    * `"gemini-4-hydrogen"` $\rightarrow$ `&[Auto, Low, Medium, High, XHigh]`
    * `"gemini-3.8-flash"` $\rightarrow$ `&[Auto, Low, Medium, High]`
    * `"gemini-3.6-flash" | "gemini-3.5-flash-lite" | "gemini-3.1-flash-lite" | "gemini-3-flash-preview"` $\rightarrow$ `&[Auto, Minimal, Low, Medium, High]`
    * `"auto"` $\rightarrow$ delegiert an `AUTO_MODEL` (`"gemini-3.5-flash-lite"`)
    * alle übrigen (`"gemini-2.5-flash-lite"`, `"gemma-4-*"`, `"hetzner-*"`) $\rightarrow$ `&[Auto]`
  * `default_thinking_level(model_name: &str) -> ThinkingPreference`: gibt immer das **höchste** unterstützte Level zurück (`XHigh` für `gemini-4-hydrogen`, `High` für Gemini 3.x / `auto`, `Auto` für alle übrigen).
* `get_default_models()` und `config/models.json` synchron gemäß Tabelle in Abschnitt 5.1 aktualisieren.
* Tests in `src/state.rs` auf neue Modelle, `tpm_limit` und entfernte Alt-Modelle aktualisieren.

### 6.2 `src/models.rs` und `migrations/011_update_thinking_and_rpd_usage.sql`
* **`src/models.rs`:**
  * `ThinkingPreference` um `XHigh` (`"xhigh"`) erweitern; `ALL`, `as_str()`, `FromStr` entsprechend ergänzen.
  * `SubmitForm`: `thinking_level` Default bleibt `High` (oder `serde(default)`), UI sendet explizit das gewählte/höchste Level.
* **`migrations/011_update_thinking_and_rpd_usage.sql` (NEU):**
  * **KRITISCH:** Erste Zeile muss `-- no-transaction` sein (damit `PRAGMA foreign_keys = OFF;` in `sqlx 0.9` außerhalb der Transaktion greift und `DROP TABLE summaries` **nicht** via `ON DELETE CASCADE` alle Einträge in `summary_ratings` löscht!).
  * **KRITISCH:** Inspiziere zuvor `.schema summaries` aus `/tmp/summaries_20261115.db`! In produktiven Alt-Datenbanken können frühe Spalten `NULL` enthalten. Verwende beim Kopieren `INSERT INTO summaries_new (...) SELECT COALESCE(...), ... FROM summaries;` exakt für alle bestehenden Spalten der Tabelle.
  * Ablauf in Migration `011`:
    1. `-- no-transaction`
    2. `PRAGMA foreign_keys = OFF;`
    3. `BEGIN IMMEDIATE;`
    4. `CREATE TABLE summaries_new (...)` mit vollständigem aktuellem Schema (Spalten aus `001`..`010`) und erweitertem Constraint:
       `CHECK (thinking_level IN ('auto', 'minimal', 'low', 'medium', 'high', 'xhigh'))`
    5. `INSERT INTO summaries_new (...) SELECT ... FROM summaries;` (mit `COALESCE` für Spalten, die im Produktiv-Backup NULL enthalten könnten!)
    6. `DROP TABLE summaries;`
    7. `ALTER TABLE summaries_new RENAME TO summaries;`
    8. Alle Indizes neu anlegen (`idx_dedup_lookup`, `idx_summaries_model_time`, `idx_summaries_lang_glossary`, `idx_summaries_generation_recovery`) plus einen neuen Index für den Backfill-Scan:
       `CREATE INDEX IF NOT EXISTS idx_summaries_title_url ON summaries (title, original_source_link);`
    9. Neue Tabelle für persistente tägliche RPD-Zähler:
       ```sql
       CREATE TABLE IF NOT EXISTS model_daily_usage (
           day TEXT NOT NULL,
           model TEXT NOT NULL,
           count INTEGER NOT NULL DEFAULT 0,
           PRIMARY KEY (day, model)
       );
       ```
    10. `COMMIT;`
    11. `PRAGMA foreign_key_check;`
    12. `PRAGMA foreign_keys = ON;`
  * **Pflicht-Test am echten Backup:** Kopiere `/tmp/summaries_20261115.db` nach `/tmp/summaries_mig_test.db` und prüfe, dass `sqlx::migrate!("./migrations")` ohne Fehler durchläuft, `summaries`- und `summary_ratings`-Zeilenanzahl exakt gleich bleiben und `PRAGMA integrity_check;` sowie `PRAGMA foreign_key_check;` sauber sind. Danach `/tmp/summaries_mig_test.db` löschen.

### 6.3 `src/services/summary.rs` (Thinking-Capabilities, `xhigh`, Kosten & Hetzner-Härtung)
* **Thinking-Level-Mapping (`effective_thinking_level`):**
  * Ersetze `gemini_3_thinking_level` durch eine Capability-gestützte Funktion auf Basis der Tabelle in Abschnitt 5.1:
    * Wenn Modell nur `Auto` unterstützt (z. B. `gemini-2.5-flash-lite`, `gemma-4-*`, `hetzner-*`): `None` (kein Parameter).
    * Bei `ThinkingPreference::Auto`: `None` (Provider-Default).
    * Bei `ThinkingPreference::XHigh`:
      * Für `gemini-4-hydrogen`: `XHigh` senden.
      * Für Gemini 3.x (`3.8`, `3.6`, `3.5-flash-lite`, `3.1-flash-lite`, `3-flash-preview`): auf **`High` clampen** (verhindert HTTP 400 bei Fallback von `4-hydrogen` auf `3.x`!).
    * Bei `ThinkingPreference::Minimal`:
      * Für `3.6-flash`, `3.5-flash-lite`, `3.1-flash-lite`, `3-flash-preview`: echtes `InteractionThinkingLevel::Minimal` senden.
      * Für `gemini-4-hydrogen` und `gemini-3.8-flash`: auf **`Low` clampen** (verhindert HTTP 400!).
    * `Low`, `Medium`, `High`: direkt an Gemini 3.x / 4 senden.
* **Versand von `xhigh` an Google Interactions API (abhängig von Vorab-Inspektion 3.2):**
  * *Fall A:* `InteractionThinkingLevel` in `gemini-rust` besitzt bereits `XHigh` $\rightarrow$ direkt `.with_thinking_level(InteractionThinkingLevel::XHigh)` aufrufen.
  * *Fall B (wahrscheinlich bei `gemini-rust 2.0.0`):* `InteractionThinkingLevel` kennt kein `XHigh`. **Keine neue Crate hinzufügen!** Nutze den bereits vorhandenen `reqwest::Client` (`Cargo.toml` hat `reqwest` mit `json` und `futures-util`) für den SSE-Call an `https://generativelanguage.googleapis.com/v1beta/interactions?alt=sse` (Header `x-goog-api-key`, `Content-Type: application/json`), wenn `effective_level == XHigh`:
    * Payload-JSON analog zu `CreateInteractionBuilder`: `model`, `input`, `system_instruction`, `stream: true`, `generation_config: { "thinking_level": "xhigh", "thinking_summaries": "auto" }`, plus optional `tools` (`[{"type": "google_search"}]`, `[{"type": "url_context"}]`).
    * Parse jede `data: {...}`-SSE-Zeile direkt in `gemini_rust::prelude::InteractionEvent` via `serde_json::from_str` und leite sie in denselben `InteractionAccumulator` + `db::append_summary_chunk_for_epoch`-Loop!
* **Kosten-Prüfung (`compute_cost`):**
  * Prüfe in `gemini-rust` / API-Usage-Daten, ob `usage.total_output_tokens` in der Interactions API bereits `total_thought_tokens` enthält (laut Google-Doku sind Output-Preise inkl. Thinking-Tokens). Falls `total_output_tokens` Thinking-Tokens *nicht* mitzählt, dokumentiere das und berechne sie korrekt ein; falls sie bereits enthalten sind, belasse es und dokumentiere das in `walkthrough.md`.
* **Hetzner-Streaming härten (`generate_summary_hetzner`):**
  * `epoch` aus DB laden (`db::fetch_summary`) und `db::update_summary_chunk` vollständig durch `db::append_summary_chunk_for_epoch(db_pool, identifier, epoch, ...)` ersetzen (bei `Ok(false)` sauber mit `"generation ownership lost"` abbrechen).
  * `finish_reason` tracken: falls der SSE-Stream abbricht, ohne dass ein `choice.finish_reason.is_some()` gesehen wurde, Fehler zurückgeben statt einen abgeschnittenen Entwurf als vollständig zu markieren.
  * Qwen-3.6/3.8-Thinking-Output separieren: Falls der OpenAI-kompatible Hetzner-Stream `reasoning_content` / `reasoning` im Delta liefert oder `<think>...</think>` am Anfang von `content` ausgibt, sammle diesen Teil für `thinking_text` und schreibe nur die eigentliche Zusammenfassung in die `summary`-Chunks (damit kein roher `<think>`-Block im HTML landet).
  * In `resolve_hetzner_model_name`: `"hetzner-qwen-3.6-35b" => "Qwen/Qwen3.6-35B-A3B-FP8"`, `"hetzner-qwen-3.8-27b" => "Qwen/Qwen3.8-27B"` (oder `"Qwen3.8-27B"`, beide in `/v1/models` vorhanden; bevorzuge das kanonische `"Qwen/Qwen3.8-27B"` und prüfe gegen `/v1/models`-Liste).

### 6.4 `src/services/rate_limiter.rs` & `src/db.rs` (Persistentes RPD, DST-Fix & TPM-Steuerung)
* **`today_la()` DST-Fix:**
  * Berechne in `today_la()` die echte US-Pacific-Übergangszeit (`02:00` Ortszeit am 2. Sonntag im März = `10:00 UTC`, und `02:00` Ortszeit am 1. Sonntag im November = `09:00 UTC`), statt nur auf dem UTC-Datum zu vergleichen.
* **Persistentes RPD in SQLite (`model_daily_usage`):**
  * Beim Serverstart (oder über `RateLimiter`-Methoden mit `&SqlitePool`): `model_counts` aus `model_daily_usage` für `today_la()` laden, oder `check_rate_limit` / `increment_counter` direkt write-through gegen SQLite + In-Memory-Cache ausführen, damit ein Server-Neustart die Tagesquoten nicht zurücksetzt.
  * Atomarität: Stelle sicher, dass beim Tageswechsel (`today_la()`) sowohl In-Memory-Map als auch SQLite-Abfrage konsistent den neuen Tag nutzen.
* **TPM-Schutz (`tpm_limit`):**
  * Führe in `RateLimiter` (oder `AppState`) ein Sliding-Window (letzte 60 Sekunden) pro Modell für geschätzte Tokens `(estimated_input_tokens + reserve)` ein:
    * Schätze die Prompt-Größe konservativ (`prompt.len() / 4` bzw. `chars / 3` + System/FewShot-Overhead).
    * Falls die geschätzte Request-Größe bereits **über dem gesamten `tpm_limit`** des gewählten Modells liegt (z. B. 25.000-Token-Transkript auf `gemma-4-31b-it` mit nur `16_000 TPM`), brich sofort mit einer klaren `SummaryError`-Fehlermeldung ab (und auf Deutsch in `user_facing_error_message`: dass das Transkript das Minuten-Token-Limit dieses Modells überschreitet).
    * Falls das 60s-Fenster temporär ausgelastet ist, warte vor dem API-Call, bis wieder genug TPM-Budget im 60s-Fenster frei ist (oder begrenze sauber, sodass kein 429 provoziert wird).

### 6.5 `src/routes/mod.rs`, `src/tasks.rs` & `templates/index.html`
* **`src/routes/mod.rs`:**
  * `supports_named_thinking_levels(model_name)` und `validate_thinking_preference(model_name, preference)` auf `supported_thinking_levels(model_name)` umstellen (damit funktionieren auch `"auto"`, `"gemini-3-flash-preview"` und `"gemini-4-hydrogen"` inkl. `xhigh`!).
  * In `process_transcript`:
    * `auto` zuerst auf `AUTO_MODEL` (`"gemini-3.5-flash-lite"`) auflösen, bevor die Quota geprüft wird.
    * `RateLimiter::check_chain_available(...)`: **Nur ablehnen, wenn alle Modelle in `get_fallback_chain(effective_model)` ihr RPD erschöpft haben** — nicht mehr beim ersten erschöpften Primärmodell abbrechen!
* **`src/tasks.rs`:**
  * `AUTO_MODEL = "gemini-3.5-flash-lite"`.
  * `get_fallback_chain` exakt nach Abschnitt 5.2 implementieren.
  * `run_model_pipeline`: beim Durchlaufen der `fallback_chain` zusätzlich prüfen, ob das Kandidaten-Modell das TPM-Limit (sofern > 0) für den konkreten Prompt grundsätzlich einhalten kann, sonst direkt nächstes Fallback-Modell probieren.
  * `user_facing_error_message`: verständliche deutsche Fehlermeldung ergänzen, falls ein Prompt das TPM-Limit sprengt.
* **`templates/index.html`:**
  * Dropdown-Text für `auto`: `Auto ({{ auto_model }})` (zeigt nun automatisch `Auto (gemini-3.5-flash-lite)`).
  * `<option>`-Tags im `#model`-Select mit Attributen aus `ModelOption` ausstatten, z. B.:
    `data-thinking-levels="auto,low,medium,high,xhigh"` und `data-default-thinking="xhigh"` (oder über Askama-Methoden auf `ModelOption`).
  * Im `<select id="thinking_level">`: Option `<option value="xhigh">Extra High (xhigh)</option>` hinzufügen.
  * In JS (`updateOptionsVisibility` + `change`-Handler auf `#model`):
    * Unterstützte Optionen anhand `data-thinking-levels` aktivieren/deaktivieren (kein `startsWith('gemini-3.')` mehr!).
    * Bei erstmaligem Laden ohne gespeicherten Wert **sowie beim expliziten Wechsel des Modells** per `modelSelect.addEventListener('change', ...)` immer automatisch auf `data-default-thinking` (das **höchste** unterstützte Level dieses Modells) setzen!
    * Beim Laden aus `localStorage` sicherstellen, dass ein gespeichertes Thinking-Level nur übernommen wird, wenn es für das wiederhergestellte Modell gültig ist; sonst auf `data-default-thinking` zurückfallen.

### 6.6 `src/services/hacker_news.rs` (SSRF-Schutz & Größenlimit)
* In `HackerNewsService::fetch_external_article(url)`:
  1. **Schema-Prüfung:** Nur `http://` und `https://` erlauben (`reqwest::Url::parse`).
  2. **Redirect-Policy:** `reqwest::redirect::Policy::none()` im Artikel-Client (oder Custom-Policy mit erneuter IP-Prüfung; noch sauberer: kleine Redirect-Schleife bis max. 5 Hops, bei jedem Hop Hostname auflösen und prüfen!).
  3. **IP-/SSRF-Validierung:** Vor jedem Request `tokio::net::lookup_host` ausführen und **alle** aufgelösten `IpAddr` prüfen — abweisen wenn Loopback (`127.0.0.0/8`, `::1`), Private (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), Link-Local (`169.254.0.0/16`, `fe80::/10`), Unspecified (`0.0.0.0`, `::`), Multicast oder IPv6 Unique Local (`fc00::/7`) / CGNAT (`100.64.0.0/10`)! (Mit `resolve(...)` oder Prüfung direkt vor Request).
  4. **Download-Größenlimit:** `Content-Length > 5 * 1024 * 1024` sofort abweisen und den Body chunkweise (`res.chunk().await`) auf maximal 5 MiB begrenzen, bevor UTF-8 dekodiert wird.
  5. Unit-Tests für blockierte private/Loopback-IPs, falsches Schema (`file://`) und akzeptierte öffentliche URLs ergänzen.

### 6.7 `src/services/download_limiter.rs`, `src/services/transcript.rs` & `RELEASE_README.md` (`yt-dlp` seriell + Timeout)
* **`src/services/download_limiter.rs`:**
  * Default für `MAX_CONCURRENT_YTDLP` von `2` auf **`1`** ändern (strikt seriell — verhindert zuverlässig das Einfrieren des Servers durch parallele `uvx yt-dlp`-Prozesse).
  * Default für `YTDLP_DELAY_MS` von `1000` auf **`2000`** erhöhen (Rate-Limit zwischen aufeinanderfolgenden `yt-dlp`-Läufen).
  * Bestehende Tests in `download_limiter.rs` entsprechend prüfen/anpassen.
* **`src/services/transcript.rs`:**
  * Alle `Command::new("uvx")`-Aufrufe (`list_subtitles`, `download_subtitles`, `fetch_title`) über eine kleine Hilfsfunktion ausführen:
    * `.kill_on_drop(true)` setzen, damit abgebrochene Futures keine Zombie-`uvx`/`yt-dlp`-Prozesse hinterlassen.
    * `tokio::time::timeout(Duration::from_secs(90), cmd.output())` herumlegen (konfigurierbar über `YTDLP_TIMEOUT_SECS`, Default `90`), bei Ablauf `Err(TranscriptError::Timeout(timeout_secs))` zurückgeben!
* **`RELEASE_README.md`:**
  * Default von `MAX_CONCURRENT_YTDLP` in der Tabelle auf `1` aktualisieren und `YTDLP_DELAY_MS` / `YTDLP_TIMEOUT_SECS` dokumentieren.

### 6.8 `src/cache.rs`, `src/lib.rs` & `doc/rate_limiting_error_handling_architecture.md` (Aufräumen & Doku)
* `src/cache.rs` löschen und `pub mod cache;` aus `src/lib.rs` entfernen (toter Code).
* `doc/rate_limiting_error_handling_architecture.md` komplett auf die tatsächliche Architektur umschreiben:
  * `AUTO_MODEL = "gemini-3.5-flash-lite"` (keine Videolängen-Heuristik).
  * Explizite Trennung günstiger vs. teurer Modelle (keine stille Eskalation).
  * Persistente RPD-Zähler in SQLite (`model_daily_usage`), exakte DST-Berechnung (`today_la()`), TPM-Prüfung (60s-Fenster), RPM-Cooldown-Locks.
  * Serielles `yt-dlp` (`MAX_CONCURRENT_YTDLP = 1`, Cooldown, 90s Timeout).
  * `GenerationStatus`-Lifecycle mit endlichem Retry (`MAX_RETRY_ATTEMPTS = 3`, exponentielles `retry_delay`) und verständlichen deutschen Fehlermeldungen in der UI.

---

## 7. Arbeitspaket 3: `CHANGELOG.md` & Release-Workflow

### 7.1 `CHANGELOG.md` (NEU im Repository-Root)
* Format: **Keep a Changelog** auf **Englisch**, SemVer-kompatibel.
* Enthält:
  1. `## [Unreleased]` (leer oder für kommende Änderungen vorbereitet)
  2. `## [1.11.0] - 2026-11-15` mit strukturierten Unterpunkten (`Added`, `Changed`, `Fixed`, `Removed`, `Security`) für alle Änderungen dieses Updates.
  3. **Rückschau** für die bisherigen Releases (rekonstruiert aus `git tag` + `git log` + `plan/`-Ordnern, mindestens ab `1.7.x`/`1.8.0` bis `1.10.1` — alle tatsächlich in Git vorhandenen Versionstags sauber abdecken!).
  4. Unten Vergleichslinks (`[1.11.0]: https://github.com/plops/rs-summarizer/compare/v1.10.1...v1.11.0` usw.).

### 7.2 `scripts/extract-changelog.sh` (NEU, ausführbar `chmod +x`)
* Nimmt eine Version oder ein Tag entgegen (`$1`, z. B. `v1.11.0` oder `1.11.0`, streift führendes `v` ab).
* Extrahiert den Abschnitt zwischen `^## \[<VERSION>\]` (nicht die Headerzeile selbst, oder mit sauberer Überschrift `## What's Changed in v<VERSION>`) bis zur nächsten `^## \[`- oder `^\[.*\]: `-Zeile aus `CHANGELOG.md`.
* Bricht mit Exit-Code `1` und Fehlermeldung auf `stderr` ab, wenn der Abschnitt fehlt oder leer ist!
* *Hinweis:* Schreibe das Skript mit reinem POSIX/bash + `awk` (keine exotischen Tools nötig).

### 7.3 `scripts/release-check.sh` & `scripts/release.sh`
* **`scripts/release-check.sh`:**
  * Prüfe `cargo fmt --check`.
  * Prüfe `cargo clippy --all-targets --all-features -- -D warnings`.
  * Prüfe `cargo test`.
  * Prüfe, dass `CHANGELOG.md` vorhanden ist und nicht leer ist.
* **`scripts/release.sh`:**
  * Prüfe vor dem Bump/Commit, dass `CHANGELOG.md` entweder bereits einen Eintrag `## [<VERSION>]` hat oder genau dann, wenn `## [Unreleased]` nicht-leeren Inhalt hat, diesen automatisch in `## [<VERSION>] - $(date -u +%Y-%m-%d)` umbenennt und einen frischen leeren `## [Unreleased]`-Block darüber einfügt (und ggf. den `[<VERSION>]`-Compare-Link ergänzt).
  * Validiere direkt danach mittels `./scripts/extract-changelog.sh "$VERSION" > /dev/null`, dass Release-Notes extrahierbar sind!
  * Füge `CHANGELOG.md` zu `git add Cargo.toml Cargo.lock CHANGELOG.md` hinzu.
  * **Conventional-Commit-Format** für den Release-Commit verwenden:
    ```bash
    git commit -m "chore(release): release $TAG" -m "Bump version to $VERSION and finalize CHANGELOG.md for $TAG."
    ```

### 7.4 `.github/workflows/release.yml`
* Vor dem Step `Upload assets to release` einen Step einfügen:
  ```yaml
  - name: Extract release notes from CHANGELOG.md
    shell: bash
    run: |
      set -euo pipefail
      chmod +x ./scripts/extract-changelog.sh
      ./scripts/extract-changelog.sh "${{ github.ref_name }}" > RELEASE_NOTES.md
      cat RELEASE_NOTES.md
  ```
* Im Step `Upload assets to release` (`softprops/action-gh-release@v2`):
  * Ergänze `name: "rs-summarizer ${{ github.ref_name }}"`
  * Ergänze `body_path: RELEASE_NOTES.md`
  *(GitHub zeigt den Inhalt von `body_path` direkt **über den Release-Artefakten** an.)*
* Im Step `Package assets`:
  * Migration-Guard ergänzen: zusätzlich zu `006_add_thinking_level.sql` auch `010_add_title.sql` und `011_update_thinking_and_rpd_usage.sql` prüfen (sowohl in `staging/migrations/` als auch via `tar tzf "$tar_name"`).
  * Optional `CHANGELOG.md` ebenfalls ins `staging/`-Archiv kopieren.

### 7.5 Release-Protokoll dokumentieren
* Dokumentiere den kompletten Release-Prozess (Pflege von `CHANGELOG.md`, `./scripts/release-check.sh`, `./scripts/release.sh <version>`, automatische Release-Notes-Extraktion über den Artefakten in `.github/workflows/release.yml`) verständlich in `README.md` (neuer Abschnitt `## Release process`).

---

## 8. Verifikationsplan & Tests

1. **Formatierung & Lints:**
   ```bash
   cargo fmt --check
   cargo clippy --all-targets --all-features -- -D warnings
   ```
2. **Unit- & Integrationstests:**
   * `cargo test` (alle nicht-ignorierten Tests müssen grün sein).
   * Neue/angepasste Tests insbesondere für:
     * `config/models.json` vs. `get_default_models()` Synchronität & keine deprecated Modelle
     * Fallback-Ketten-Invariante: kein günstiges Modell fällt jemals auf ein teures Modell zurück
     * Thinking-Level-Matrix (`4-hydrogen` mit `xhigh` + kein `minimal`; `3.8-flash` ohne `minimal`; `3.6/3.5-lite/3.1-lite/3-flash-preview` mit `minimal..high`; `2.5-lite/gemma-4/hetzner` nur `auto`; Clamping beim Fallback)
     * `templates/index.html` UI-Defaults & `data-default-thinking` / `Auto (gemini-3.5-flash-lite)`
     * Migration `011` inkl. Test an einem Schema mit `summary_ratings` + Legacy-NULLs + anschließender Test auf Kopie `/tmp/summaries_mig_test.db` des realen Produktiv-Backups!
     * `RateLimiter`: Persistenz über SQLite, DST-Testfälle für `today_la()`, TPM-Limit-Prüfung (16K TPM für Gemma 4)
     * `HackerNewsService`: SSRF-Blocklist (IPv4 loopback/private/link-local, IPv6 `::1`/`fc00::`/`fe80::`, falsches Schema)
     * `DownloadLimiter`: Standardmäßig `MAX_CONCURRENT_YTDLP = 1`
     * `scripts/extract-changelog.sh 1.11.0` liefert gültige Release-Notes; Fehlerfall bei unbekannter Version liefert Exit != 0
3. **Optionale Live-Smoke-Tests (falls Keys vorhanden, Ergebnis im Walkthrough vermerken):**
   * Hetzner-Test (ohne den Key je auszulesen oder zu loggen!):
     `HETZNER_API_KEY="$(cat /workspace/src/rs-summarizer/hetzner_token)" ...` prüfen, ob `Qwen/Qwen3.6-35B-A3B-FP8` und `Qwen/Qwen3.8-27B` sauber streamen und `<think>` korrekt getrennt wird.
   * Gemini-API-Test (falls `GEMINI_API_KEY` gesetzt): Verifizierung mit `gemini-4-hydrogen` (`xhigh`) und `gemini-3.5-flash-lite`.
4. **Aufräumen:**
   * `/tmp/summaries_20261115.db` und `/tmp/summaries_mig_test.db` nach Abschluss löschen.
   * `git status` prüfen: keine `.db`, `.db.zst`, `RELEASE_NOTES.md`, `hetzner_token` oder sonstigen Artefakte im Arbeitsverzeichnis!

---

## 9. Git-Commit- & Release-Strategie

Alle Commits müssen **Conventional Commits** mit aussagekräftigem Body sein (kein `Release vX.Y.Z` ohne Typ!). Teile die Arbeit sauber in logische Commits auf, z. B.:

1. `feat(models): update model catalog to Gemini 4 Hydrogen and remove deprecated models`
2. `feat(thinking): add xhigh support, per-model capabilities, and safe migration 011`
3. `fix(rate-limit): persist daily RPD usage, enforce TPM limits, and preserve fallback chains`
4. `fix(security,streaming): harden HN fetcher against SSRF, guard Hetzner chunks by epoch, and serialize yt-dlp`
5. `docs(architecture): remove dead MetadataCache and update rate-limiting documentation`
6. `chore(release): add CHANGELOG.md and render release notes above GitHub release artifacts`
7. `docs(plan): add walkthrough and effort report for 20261115 update` (oder vor dem Release-Commit, sodass auch die Doku im Tag steckt!)

**Veröffentlichung auf GitHub:**
1. Stelle sicher, dass `CHANGELOG.md` den Abschnitt `## [1.11.0] - 2026-11-15` vollständig enthält (und `plan/20261115_01_update/walkthrough.md` + `plan_effort.md` bereits committed sind!).
2. Führe `./scripts/release-check.sh` aus — alle Checks müssen grün (`✓`) sein.
3. Führe `./scripts/release.sh 1.11.0` aus, um Version `1.11.0` in `Cargo.toml`/`Cargo.lock` zu bumpen, Tag `v1.11.0` zu erstellen und `main` + `v1.11.0` nach `origin` zu pushen.
4. Verifiziere nach dem Push (z. B. `git ls-remote origin main refs/tags/v1.11.0` und falls `gh` verfügbar `gh run watch` / `gh release view v1.11.0`), dass der GitHub-Actions-Build durchläuft und die Release-Notes über den Artefakten erscheinen.

---

## 10. Walkthrough & Aufwand (Pflicht-Abschlussdokumente)

### 10.1 Walkthrough unter `plan/20261115_01_update/walkthrough.md`
(Achtung: **Nicht** in `plan/20261024_01_titles/walkthrough.md` schreiben!)

Strikte Regeln:
* **Sprache & Stil:** Zwingend auf Deutsch, verständlich, didaktisch hochwertig, flüssig lesbar; Fachbegriffe kurz erklären.
* **Visualisierung:** Code-Beispiele und **Mermaid-Diagramme** (z. B. Fallback- & Thinking-Level-Clamping-Fluss, Rate-Limiter-Architektur mit SQLite/TPM, sowie Release-Pipeline `release.sh` $\rightarrow$ `extract-changelog.sh` $\rightarrow$ GitHub Release).
* **Inhaltliche Struktur:**
  1. Was exakt implementiert und behoben wurde — **inklusive der vollständigen Auswertung der nachgetragenen YouTube-Videotitel aus dem Backup `summaries_20261115.db.zst`** (Anteil betitelter Alt-Zeilen, Tagesverlauf aus `title_fetch_budget`, Einschätzung ob die Datenbank wie geplant langsam aufgefüllt wird).
  2. Architektur- und Designentscheidungen (u. a. warum günstige Modelle nicht mehr auf teure eskalieren, wie `xhigh` trotz `gemini-rust 2.0.0` gelöst wurde, warum `PRAGMA foreign_keys = OFF` + `-- no-transaction` bei Migration `011` nötig war).
  3. Learnings und Vorschläge für zukünftige Erweiterungen (z. B. spätere Einführung eines HTML-Sanitizers vor Produktivöffnung für unbekannte Nutzer, Embedding-Reindexing, `extension/`-Update).
  4. Liste eventuell neu benötigter Programme/Pakete für das Dockerfile.

### 10.2 Aufwandsdokumentation unter `plan/20261115_01_update/plan_effort.md`
Halte den Token-Verbrauch und Aufwand der Session transparent fest (Input/Output/Cached/Total Tokens, Turns, Subagents, Laufzeit).
