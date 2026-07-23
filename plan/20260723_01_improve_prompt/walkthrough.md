# Walkthrough & Report: Prompt-Optimierung auf Basis der Gemini 3.5 System-Prompt-Analyse

Dieses Dokument beschreibt die durchgeführten Arbeiten zur Optimierung der System- und User-Prompts in `rs-summarizer`, die gewonnenen Erkenntnisse und die Testergebnisse.

## 1. Was gemacht wurde

- **Analyse des geleakten Google-System-Prompts (`gemini-3.5-flash.md`)**:
  - Untersuchung der von Google eingebetteten Verhaltensregeln für Gemini 3.5.
  - Gegenüberstellung der interaktiven Chat-Orientierung (Peer Voice, Empathie, Elicitation-Komponenten, Follow-Up-Fragen) mit dem Ziel von `rs-summarizer` (objektive, hochdichte Einzeldurchlauf-Zusammenfassungen).
  - Identifikation interner Stellschrauben:
    - **RULE 1 (STRICT COMPLETION)**: Verhindert Follow-Up-Fragen und Chat-Geplänkel bei strikt geregelten Prompts.
    - **Specifics Over Generalities**: Erzwingt konkrete Messwerte, Zahlen, Prozentangaben und Eigennamen statt vager Aussagen.
    - **Markdown-Toolkit**: Verlangt visuelle Scannbarkeit durch klare `##`-Hierarchien und Fettung.

- **Optimierung von `prompts/system_instruction.txt`**:
  - Ergänzung der **Single-Pass Output Directive** zur Vermeidung von Begrüßungen, Floskeln und Follow-Up-Fragen (Aktivierung von Googles "Strict Completion Mode").
  - Ergänzung der **Structural Repeatability Directive** zur Außerkraftsetzung von Googles `<variety_principle>` (garantiert identischen, wiederholbaren Aufbau aller Zusammenfassungen mit `## Abstract` und `## Key Highlights & Timestamps`).
  - Ergänzung der Direktive **Specifics Over Generalities** zur Maximierung der Informationsdichte (Zahlen, Prozentwerte, Metriken, Versionen).
  - Klarstellung der Markdown-Strukturvorgaben (`## Abstract`, `## Key Highlights & Timestamps`, ggf. `## Analyst Notes`).
  - Wahrung des Prinzipien der "Dutch Directness" und der Senior-Analyst-Persona.

- **Überarbeitung von `src/services/summary.rs`**:
  - Korrektur eines grammatikalischen Tippfehlers in `build_prompt` (`"Please summarize provide a summary..."` -> `"Please provide a summary..."`).
  - Ergänzung von Vorgaben zur strikten strukturellen Wiederholbarkeit und Informationsdichte in `build_prompt` und `build_hn_prompt`.
  - Aktualisierung der Unit-Tests in `src/services/summary.rs`, um die neuen Directives der `SYSTEM_INSTRUCTION` abzuprüfen.

- **Verifikation & Testabdeckung**:
  - Ausführung der gesamten Testsuite (`cargo test`). Alle 106 Unit-Tests liefen erfolgreich durch.

## 2. Erkenntnisse & Gelerntes

- **Overriding von Googles `<variety_principle>`**:
  - Googles interner Prompt enthält den Abschnitt `<variety_principle>`, der das Modell dazu anhält, das Layout nicht mechanisch zu wiederholen ("Avoid falling into a mechanical rhythm of using the exact same layout...").
  - Für automatisierte Zusammenfassungen ist jedoch genau das Gegenteil gewünscht: Eine vorhersagbare, strikt wiederholbare Struktur. Durch das Einfügen der **Structural Repeatability Directive** wird das Variationsgebot von Gemini überschrieben.

- **Chat-System-Prompts vs. API-Tasks**:
  - Da Google Gemini standardmäßig mit einem System-Prompt ausführt, der auf interaktive Konversationen getrimmt ist, versucht das Modell ohne explizite Gegenanweisung gelegentlich, am Ende des Texts Rückfragen zu stellen oder Einleitungen ("Here is your summary...") hinzuzufügen.
  - Indem wir im System-Prompt explizit den "Strict Single-Pass Execution Mode" deklarieren, triggern wir Googles interne "Rule 1: Strict Completion", wodurch saubere, reine Zusammenfassungen ohne Metatext generiert werden.

- **Nutzen von "Specifics Over Generalities"**:
  - Gemini 3.5 spricht extrem stark auf die explizite Aufforderung an, konkrete Zahlen, Metriken und Namen zu verwenden. Dies hebt die Qualität der Zusammenfassungen von generischen Phrasen ("the presenter talked about speed improvements") auf präzise Fachinformationen ("latency decreased by 45ms (38%)").

- **Few-Shot Alignment**:
  - Die bestehenden Few-Shot-Beispiele in `prompts/example_output.txt` und `prompts/example_output_abstract.txt` passten bereits hervorragend zum verlangten Muster (viele konkrete Zahlen wie 488nm, 50 Megapixel, etc.).

## 3. Testergebnisse & Status

- `cargo test`: 106 passed, 0 failed.
- Alle Prompt-Builder und System-Instructions sind aktualisiert und sauber im Repository gepflegt.
