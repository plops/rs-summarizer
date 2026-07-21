# Implementierungsplan: Integration von Gemini 3.6 Modellen

Dieses Dokument beschreibt das Vorgehen zur Umsetzung der neuen Modell-Auswahl in `rs-summarizer`.

## 1. Analyse & Anforderungen
- **Neue Hauptmodelle**: `Gemini 3.6 Flash` und `Gemini 3.5 Flash Lite` (das neueste Flash Lite Modell, da 3.6 Flash Lite nicht existiert) sollen als erste auswählbare Modelle in der Benutzeroberfläche (nach `auto`) platziert werden.
- **CSV-Abgleich (`gemini36_20260721.csv`)**:
  - `Gemini 3.6 Flash` -> RPM 5, RPD 20.
  - `Gemini 3.5 Flash Lite` -> RPM 15, RPD 500.
- **Bestehende funktionierende Modelle**: Alle anderen zuvor unterstützten Modelle mit RPD-Limit > 0 werden beibehalten:
  - `gemini-3.5-flash`
  - `gemini-3-flash-preview`
  - `gemini-3.1-flash-lite`
  - `gemini-2.5-flash`
  - `gemini-2.5-flash-lite`
  - `gemma-4-31b-it` (mit aktualisierten Limits aus CSV: RPM 30, RPD 14.4K)
  - `gemma-4-26b-a4b-it` (mit aktualisierten Limits aus CSV: RPM 30, RPD 14.4K)
- **Heuristische Steuerung (`auto`)**:
  - Aktualisierung der Heuristik in `src/tasks.rs`. Bei kurzen Transkripten (<30 Minuten) soll `gemini-3.5-flash-lite` gewählt werden, bei längeren bzw. für Hacker News `gemini-3.6-flash`.
- **Fallback-Ketten (`src/tasks.rs`)**:
  - Erweiterung von `get_fallback_chain` für die neuen Modelle, damit bei Erreichen des RPD-Limits sauber auf ältere/günstigere Modelle ausgewichen wird.

## 2. Vorgehensweise
1. **Modellanpassung in `src/state.rs`**:
   - Einfügen von `gemini-3.6-flash` und `gemini-3.5-flash-lite` an den Anfang der Liste (nach `auto`).
   - Aktualisierung der Limits für `gemma-4-31b-it` und `gemma-4-26b-a4b-it` auf RPM 30, RPD 14400.
   - Aktualisierung/Erweiterung der Modellanforderungen und der Tests in `src/state.rs`.
2. **Heuristik & Fallback in `src/tasks.rs`**:
   - `get_fallback_chain` anpassen, um die neuen Modelle einzupflegen.
   - Die `auto`-Heuristik aktualisieren, sodass standardmäßig `gemini-3.6-flash` bzw. `gemini-3.5-flash-lite` verwendet wird.
3. **Dokumentations-Update**:
   - Falls notwendig, Aktualisierung von Referenzen in `README.md` oder anderen Hilfsdokumenten.
4. **Tests ausführen**:
   - Ausführen aller Cargo-Tests und Beheben eventueller Fehler in Unit- und Integrationstests.
5. **Report & Walkthrough**:
   - Erstellung des Abschlussberichts (`walkthrough.md`) im selben Verzeichnis wie dieser Plan.
