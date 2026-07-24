# Implementierungsplan: 1-5 Sterne Rating System für `/browse`

Dieses Dokument beschreibt den detaillierten Implementierungsplan für das 1 bis 5 Sterne Rating System in `rs-summarizer`.

---

## 1. Kontext & Relevante Dateien

Ein unabhängiger AI-Agent kann sich mithilfe dieser Dateiliste selbstständig den Kontext erarbeiten:

* **`migrations/004_add_ratings.sql`**  
  *Beschreibung:* Neue SQL-Migration zur Erstellung der Tabelle `summary_ratings` und dem Unique Index `(summary_id, client_ip)`.
* **`src/models.rs`**  
  *Beschreibung:* Erweitert um Datenstrukturen für Ratings (`SummaryRating`, `RatingStats`, `SubmitRatingForm` etc.).
* **`src/db.rs`**  
  *Beschreibung:* Enthält die SQL-Datenbankfunktionen zum Upserten von Benutzer-Ratings, Berechnen von Rating-Statistiken (Durchschnitt & Anzahl) und Laden von Rating-Informationen für die Browse-Seite.
* **`src/routes/mod.rs`**  
  *Beschreibung:* HTTP Handler für das Einreichen von Ratings (`POST /summaries/{identifier}/rate`) und Anpassung der Browse-Route (`GET /browse`) zum Auslesen von Ratings und IP-Extraktion.
* **`src/lib.rs`**  
  *Beschreibung:* Axum Router-Registrierung der neuen Rating-Endpoints.
* **`src/templates.rs`**  
  *Beschreibung:* Askama-Template-Strukturen (`BrowseSummaryItem`, `RatingPartialTemplate`, `BrowseTemplate`).
* **`templates/browse.html`**  
  *Beschreibung:* HTML-Template der Browse-Seite, erweitert um interaktive Sterne-Rating-Komponenten (HTMX-basiert).
* **`templates/rating_partial.html`** (optional/integriert)  
  *Beschreibung:* Partial-Template zur dynamischen HTMX-Aktualisierung der Sterne nach der Stimmabgabe.
* **`tests/integration_ratings.rs`**  
  *Beschreibung:* Integrationstests zur Verifikation von Rating-Erstellung, Upsert per IP-Adresse, Grenzwertprüfungen und HTML-Partial-Rendering.

---

## 2. Anforderungen & Systemarchitektur

### Kernanforderungen
1. **Dual-Rating System (1 bis 5 Sterne):**
   * **Zusammenfassungs-Rating (`summary_rating`):** Wie gut ist die generierte Zusammenfassung? (1–5 Sterne)
   * **Artikel-Rating (`content_rating`):** Wie gut/herausragend ist der Original-Artikel? (1–5 Sterne)
2. **Anonyme IP-basierte Zuordnung (Upsert):**
   * Jeder Client wird anhand seiner IP-Adresse identifiziert.
   * Gibt ein Client erneut eine Bewertung ab, wird sein bisheriges Rating aktualisiert (Upsert).
   * **Datenschutz & Sicherheit:** Die IP-Adresse wird ausschließlich in der Datenbank zur Zuordnung gespeichert und **niemals** im Frontend oder in Web-Antworten angezeigt.
3. **Statistik-Berechnung:**
   * Für jede Zusammenfassung werden der Mittelwert (z.B. 4.2 / 5.0) und die Gesamtzahl der Stimmen für beide Kategorien ermittelt.
   * Wenn der aktuelle Client bereits abgestimmt hat, wird seine gewählte Sternanzahl hervorgehoben.
4. **Interaktive HTMX-Integration:**
   * Beim Klick auf einen Stern wird per `hx-post` ein Request gesendet und das Rating-Element nahtlos ohne Neuladen der gesamten Seite aktualisiert.

---

## 3. Datenbank-Design

Tabelle `summary_ratings` in `migrations/004_add_ratings.sql`:

```sql
CREATE TABLE IF NOT EXISTS summary_ratings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    summary_id INTEGER NOT NULL,
    client_ip TEXT NOT NULL,
    summary_rating INTEGER CHECK(summary_rating IS NULL OR (summary_rating >= 1 AND summary_rating <= 5)),
    content_rating INTEGER CHECK(content_rating IS NULL OR (content_rating >= 1 AND content_rating <= 5)),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY(summary_id) REFERENCES summaries(identifier) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_summary_ratings_summary_ip
    ON summary_ratings(summary_id, client_ip);
```

Upsert-Logik in SQLite:
```sql
INSERT INTO summary_ratings (summary_id, client_ip, summary_rating, content_rating, created_at, updated_at)
VALUES (?, ?, ?, ?, datetime('now'), datetime('now'))
ON CONFLICT(summary_id, client_ip) DO UPDATE SET
    summary_rating = COALESCE(excluded.summary_rating, summary_ratings.summary_rating),
    content_rating = COALESCE(excluded.content_rating, summary_ratings.content_rating),
    updated_at = datetime('now');
```

---

## 4. Conventional Commit Regeln

Alle Commits müssen dem **Conventional Commit** Format entsprechen und eine umfassende Beschreibung im Body enthalten:

Format:
```text
<type>(<scope>): <kurze Zusammenfassung im Präsens>

<Ausführliche Beschreibung, WARUM und WAS geändert wurde.>
```

Typen:
* `feat(ratings)`: Neue Features für Sterne-Bewertungen.
* `db(ratings)`: Datenbankmigrationen und Abfragen.
* `test(ratings)`: Neue Unit- und Integrationstests.
* `docs(ratings)`: Plan- und Walkthrough-Dokumentation.

---

## 5. Teststrategie

1. **Unit Tests (`src/db.rs` / `src/routes/mod.rs`):**
   * Validierung von `summary_rating` und `content_rating` im Bereich 1..=5.
   * DB-Upsert-Test: Mehrmalige Bewertung derselben IP überschreibt den Wert korrekt.
   * Berechnung der Durchschnittswerte und Anzahl bei verschiedenen Stimmenabgaben.
   * Anonymisierungsprüfungen (IP-Adresse taucht in keiner HTML-Ausgabe auf).
2. **Integration Tests (`tests/integration_ratings.rs`):**
   * Endpoint `POST /summaries/{id}/rate` mit Formular-Parametern aufrufen.
   * Unerlaubte Werte (z.B. 0, 6, String) werden abgelehnt (HTTP 400 oder Fehlermeldung).
   * HTML-Partial Rückgabe prüfen und Verifizieren der aktualisierten Werte.
   * `/browse` Route abfragen und korrekte Sterneanzeige testen.

---

## 6. Walkthrough Dokument

Nach erfolgreicher Implementierung und vollständigem Bestehen aller Tests wird die Dokumentation unter:
`plan/20260724_01_star_rating/walkthrough.md`
erstellt.
