# Implementierungsplan: Batch-Verarbeitung von Hacker News & Video-URLs

Dieses Dokument beschreibt das Vorgehen zur Umsetzung der Batch-Verarbeitung von gemischten URLs (Hacker News und Videos) sowie zur Verbesserung des Loggings bezüglich heruntergeladener Inhalte in `rs-summarizer`.

## 1. Analyse & Anforderungen
- **Batch-Verarbeitung für Hacker News und gemischte URLs**:
  - Wenn ein Benutzer eine Liste von URLs eingibt (z. B. mehrere HN-Links oder eine Mischung aus YouTube- und HN-Links) und kein Transkript einfügt (`transcript.is_empty()`), sollen alle URLs nacheinander verarbeitet werden.
  - Derzeit wird bei einem HN-Link als erstem Eintrag nur dieser eine Eintrag verarbeitet und alle weiteren URLs ignoriert.
- **Verbessertes Logging für Downloads**:
  - Es soll im Log ersichtlich sein, welche URLs heruntergeladen werden.
  - Die Größe der heruntergeladenen Inhalte (nach Konvertierung in Text) in Bytes/Zeichen und Wortanzahl soll protokolliert werden. Dies betrifft:
    - YouTube-Transkripte
    - Hacker-News-Kommentare/Diskussionen
    - Externe Artikel, die über Hacker News verlinkt sind

## 2. Vorgehensweise
1. **Erweiterung der Batch-Schleife in `src/tasks.rs`**:
   - Strukturierung von `process_summary_inner` so umbauen, dass bei leerem `summary.transcript` generell die Schleife `for url in &urls` durchlaufen wird.
   - Innerhalb der Schleife prüfen, ob die URL ein Hacker News Link ist.
     - Falls ja: Fetch via `HackerNewsService`. Bei `auto` als Modell wird standardmäßig `gemini-3.6-flash` gewählt.
     - Falls nein: Download via `TranscriptService`. Modellwahl erfolgt dynamisch basierend auf der Videolänge (unter 30 Minuten: `gemini-3.5-flash-lite`, sonst `gemini-3.6-flash`).
   - Beide Pfade führen dieselbe Ratenbegrenzung, Modellauswahl und Zusammenfassungs-Logik aus.
   - Am Ende der Schleife werden die Transkripte, Denkprozesse (Thinking) und Zusammenfassungen kombiniert in der Datenbank gespeichert.

2. **Detailliertes Logging implementieren**:
   - **`src/services/transcript.rs`**:
     - Loggen vor dem Start des Downloads (URL).
     - Loggen nach erfolgreichem Download/Parsing mit Angabe der Textgröße in Bytes und Wörtern.
   - **`src/services/hacker_news.rs`**:
     - Loggen bei Beginn des Abrufs einer HN-Story.
     - Loggen der abgerufenen Kommentare (Größe).
     - Loggen bei Beginn des Downloads eines externen Artikels sowie nach erfolgreicher HTML-zu-Text-Konvertierung (Größe in Bytes und Wörtern).

3. **Tests**:
   - Erstellen eines Unit- oder Integrationstests, der die Batch-Verarbeitung von mehreren Links (HN & Video gemescht) simuliert.
   - Ausführen von `cargo test`.

4. **Abschlussbericht**:
   - Erstellen von `plan/20260722_01_hn_batch/walkthrough.md`.
