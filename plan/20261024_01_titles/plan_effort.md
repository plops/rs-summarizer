# Aufwand: Titel-Feature (`20261024_01_titles`)

Gemessene Session-Werte (Muse Code, 24.10.2026, ca. 3:58 Uhr):

| Kennzahl   | Wert       |
|:-----------|:-----------|
| Input      | 20.156.692 |
| Cached     | 19.355.178 |
| Output     | 127.733    |
| Total      | 20.284.425 |
| Turns      | 126        |
| Subagents  | keine      |
| Observers  | 91 abgeschlossen (1.481.929 Tokens) |
| Dauer      | ca. 5 Min. 17 Sek. (reine Arbeitszeit im Titel-Task) |
| Modell     | muse-spark-1.3 · high |

Testläufe (alle beobachtet, grün):
- `cargo test --lib db::` (fokussiert): 19 bestanden
- `cargo test --test integration_titles --test integration_entries --test integration_ui`: 4 + 4 + 7 bestanden
- `cargo test` (breit): alle 10 Test-Binaries ok
- Browser (Headless-Chrome 156): `test_titles_render_in_entries_and_browse`
  + Regression `test_entries_browser_overview_and_lazy_detail` bestanden
- `cargo clippy --all-targets -- -D warnings`: sauber
- `cargo fmt --check`: sauber
