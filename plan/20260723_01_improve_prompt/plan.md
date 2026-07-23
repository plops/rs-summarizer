# Implementierungsplan: Optimierung der System- und User-Prompts in rs-summarizer

Dieses Dokument beschreibt die Analyse des internen Google-System-Prompts (`gemini-3.5-flash.md`) und das geplante Vorgehen zur Präzisierung und Verbesserung der Prompts in `rs-summarizer`.

## 1. Analyse & Anforderungen

### Ausgangslage
- **Googles interner System-Prompt (`gemini-3.5-flash.md`)**:
  - Ist primär für interaktive Web-Chats konzipiert (empathischer Tonfall, Peer Voice, Elicitation-Komponenten, Follow-Up-Fragen, interaktive Widgets).
  - Enthält **RULE 1 (STRICT COMPLETION)**: Wenn ein Prompt strikte Regeln (z. B. JSON, Wortgrenzen) oder eine in sich geschlossene Aufgabe erzwingt, sollen sämtliche Rückfragen, Follow-Ups, Menüs und conversational Fluff zwingend weggelassen werden.
  - Enthält das Qualitätsgebot **"Specifics Over Generalities"**: Vage Aussagen ("Exercise has benefits") müssen durch konkrete Messwerte, Zahlen, Prozentangaben und Fakten ersetzt werden ("150 min/week reduces risk by 30-40%").
  - Verlangt klare Markdown-Hierarchien (`##`, `###`, Fettung) für optimale Lesbarkeit.

- **Bisherige Prompts in `rs-summarizer`**:
  - `prompts/system_instruction.txt`: Definiert die Rolle des "adaptive knowledge synthesis engine" und "Top-Tier Senior Analyst", Objektivität ("Dutch Directness") sowie bedingte "Analyst Notes".
  - `src/services/summary.rs`: Erstellt den Few-Shot-User-Prompt (`build_prompt`) und den Hacker News Prompt (`build_hn_prompt`).
  - Im aktuellen `build_prompt` existiert ein Tippfehler: `"Please summarize provide a summary like they would:"`.

### Optimierungspotenziale & Ziele
1. **Aktivierung von Googles "Strict Completion Mode"**:
   - Durch explizite Formulierung der Anfrage als in sich geschlossener Einzeldurchlauf (Single-Pass Execution) verhindern wir, dass das Modell höfliche Floskeln, Begrüßungen oder abschließende Chat-Rückfragen generiert.
2. **Erhöhung der Informationsdichte ("Specifics Over Generalities")**:
   - Die System-Instruction wird explizit angewiesen, konkrete Zahlen, Metriken, Versionen, Fachbegriffe und Kernfakten gegenüber vagen Verallgemeinerungen zu bevorzugen.
3. **Strukturierte Markdown-Ausgabe**:
   - Vorgabe klarer Markdown-Überschriften (`## Abstract`, `## Key Highlights & Timestamps`, ggf. `## Analyst Notes`), was der internen Formatierungskonvention von Gemini entspricht und die Visualisierung/Lesbarkeit im Frontend verbessert.
4. **Bereinigung von Tippfehlern & Code-Schärfung**:
   - Bereinigung der Grammatik in `build_prompt`.
   - Schärfung von `build_hn_prompt` bzgl. Diskussionstiefe und Struktur.
5. **Unit-Test-Aktualisierung**:
   - Anpassung und Erweiterung aller Prompt-bezogenen Tests in `src/services/summary.rs`.

## 2. Vorgehensweise

1. **Erstellung des Implementierungsplans**:
   - Speichern dieses Dokuments unter `plan/20260723_01_improve_prompt/plan.md`.

2. **Überarbeitung von `prompts/system_instruction.txt`**:
   - Ergänzung einer klaren Direktive für **Strict Single-Pass Execution** (keine Floskeln, keine Rückfragen).
   - Hervorhebung von **Data Density & Concrete Specifics** (Zahlen, Daten, konkrete Fakten statt Verallgemeinerungen).
   - Schärfung der **Markdown-Strukturvorgaben** (`## Abstract`, `## Key Highlights & Timestamps`).

3. **Überarbeitung der Prompt-Builder in `src/services/summary.rs`**:
   - Korrektur des Tippfehlers in `build_prompt`.
   - Präzisierung der Vorlagen für Standard-Transkripte und Hacker News Submissions.
   - Aktualisierung der Unit-Tests in `src/services/summary.rs`.

4. **Überprüfung und Tests**:
   - Ausführen der Cargo Test Suite (`cargo test`), um die korrekte Prompt-Generierung und Testabdeckung sicherzustellen.

5. **Dokumentation & Abschlussbericht**:
   - Erstellung des Berichts `plan/20260723_01_improve_prompt/walkthrough.md`.

6. **Git Commits**:
   - Einpflegen der Änderungen mittels sauberer Conventional Commit Messages.
