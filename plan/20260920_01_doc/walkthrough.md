# Walkthrough: Architektur-Dokumentation rs-summarizer (`plan/20260920_01_doc`)

Datum: 2026-09-20 · Stand: `doc.md` + `prompt.txt` geschrieben, validiert und
committet; diese Datei schließt die Arbeit ab.

## Was passiert ist

1. **Fehlstart korrigiert:** Der vorherige Commit (`0ad2b79`) dokumentierte
   einen Treemap-Iterations-Vergleich aus einem Fremd-Repo — falsche Aufgabe.
   Nach Hinweis des Anwenders und neuem `prompt.txt` (Zeile 3: „Dokumentation
   des Codes in diesem Projekt, Architektur erklären") wurde `0ad2b79` per
   `git revert` verworfen (Commit `ed48417`), das neue `prompt.txt` blieb
   erhalten. Recovery-Ref der Sicherung:
   `refs/tbh/recovery/before-discard/20260920T125413Z-81051`.
2. **Code gelesen:** Einstieg (`main.rs`, `lib.rs`, `state.rs`), Pipeline
   (`tasks.rs` vollständig), Lebenszyklus (`generation.rs`, `db.rs`-CAS),
   Routen (`routes/mod.rs` vollständig), Services (summary, transcript,
   embedding, hacker_news, rate_limiter, deduplication) sowie Extension,
   Templates, Migrationen und `viz-tool/AGENTS.md` im Überblick.
3. **`doc.md` geschrieben** (445 Zeilen, 14 Sektionen, deutsch): Überblick,
   System-Mermaid, Start/`AppState`, Routen-Tabelle + HTMX-Polling,
   State-Machine mit Epochen-Schutz, Pipeline (Beschaffung/Auswahl/Limits/
   Erzeugung/Abschluss), Preis-Formel, Datenmodell, Templates/Extension,
   Kosinus-Suche + VizData, Betrieb, Tests, Deps-Tabelle, Vorschlag-Box.
4. **Validiert:** 5 Mermaid-Blöcke balanciert; `cargo test --offline`:
   150 Unit + 4 Integration grün, 0 Fehler (Browser-/Netz-Tests ignoriert —
   brauchen Chromium/WebDriver bzw. echte API-Schlüssel).

## Learnings

- Die Architektur folgt einer klaren Regel: **ein Modul, ein Thema, ein
  Außenkontakt** — `db.rs` hat alles SQL, `tasks.rs` die Orchestrierung,
  jeder Service genau einen Fremddienst. Das machte die Doku fast zu einer
  Inhaltsangabe der Modulliste.
- Der spannendste Mechanismus ist der **Epochen-Schutz** gegen Geister-Streams
  (`append_summary_chunk_for_epoch`) — eine Zeile SQL, die eine ganze Klasse
  von Race-Conditions ausschließt.
- Ehrliche Grenzen gehören in die Doku: linearer Embedding-Scan, fixe
  UTC-8-Näherung, fehlende Proxy-Vertrauensliste — alle mit Datei-Verweis,
  sodass ein Folge-Agent sie direkt findet.
- Prompt-Zeile 39 (Treemap/Benchmarks) ist ein Überbleibsel der alten Aufgabe
  und gilt nicht für diese Codebase (keine Benchmarks, keine Treemap hier) —
  bewusst nicht umgesetzt, hier dokumentiert statt still ignoriert.

## Quellen

- Code: `src/main.rs`, `src/lib.rs`, `src/state.rs`, `src/tasks.rs`,
  `src/generation.rs`, `src/db.rs`, `src/routes/mod.rs`, `src/models.rs`,
  `src/services/`, `src/utils/`, `src/templates.rs`, `src/cache.rs`,
  `src/errors.rs`, `src/commands/export_db.rs`, `migrations/001–009`,
  `config/models.json`, `templates/`, `extension/popup.js`,
  `rs-summarizer.service`, `viz-tool/AGENTS.md`, `viz-tool/deps.md`.
- Pläne/Specs: `.kiro/specs/rs-summarizer/`, `doc/rate_limiting_error_handling_architecture.md`.
- GitHub-Rendering: MathJax (`$…$`, `$$…$$`) und Mermaid
  (` ```mermaid `) laut Aufgabenstellung nativ unterstützt.
- Kein DeepWiki-MCP in dieser Session verfügbar — Recherche direkt an den
  Quellen; `doc.md` § 13 enthält fertige `org/projekt`-Paare für spätere
  DeepWiki-Abfragen.

## Neue Programme für den Docker-Container

Keine: Dokumentationsarbeit ohne Code-Änderung, keine Dependencies
eingeführt. Testläufe nutzten die vorhandene Toolchain (`cargo test
--offline` grün). Für Browser-Integrationstests wären zusätzlich Chromium +
Chromedriver nötig — nicht installiert, Tests bleiben ignoriert.
