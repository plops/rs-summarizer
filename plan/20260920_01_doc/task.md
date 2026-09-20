# task.md — serielle Arbeits- und Validierungsschritte

Jeder Schritt endet mit einer Validierung; erst bei grüner Validierung folgt
der nächste Schritt. Basis-Pfad der dokumentierten Quellen:
`/workspace/src/transpiled_treemap/`.

## Schritt 1 — Quellen sichten und Diff iter1 → iter2 erheben ✅

- `iter2/src/main.rs` vollständig lesen; `iter1/src/main.rs` per `diff`
  vergleichen (Helfer `is_virtual`/`dir_node`/`file_node`, Cap, Tiefe).
- `iter2/cpp/src/010_scanner.hpp` lesen; C++-Diff iter1 → iter2 erheben
  (Pfade by value, `ScanWorkerLimit`, `needs_recursion`).
- Validierung: jede Zeile der §-2.2-Tabelle in `doc.md` nennt die
  Quelldatei, die sie belegt.

## Schritt 2 — Benchmark-Reports prüfen und eigene Stichprobe fahren ✅

- `report_bench_workspace.md` + `report_rust_cpp.md` lesen (Methode,
  Tabellen, `/`-Abbruch mit Mismatch als erwartet).
- Eigene Messung mit vorhandenen Release-Binaries (kein Neu-Build):
  `treemap_iter2 --bench /usr`, `treemap_cpp --bench /usr`,
  Fixture-Lauf (4000 Dateien) — je 5 Runden.
- Validierung: Paritäts-Assert in allen Läufen bestanden; Rangfolge und
  Größenordnung decken die Reports (Rust warm ~1,0×, C++ ~1,6×, Fixture
  ~4–6×). Ergebnis in `doc.md` § 5.2 eingetragen.

## Schritt 3 — `doc.md` ablegen und härten ✅

- Basis aus verifizierter Schwester-Doku übernommen, relative Links auf
  absolute Repo-Pfade umgestellt (diese Ablage liegt in einem anderen Repo),
  eigene Messung in § 5.2 ergänzt.
- Validierung: Linkcheck (Schritt 5) + Mermaid-Härtung (Labels ohne
  ungequotete `<`/`>`, balancierte Zäune).

## Schritt 4 — `deps.md`, `plan.md`, `task.md` schreiben ✅

- `deps.md`: macroquad → `not-fl3/macroquad`, olcPixelGameEngine →
  `OneLoneCoder/olcPixelGameEngine`, CMake/Ninja als Build-Werkzeuge;
  DeepWiki-Fragen als Beispiele (kein MCP-Zugang in dieser Session).
- `plan.md`: Dateiliste mit Leseaufträgen, Conventional-Commit-Regeln,
  Validierungsstrategie.
- Validierung: jede Dependency in `deps.md` ist im Code auffindbar
  (`use macroquad`, `third_party/olcPixelGameEngine3.h`, `CMakeLists.txt`).

## Schritt 5 — Validierung ausführen (dieser Schritt)

1. `cargo test` (iter2) und `ctest` (C++-Builds) ausführen bzw. als nicht
   ausführbar dokumentieren.
2. Linkcheck-Skript über `doc.md`: alle referenzierten Dateien existieren.
3. Mermaid-Sanity: 7 `mermaid`-Blöcke, Zäune balanciert.
4. Erst bei 1–3 grün: `walkthrough.md` schreiben und committen.

## Schritt 6 — `walkthrough.md` schreiben und committen

- Experimente, Learnings, Quellen, neue Container-Programme (siehe Schritt 5).
- Commits nach `plan.md` § 4, nur eigene Dateien, vorher `git status`.
