# Implementierungsplan: Technische Dokumentation Treemap-Experimente

Stand: 2026-09-20 · Auftrag: `prompt.txt` (wol pumba) · Ablage:
`rs-summarizer/plan/20260920_01_doc/`

## 1. Kontext für den implementierenden Agenten

Die zu dokumentierenden Programme liegen **nicht** in diesem Repo, sondern im
Schwester-Repo `/workspace/src/transpiled_treemap/` (Rust- und C++-Programme,
Iterationen `iter1/` und `iter2/`). Eine verifizierte Schwester-Doku existiert
unter `/workspace/src/transpiled_treemap/plan/20260920_02_doc/doc.md` — sie
ist die inhaltliche Basis; diese Ablage übernimmt sie geprüft und ergänzt sie
um eine zweite unabhängige Benchmark-Stichprobe. Jede übernommene Aussage muss
an einer der unten genannten Quelldateien verankert sein.

## 2. Dateien, die der Agent lesen muss (mit Leseauftrag)

| Datei | Was daraus zu belegen ist |
|---|---|
| `transpiled_treemap/iter2/src/main.rs` (~662 Zeilen) | Referenz: serieller Scan, paralleler Scan (Cap `4×nCPU`, Tiefe 64, Dateien inline), Squarified-Layout, Modi `--scan`/`--bench`/GUI |
| `transpiled_treemap/iter1/src/main.rs` (~672 Zeilen) | Diff-Basis: was iter2 herausfaktoriert (`is_virtual`, `dir_node`, `file_node`) und von Anfang an mitbringt (Cap, Tiefe) |
| `transpiled_treemap/iter2/cpp/src/010_scanner.hpp` | C++-Gegenstück: Pfade by value, `error_code`-Disziplin, `report_skip` mit Mutex |
| `transpiled_treemap/iter2/cpp/src/015_color.hpp`, `020_layout.hpp` | Farbe (`color_for_path`), Layout, `needs_recursion`-Helfer |
| `transpiled_treemap/iter2/lisp/gen.lisp` (~462 Zeilen) + `lisp/gen/*.hpp` | Transpiler-Input; Beleg, dass das Generat eine vollständige, kompilierbare TU ist |
| `transpiled_treemap/iter1/cpp/gen.lisp` (~400 Zeilen) | Beleg für „iter1-Generat nur syntaxgeprüft" |
| `transpiled_treemap/iter1+iter2/tests/headless_scan.rs` | Paritäts-/Kantenfall-Tests (bis auf Fixture-Namen identisch) |
| `transpiled_treemap/plan/20260919_01_merge/rust_cpp_vergleich.md` | iter1-Lektionen (Single-Stat, Thread-pro-Datei, Cap) |
| `transpiled_treemap/plan/20260920_01_redo/report_bench_workspace.md`, `report_rust_cpp.md` | Reale Benchmark-Zahlen (Workspace/Root/usr) |
| `transpiled_treemap/plan/20260919_01_merge/walkthrough.md`, `plan/20260920_01_redo/walkthrough.md` | Experiment-Verlauf, offene Punkte (rayon verworfen, TSan, Cushion-Shading) |
| `transpiled_treemap/iter2/scripts/setup01–09_*.sh` | Reproduktionspfad Build/Test/Bench |

## 3. Deliverables (diese Ablage)

- `doc.md` — deutsches Technik-Dokument, GitHub-nativ (MathJax `$…$`/`$$…$$`,
  Mermaid-` ```mermaid `-Blöcke, `<`/`>` in Labels quoten). Pflicht-Sektionen:
  Architektur Rust vs. C++ (§ 2.1), Iteration 1 vs. 2 (§ 2.2), Datenerhebung
  (§ 3), Treemap-Layout (§ 4), Benchmark-Bewertung inkl. Leitfragen
  (Annahmen bestätigt? Großtauglich? Redesign nötig?) (§ 5).
- `deps.md` — Dependencies mit GitHub-Organisation (DeepWiki-fähig).
- `task.md` — serielle Arbeits- und Validierungsschritte (dieser Plan).
- `walkthrough.md` — Experimente, Learnings, Quellen, neue Container-Programme.

## 4. Commit-Konvention (Conventional Commits, ausführlicher Body)

Format: `<typ>(<scope>): <kurze Zusammenfassung>` + Leerzeile + Body mit
Was/Warum/Verifikation. Typen: `docs` (Dokumentation), `test` (Tests),
`chore` (Ablage/Commits). Beispiele:

- `docs(doc): Treemap-Architektur iter1/iter2 dokumentieren`
- `docs(doc): unabhängige Benchmark-Stichprobe ergänzen`
- `docs(walkthrough): Experimente und Learnings festhalten`

Commits erfolgen inkrementell („gelegentlich committen"), nur eigene Dateien
(`git add plan/20260920_01_doc/...`), vor jedem Commit `git status` prüfen.

## 5. Test- und Validierungsstrategie

Neue Unit-/Integrationstests „wie erforderlich": Für ein reines
Dokumentations-Deliverable sind das Validierungsprüfungen statt Code-Tests:

1. Bestehende Suiten unverändert grün: `cargo test` (iter2),
   `ctest` (C++-Builds) — falls der Container sie ohne Neu-Installation
   ausführen kann, sonst als „nicht ausführbar" dokumentieren.
2. Paritäts-Assert der `--bench`-Läufe (seriell == parallel) als
   Korrektheits-Orakel jeder eigenen Messung.
3. Link-/Konsistenzprüfung der Doku: jede referenzierte Quelldatei existiert;
   Mermaid-Blöcke balanciert; keine ungequoteten `<`/`>` in Labels.
4. Eigene Benchmark-Stichprobe mit vorhandenen Release-Binaries
   (kein Neu-Build nötig): je 5 Runden `--bench` auf `/usr` + Fixture,
   Abgleich gegen `report_bench_workspace.md` (Toleranz: Rangfolge und
   Größenordnung, keine Bit-Gleichheit wegen Cache-Lage).
