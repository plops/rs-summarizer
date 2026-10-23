# Aufwand: All-Entries-Browser (`20261023_01_all_entries_browser`)

Hinweis: Token-Zähler der Session waren für mich nicht einsehbar; die
Angaben unten sind daher grobe Schätzungen aus Verlauf und Laufzeit.

- Turns (Modell-Antworten mit Tool-Calls): ca. 20
- Subagents: 0 (alles inline, keine Delegation)
- WebDriver/Browser-Setup: 1 Download (chrome-headless-shell +
  chromedriver 156.0.8078.47, ca. 150 MB nach /opt/chrome-test)
- Testläufe:
  - `cargo test` (gesamt): grün — 166 Lib-Tests, 4
    `integration_entries`, 7 weitere Route-/Rating-Tests, Rest ignoriert
    (Browser) oder ohne Key übersprungen
  - `cargo test --test integration_browser test_entries_browser_overview_and_lazy_detail -- --ignored`: 1 bestanden (echter Headless-Chrome)
  - `cargo clippy --all-targets -- -D warnings`: sauber
  - `cargo fmt --check`: sauber
- Größter Kostenpunkt: Erstimplementierung + ein Test-Fix (ungültige
  YouTube-Test-ID). Der Browser-Test selbst lief beim ersten Versuch.
