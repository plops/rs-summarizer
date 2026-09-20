# deps.md — GitHub-Repositories der Dependencies

Ziel: Jede eingeführte Abhängigkeit steht hier mit ihrer GitHub-Organisation,
sodass sich später DeepWiki-Abfragen der Form `<org>/<projekt>` konstruieren
lassen. Stand: 2026-09-20, keine neuen Dependencies eingeführt (reine
Dokumentationsarbeit; die dokumentierten Programme nutzen nur die
Standardbibliotheken ihrer Sprache plus je ein GUI-Backend).

## Dokumentierte Programme (`/workspace/src/transpiled_treemap`)

| Crate / Paket | GitHub `org/projekt` | Verwendung |
|---|---|---|
| macroquad | not-fl3/macroquad | GUI-Backend der Rust-Programme (iter1 + iter2) |
| olcPixelGameEngine (vendored Header) | OneLoneCoder/olcPixelGameEngine | GUI-Backend der C++-Programme (`third_party/olcPixelGameEngine3.h`) |

## Build-Werkzeuge (keine Code-Dependencies, zur Vollständigkeit)

| Werkzeug | GitHub `org/projekt` | Verwendung |
|---|---|---|
| CMake | Kitware/CMake | Build der C++-Targets (`treemap_cpp`, `treemap_gui`, `core_tests`) |
| Ninja | ninja-build/ninja | CMake-Generator der C++-Builds |

## Hinweis zu DeepWiki

Aktuell ist kein DeepWiki-MCP-Werkzeug in dieser Session verfügbar; die
Recherche erfolgte direkt an den Quellen (siehe `walkthrough.md`). Sobald
ein DeepWiki-Zugang besteht, lassen sich Usage-Examples z. B. so erfragen:

- `repoName="not-fl3/macroquad"`, Frage: „Wie öffnet man ein Fenster mit
  eigenem Render-Loop und Maus-Hover (vgl. `render_tree`)?"
- `repoName="OneLoneCoder/olcPixelGameEngine"`, Frage: „Wie bindet man eine
  PGE-App als draw-Schleife über einen eigenen Szenengraphen ein (vgl.
  `030_pge_app.hpp`)?"

## Rust-Toolchain

Dokumentationsarbeit ohne Code-Änderung: kein Toolchain-Wechsel vorgenommen.
Verwendete stabile Toolchain des Containers für die Verifikations-Benchmarks:
`rustc`/`cargo` laut `rustc --version` zum Messzeitpunkt (siehe
`walkthrough.md`); kein `cargo upgrade`, keine neuen Crates.
