# Walkthrough: Technische Dokumentation (`plan/20260920_01_doc`)

Datum: 2026-09-20 · Stand: `doc.md`, `prompt.txt`, `deps.md`, `plan.md`,
`task.md` geschrieben und validiert; diese Datei schließt die Arbeit ab.

## Experimente

1. **Repo-Kartierung:** Die dokumentierten Programme liegen im Schwester-Repo
   `/workspace/src/transpiled_treemap/` (`iter1/`, `iter2/` mit je einem
   Rust-Programm, einem direkten C++-Port und einem Lisp-transpilierten
   C++-Generat). Dateilisten per `find` erhoben, keine C++-/Treemap-Quellen
   in `rs-summarizer` selbst (Verwechslungsgefahr `viz-tool`/Embedding-Umap
   ausgeschlossen).
2. **Basis-Übernahme mit Verifikation:** Eine inhaltsgleiche Schwester-Doku
   (`transpiled_treemap/plan/20260920_02_doc/doc.md`, 609 Zeilen, §§ 1–6)
   existierte bereits aus einer früheren Session. Statt neu zu schreiben,
   wurde sie **Aussage für Aussage an den Quellen geprüft**: `scan_tree`/
   `scan_entry`/`file_node`/`is_virtual`/`report_skip`,
   Cap `4×nCPU` (`worker_limit`), Tiefenbegrenzung 64, Filter
   `0 < size < 2^48`, Pfade-by-value und `error_code`-Disziplin in
   `010_scanner.hpp` — alle verifiziert. Relative Links auf absolute
   Repo-Pfade umgestellt (andere Ablage-Location).
3. **Eigene Benchmark-Stichprobe** (vorhandene Release-Binaries, kein
   Neu-Build, 32 CPUs, je 5 Runden `--bench`, Paritäts-Assert überall
   bestanden):
   - `/usr` (4,4 GB): Rust kalt 2,50×, warm 0,97–1,13×; C++ direkt stabil
     1,56–1,77×.
   - Fixture (4000 Dateien, 3,9 MB): Rust 3,89–5,99×.
   - Deckt Schwester-Messung und `report_bench_workspace.md`
     (Rust ~1,0× warm / C++ ~1,5–2,4× je Baum) — zweite unabhängige
     Bestätigung, in `doc.md` § 5.2 eingetragen.
4. **Test-Validierung (Schritt 5 aus `task.md`):**
   - `cargo test --release --offline` (iter2): 4 Unit- + 3
     Integrations-Tests (`headless_scan.rs`) — alle grün.
   - `ctest` (`iter2/cpp/build/bench`): 1/1 bestanden.
   - Linkcheck: alle 14 referenzierten Quelldateien existieren.
   - Mermaid: 7 Blöcke, alle Zäune balanciert; `<br/>` nur als
     sanktionierter Zeilenumbruch in Labels.
5. **Antwort auf die Leitfragen** (`doc.md` § 5.2–5.3): Initiale Annahmen
   bestätigt mit Korrektur (warmer Cache schrumpft Speedup; C++ seriell
   ~1,5× hinter Rust, parallel vorn; Layout irrelevant). Architektur für
   große Dateisysteme ausreichend; Überarbeitungs-Kriterien dokumentiert
   (Work-Stealing erst bei Millionen Dateien/Netz-FS, `inotify`-Schicht für
   Live-Updates, `rayon` weiter ohne Beleg).

## Learnings

- **Verifizieren schlägt Neu-Schreiben:** Der `diff`- und Stichproben-Ansatz
  (Quellcode-Diff + eigene Messung) war billiger und belastbarer als eine
  Neuerstellung der Doku — jede übernommene Zahl hat jetzt zwei Messungen.
- **Warmer Cache dominiert alles:** Der auffälligste eigene Befund (Rust
  `/usr` Runde 1: 2,50×, danach ~1,0×) erklärt, warum Fixture-Speedups nicht
  auf reale Bäume skalieren — VFS-Bindung, nicht CPU.
- **Ablage-Ort vs. Quell-Ort trennen:** Doku liegt in `rs-summarizer`, Code
  in `transpiled_treemap` — relative Markdown-Links wären still gebrochen;
  absolute Pfade + Herkunftshinweis im Kopf lösen das explizit.
- **Forschungs-Workflow lief parallel:** Ein Recherche-Workflow mit 4
  Recherche-Agenten + Kritiker + Synthese wurde gestartet; die eigenen
  Befunde (Diff, Messungen, Testläufe) deckten den Bedarf bereits ab. Falls
  sein Outline-Synthese-Ergebnis noch eintrifft, wird es gegen § 6
  (Vorschlag-Box) geprüft und ggf. nachgetragen.

## Quellen

- Code: `iter1/src/main.rs`, `iter2/src/main.rs`,
  `iter1/cpp/src/`, `iter2/cpp/src/010_scanner.hpp`,
  `iter2/cpp/src/015_color.hpp`, `iter2/cpp/src/020_layout.hpp`,
  `iter2/lisp/gen.lisp`, `iter1/tests/headless_scan.rs`,
  `iter2/tests/headless_scan.rs`, `iter2/scripts/setup03_rust_bench.sh`.
- Pläne/Reports: `plan/20260919_01_merge/walkthrough.md`,
  `plan/20260919_01_merge/rust_cpp_vergleich.md`,
  `plan/20260920_01_redo/walkthrough.md`,
  `plan/20260920_01_redo/report_rust_cpp.md`,
  `plan/20260920_01_redo/report_bench_workspace.md`.
- Schwester-Doku: `plan/20260920_02_doc/doc.md` (+ dortiges `walkthrough.md`).
- Algorithmus: Bruls / Huizing / van Wijk, „Squarified Treemaps".
- GitHub-Rendering: MathJax (`$…$`, `$$…$$`) und Mermaid
  (` ```mermaid `) laut Aufgabenstellung nativ unterstützt.

## Neue Programme für den Docker-Container

Keine neuen Programme erforderlich: Die gesamte Arbeit nutzte vorhandene
Werkzeuge (cargo/rustc, cmake/ctest, g++, fallocate, git). Für künftige
Reproduktionen genügt die bereits dokumentierte Toolchain; `cargo-insta` o.
ä. wurde nicht eingeführt.
