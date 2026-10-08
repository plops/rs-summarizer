# Implementierungsplan: auto → gemini-3.1-flash-lite + localStorage-Einstellungen

Datum: 2026-10-08 · Ordner: `plan/20261008_01_local_storage/` · Autor-Auftrag: wol pumba

## 1. Ziel

1. Die Modellwahl `auto` soll nicht mehr per Heuristik (Transkriptdauer /
   Wortzahl) zwischen Modellen unterscheiden, sondern **immer**
   `gemini-3.1-flash-lite` verwenden.
2. Ohne Nutzerverwaltung sollen Webseiten-Besucher ihre
   Parameterauswahl (KI-Modell, Thinking-Effort, Ausgabesprache, Glossar,
   Grounding, URL-Context) **im Browser-`localStorage`** speichern können.
   Beim nächsten Seitenaufruf werden die Einstellungen automatisch
   wiederhergestellt.

## 2. Kontext-Dateien (vom Agenten zu lesen)

| Datei | Warum lesen |
|---|---|
| `plan/20261008_01_local_storage/prompt.txt` | Aufgabenstellung (verbindlich). |
| `src/tasks.rs` (v. a. `run_model_pipeline`, `get_fallback_chain`, `get_transcript_duration_secs`, Tests ab ca. Zeile 890) | Enthält die `auto`-Heuristik, die ersetzt wird, sowie die Fallback-Ketten und bestehenden Tests. |
| `src/state.rs` (`ModelOption`, `get_default_models`, `load_models_config`) | Modelldefinitionen; `auto` ist hier als Pseudomodell registriert. |
| `templates/index.html` | Das Webformular (Modell, Thinking-Level, Ausgabesprache, Glossar, Grounding, URL-Context) inkl. bestehendem JS für Sichtbarkeitslogik — hier kommt die `localStorage`-Logik hinzu. |
| `src/models.rs` (`SubmitForm`, `ThinkingPreference`) | Formular-Datenmodell serverseitig; zeigt, welche Felder persistierbar sind. |
| `src/routes/mod.rs` (`index`, `process_transcript`) | Rendert das Formular; validiert Modell + Thinking-Level. |
| `src/templates.rs` (`IndexTemplate`) | Template-Bindung Modelliste → HTML. |
| `config/models.json` | Externe Modellkonfiguration (muss `gemini-3.1-flash-lite` enthalten — prüfen). |
| `extension/popup.js` | Referenz: Die Browser-Extension persistiert Einstellungen bereits via `chrome.storage.local` — Namens- und UX-Vorbild. |
| `tests/integration_ui.rs` | Bestehende UI-Integrationstests (Muster für neue Assertions auf gerendertes HTML) und Prod-DB-Regressionstest. |
| `Cargo.toml` | Version (`[package] version`) für den Release-Bump. |
| `scripts/release.sh`, `scripts/release-check.sh`, `.github/workflows/release.yml` | Release-Ablauf: Version → Commit → Tag `vX.Y.Z` → GitHub-Action baut Release. |

## 3. Umsetzungsschritte

### Schritt 1 — `auto` fixieren (Rust)

- In `src/tasks.rs::run_model_pipeline` den `if model_name == "auto"`-Block
  ersetzen: keine HN/YouTube-Verzweigung mehr, sondern direkt
  `model_name = "gemini-3.1-flash-lite"`.
- Empfehlung: als kleine Funktion `resolve_auto_model() -> &'static str`
  extrahieren (derzeit `&str`-Konstanten inline), damit sie direkt
  unit-testbar ist. `get_transcript_duration_secs` bleibt für mögliche
  spätere Nutzung erhalten (nur verwenden, wenn noch referenziert —
  sonst `#[allow(dead_code)]`/Entfernen je nach Clippy).
- Heuristik-Unit-Tests (`test_hn_model_auto_selection_thresholds`,
  `test_transcript_duration_auto_selection_thresholds`) auf die neue
  Funktion umstellen: sie müssen das *echte* Verhalten testen, nicht die
  Heuristik duplizieren.
- Fallback-Kette für `gemini-3.1-flash-lite` existiert bereits
  (`get_fallback_chain`) — prüfen, nicht duplizieren.
- `cargo test auto`, danach volles `cargo test`.

### Schritt 2 — `localStorage`-Persistenz (Template-JS)

- In `templates/index.html` im bestehenden `DOMContentLoaded`-Skript:
  - **Ein Key, ein JSON-Objekt**, z. B. `rs-summarizer:settings:v1` mit den
    Feldern `model`, `thinking_level`, `output_language`,
    `include_glossary`, `google_search_grounding`, `url_context`.
  - **Speichern:** bei `change`-Events der genannten Controls (nicht bei
    URL/Transkript-Eingaben — kein Nutzerinhalt, keine großen Texte).
    `try/catch` um `localStorage`-Zugriffe (Privatmodus kann werfen).
  - **Wiederherstellen:** beim Laden Werte einlesen, gegen vorhandene
    `<option>`/Controls validieren (unbekannte Modellnamen ignorieren),
    Checkboxen setzen. Danach die bestehende
    `updateOptionsVisibility()` aufrufen, damit Thinking-Level- und
    Grounding-Sichtbarkeit konsistent bleiben.
  - Reihenfolge beachten: erst Modell setzen, dann Sichtbarkeit
    aktualisieren, dann Thinking-Level setzen (nicht-unterstützte Werte
    fallen auf `auto` zurück).
- Kein Server-Code nötig; kein Cookie-Banner nötig (`localStorage` ist
  rein lokal, keine Übertragung an den Server).
- Kurzen Hinweistext im Formular ergänzen (z. B. „Einstellungen werden
  lokal in deinem Browser gespeichert.").

### Schritt 3 — Tests

- **Unit (Rust):** `resolve_auto_model` gibt immer
  `gemini-3.1-flash-lite` zurück (ggf. parametrisiert über HN/YT-Dummies).
- **Integration (Rust, `tests/integration_ui.rs`):** `GET /` rendert 200
  und das HTML enthält die Persistenz-Marker (`localStorage`,
  Settings-Key, alle Feld-IDs). Kein headless Browser nötig — prüfen,
  dass das Skript mit den richtigen Bezeichnern ausgeliefert wird.
- **Regression:** bestehenden Prod-DB-Test (`prod_copy_browse_and_generation_render`)
  mitlaufen lassen (wird ohne `summaries.db` übersprungen).
  Hinweis: Die in `prompt.txt` genannten Paket-/Echo-Datensätze stammen
  aus einer anderen Repo-Vorlage und existieren hier nicht; das
  Analogon ist der Prod-DB-Regressionstest.
- Gates: `cargo test`, `cargo clippy --all-targets -- -D warnings`,
  `cargo fmt --check`.

### Schritt 4 — Doku

- `walkthrough.md` (Deutsch, Mermaid, Code-Beispiele, siehe
  `prompt.txt`-Vorgaben) und `plan_effort.md` (Token-Verbrauch)
  in diesen Ordner legen.

### Schritt 5 — Version + Release-Commit

- Patch-Bump in `Cargo.toml` (z. B. `1.8.4` → `1.8.5`; Repo-Konvention:
  auch kleine `feat(ui)`-Änderungen gingen zuletzt als Patch).
- Conventional-Commit(s) nach Abschnitt 4; **kein Push durch den
  Agenten** — Push-/Tag-Kommandos dem Nutzer geben.

## 4. Commit-Message-Konvention

- Format: **Conventional Commits** — `<typ>(<scope>): <kurze Zeile>`,
  danach Leerzeile + ausführlicher Body (was, warum, Auswirkungen).
- Typen: `feat` (neues Verhalten), `fix`, `test`, `docs`, `refactor`,
  `chore`. Release-Commits heißen `Release vX.Y.Z` (Repo-Konvention,
  siehe `git log`).
- Scopes hier: `tasks`/`models` (auto-Fixierung), `ui` (localStorage),
  `plan` (Doku).
- Nur namentlich genannte Dateien stagen (`git add <datei>...`), kein
  `git add -A`. `Cargo.lock` ist per `.gitignore` ausgeschlossen und wird
  nicht committet.
- Beispiel:

```text
feat(ui): persist form settings in browser localStorage

Store model, thinking effort, output language, glossary and grounding
flags as JSON under rs-summarizer:settings:v1. Restore on page load
with validation against available options; no server round-trip and
no user accounts required.
```

## 5. Risiken / Nicht-Ziele

- `localStorage` ist domain-gebunden: Extension (`chrome.storage`) und
  Webseite teilen sich nichts — bewusst kein Ziel.
- Alte gespeicherte Modellnamen müssen tolerant ignoriert werden
  (Validierung gegen `<option>`-Liste).
- Keine Migration nötig (kein DB-Schema betroffen).
