# Requirements Document

## Introduction

Dieses Feature erweitert rs-summarizer um eine Embedding-Visualisierung. Ausgangspunkt ist ein Python-Prototyp (`gemini-summary-embedding`), der Embeddings aus der SQLite-Datenbank mit UMAP reduziert und mit DBSCAN clustert. Das Ziel ist, diese Funktionalität vollständig in Rust zu implementieren und in rs-summarizer zu integrieren.

Das Feature umfasst sechs Teilbereiche:
1. Einen Skill für das Arbeiten mit Rust-Projekten (Cargo-Workflow)
2. Eine CLI-Option zum Exportieren einer kompakten Datenbank (ohne Transkripte)
3. Ein neues Rust-Subprojekt als egui-GUI für UMAP/DBSCAN-Visualisierung
4. Automatische Cluster-Betitelung via Gemini/Gemma API
5. Parametric UMAP (neuronales Netz) für stabiles, zeitinvariantes Mapping
6. Integration der Visualisierung in das bestehende HTMX-Web-Frontend

## Glossary

- **Summarizer**: Die bestehende rs-summarizer Rust/axum-Webanwendung
- **DB_Exporter**: Die neue CLI-Komponente in rs-summarizer, die eine kompakte Datenbank exportiert
- **Compact_DB**: Die exportierte SQLite-Datenbank, die nur Zusammenfassungen, Embeddings und Metadaten enthält (keine Transkripte)
- **Source_DB**: Die originale SQLite-Datenbank mit allen Daten inklusive Transkripten (~2 GB)
- **Viz_Tool**: Das neue Rust-Subprojekt (Cargo workspace member) mit egui-GUI für Embedding-Visualisierung
- **UMAP_Engine**: Die `fast-umap`-basierte Komponente im Viz_Tool, die Dimensionsreduktion durchführt
- **DBSCAN_Engine**: Die `linfa-clustering`-basierte Komponente im Viz_Tool, die Clustering durchführt
- **Cluster_Titler**: Die Komponente im Viz_Tool, die Gemini/Gemma API aufruft, um Cluster-Titel zu generieren
- **NN_Mapper**: Die parametric-UMAP-Komponente (neuronales Netz), die das UMAP-Mapping für neue Punkte stabil hält
- **Web_Viz**: Die neue Visualisierungskomponente im HTMX-Web-Frontend des Summarizers
- **Embedding**: Ein Float32-Vektor im Matryoshka-Format (erzeugt von `gemini-embedding-001` oder `gemini-embedding-2`), der eine Zusammenfassung semantisch repräsentiert. Die maximale Dimension beträgt 3072, aber nicht alle Einträge in der Datenbank haben zwingend diese Dimension. Für erste Experimente werden die ersten 768 Elemente verwendet (konfigurierbar per CLI-Parameter).
- **Embedding_Dim**: Die tatsächlich verwendete Präfix-Länge des Matryoshka-Embeddings (CLI-Parameter, Standard: 768, Maximum: 3072)
- **Cargo_Skill**: Die Kiro-Skill-Datei, die den Cargo-Workflow für dieses Projekt dokumentiert (Dependency-Verwaltung, Versionen, DeepWiki-Lookups)

---

## Requirements

### Requirement 1: Cargo-Workflow-Skill

**User Story:** Als Entwickler möchte ich eine dokumentierte Skill-Datei für den Cargo-Workflow, damit ich und zukünftige Agenten wissen, wie Dependencies korrekt verwaltet werden.

#### Acceptance Criteria

1. THE Cargo_Skill SHALL eine Datei `.kiro/skills/cargo-workflow.md` im rs-summarizer-Projektverzeichnis erzeugen.
2. THE Cargo_Skill SHALL dokumentieren, wie Dependencies in `Cargo.toml` eingetragen werden (Format: `name = { version = "x.y", features = [...] }`).
3. THE Cargo_Skill SHALL dokumentieren, dass `cargo install cargo-edit` vor der Nutzung von `cargo upgrade` ausgeführt werden muss.
4. THE Cargo_Skill SHALL dokumentieren, dass `cargo upgrade --incompatible allow` die neuesten Versionsnummern in `Cargo.toml` setzt.
5. THE Cargo_Skill SHALL dokumentieren, dass vor der Implementierung DeepWiki MCP verwendet wird, um Informationen über GitHub-Dependencies zu erhalten (Format: `<organisation>/<projekt>`).
6. THE Cargo_Skill SHALL dokumentieren, dass für jede neue Dependency eine Zeile in eine `deps.md`-Datei geschrieben wird (Format: `<github-organisation>/<projekt>`).

---

### Requirement 2: Datenbank-Export-CLI

**User Story:** Als Nutzer möchte ich rs-summarizer mit einer CLI-Option aufrufen können, die eine kompakte Datenbank ohne Transkripte exportiert, damit ich eine kleine, portable Datei für die Visualisierung erhalte.

#### Acceptance Criteria

1. WHEN rs-summarizer mit dem Argument `export-db --source <quellpfad> --output <zielpfad>` aufgerufen wird, THE DB_Exporter SHALL eine neue SQLite-Datei am angegebenen Zielpfad erzeugen.
2. THE DB_Exporter SHALL in die Compact_DB folgende Felder aus der Source_DB kopieren: `identifier`, `original_source_link`, `model`, `embedding`, `embedding_model`, `summary`, `summary_timestamp_start`, `summary_timestamp_end`, `cost`, `timestamped_summary_in_youtube_format`.
3. THE DB_Exporter SHALL das Feld `transcript` NICHT in die Compact_DB kopieren.
4. THE DB_Exporter SHALL nur Zeilen exportieren, bei denen `embedding IS NOT NULL` und `summary_done = 1`.
5. THE DB_Exporter SHALL die Source_DB weder verändern noch löschen.
6. WHEN der Export abgeschlossen ist, THE DB_Exporter SHALL die Anzahl der exportierten Zeilen und die Dateigröße der Compact_DB in Bytes auf stdout ausgeben.
7. IF die Zieldatei bereits existiert, THEN THE DB_Exporter SHALL mit einer Fehlermeldung abbrechen, ohne die bestehende Datei zu überschreiben.
8. IF die Source_DB keine Zeilen mit `embedding IS NOT NULL AND summary_done = 1` enthält, THEN THE DB_Exporter SHALL mit einer informativen Fehlermeldung abbrechen.
9. THE Compact_DB SHALL WAL-Mode aktiviert haben (`PRAGMA journal_mode=WAL`).
10. IF die Source_DB-Datei nicht existiert oder nicht lesbar ist, THEN THE DB_Exporter SHALL mit einer Fehlermeldung abbrechen.
11. IF das übergeordnete Verzeichnis des Zielpfads nicht existiert, THEN THE DB_Exporter SHALL mit einer Fehlermeldung abbrechen.

---

### Requirement 3: Viz_Tool – Subprojekt-Struktur

**User Story:** Als Entwickler möchte ich ein eigenständiges Rust-Subprojekt im rs-summarizer-Workspace, das die Embedding-Visualisierung als Desktop-GUI bereitstellt.

#### Acceptance Criteria

1. THE Viz_Tool SHALL als Cargo-Workspace-Member in `rs-summarizer/Cargo.toml` eingetragen sein, sodass `[workspace]` den Eintrag `members = [".", "viz-tool"]` enthält und der bestehende `[package]`-Abschnitt erhalten bleibt.
2. THE Viz_Tool SHALL im Verzeichnis `rs-summarizer/viz-tool/` liegen und eine eigene `Cargo.toml` mit einem `[package]`-Abschnitt enthalten.
3. THE Viz_Tool SHALL `eframe` und `egui_plot` als Dependencies verwenden.
4. THE Viz_Tool SHALL `fast-umap` mit `features = ["gpu"]` als Dependency verwenden (WGPU-Backend für parametric UMAP mit Out-of-Sample-Projektion; läuft auch ohne dedizierte GPU via Software-Rendering).
5. THE Viz_Tool SHALL `linfa-clustering` als Dependency verwenden; die Version SHALL mit der transitiven `ndarray`-Version von `fast-umap` kompatibel sein.
6. THE Viz_Tool SHALL `sqlx` mit SQLite-Feature als Dependency verwenden, um die Compact_DB zu lesen.
7. THE Viz_Tool SHALL über eine `deps.md`-Datei verfügen, die alle GitHub-Repositories der Dependencies im Format `<github-organisation>/<projekt>` (eine Zeile pro Dependency) auflistet.

---

### Requirement 4: Viz_Tool – Daten laden

**User Story:** Als Nutzer möchte ich im Viz_Tool eine Compact_DB-Datei öffnen können, damit die Embeddings und Metadaten für die Visualisierung geladen werden.

#### Acceptance Criteria

1. WHEN das Viz_Tool mit einem CLI-Pfad-Argument gestartet wird, THE Viz_Tool SHALL die Compact_DB am angegebenen Pfad öffnen, ohne einen Datei-Öffnen-Dialog anzuzeigen.
2. WHEN das Viz_Tool ohne CLI-Pfad-Argument gestartet wird, THE Viz_Tool SHALL einen nativen Datei-Öffnen-Dialog anzeigen, der auf `.db`-Dateien gefiltert ist.
3. WHEN eine Compact_DB geladen wird, THE Viz_Tool SHALL alle Zeilen mit `embedding IS NOT NULL` aus der Tabelle lesen.
4. WHEN eine Compact_DB geladen wird, THE Viz_Tool SHALL den Embedding-BLOB als Little-Endian Float32-Array aus dem `embedding`-Feld deserialisieren und nur die ersten `embedding_dim` Elemente (konfigurierbar per CLI-Parameter, Standard: 768) verwenden.
5. WHEN eine Compact_DB geladen wird, THE Viz_Tool SHALL `original_source_link`, `summary`, `model`, `embedding_model` und `timestamped_summary_in_youtube_format` als Metadaten pro Punkt speichern.
6. WHEN das Laden abgeschlossen ist, THE Viz_Tool SHALL die Anzahl der erfolgreich geladenen Punkte in der GUI anzeigen.
7. IF die Datei nicht geöffnet werden kann oder die Tabelle `summaries` mit den Feldern `embedding`, `original_source_link`, `summary`, `model`, `embedding_model` nicht enthält, THEN THE Viz_Tool SHALL eine Fehlermeldung in der GUI anzeigen.
8. IF die Datei nicht lesbar ist oder ein I/O-Fehler auftritt, THEN THE Viz_Tool SHALL eine separate Fehlermeldung in der GUI anzeigen.
9. IF alle Zeilen in der Compact_DB `embedding IS NULL` haben, THEN THE Viz_Tool SHALL eine informative Meldung in der GUI anzeigen, dass keine Embeddings vorhanden sind.

---

### Requirement 5: Viz_Tool – UMAP-Dimensionsreduktion

**User Story:** Als Nutzer möchte ich die hochdimensionalen Embeddings mit UMAP auf 4D und 2D reduzieren können, damit ich die semantische Struktur der Daten visualisieren kann.

#### Acceptance Criteria

1. THE UMAP_Engine SHALL `fast-umap` mit CPU-Backend verwenden.
2. THE UMAP_Engine SHALL eine 4D-Reduktion mit konfigurierbaren Parametern `n_neighbors` (Standard: 5) und `min_dist` (Standard: 0.1) durchführen (analog zum Python-Prototyp: `UMAP(n_neighbors=5, min_dist=.1, n_components=4)`).
3. THE UMAP_Engine SHALL eine separate 2D-Reduktion mit konfigurierbaren Parametern `n_neighbors` (Standard: 12) und `min_dist` (Standard: 0.13) durchführen (analog zum Python-Prototyp: `UMAP(n_neighbors=12, min_dist=.13, n_components=2)`).
4. THE Viz_Tool SHALL in der GUI Schieberegler für `n_neighbors` (Bereich: 2–200) und `min_dist` (Bereich: 0.0–1.0) für beide UMAP-Konfigurationen anzeigen.
5. WHEN der Nutzer auf "Berechnen" klickt, THE UMAP_Engine SHALL die Reduktion mit den aktuellen GUI-Parametern neu berechnen.
6. WHEN die UMAP-Berechnung läuft, THE Viz_Tool SHALL einen Fortschrittsindikator in der GUI anzeigen und den "Berechnen"-Button deaktivieren.
7. THE UMAP_Engine SHALL die 4D-Embeddings als Eingabe für das nachfolgende DBSCAN-Clustering bereitstellen.
8. THE UMAP_Engine SHALL die 2D-Embeddings für die Scatter-Plot-Visualisierung bereitstellen.
9. IF die UMAP-Berechnung fehlschlägt (z.B. weil `n_neighbors` ≥ Anzahl geladener Punkte), THEN THE Viz_Tool SHALL eine Fehlermeldung in der GUI anzeigen und das vorherige Ergebnis beibehalten.

---

### Requirement 6: Viz_Tool – DBSCAN-Clustering

**User Story:** Als Nutzer möchte ich die 4D-UMAP-Embeddings mit DBSCAN clustern können, damit semantisch ähnliche Videos gruppiert werden.

#### Acceptance Criteria

1. THE DBSCAN_Engine SHALL `linfa-clustering` verwenden.
2. THE DBSCAN_Engine SHALL auf den 4D-UMAP-Embeddings operieren (nicht auf den Roh-Embeddings).
3. THE DBSCAN_Engine SHALL konfigurierbare Parameter `eps` (Standard: 0.3, gültiger Bereich: > 0.0) und `min_samples` (Standard: 5, gültiger Bereich: ≥ 1) unterstützen.
4. THE Viz_Tool SHALL in der GUI Eingabefelder für `eps` und `min_samples` anzeigen; IF ein ungültiger Wert eingegeben wird, THEN THE Viz_Tool SHALL das Feld rot markieren und das Clustering deaktivieren.
5. WHEN der Nutzer auf "Clustern" klickt, THE DBSCAN_Engine SHALL das Clustering mit den aktuellen GUI-Parametern ausführen.
6. IF keine 4D-UMAP-Embeddings berechnet wurden, THEN THE Viz_Tool SHALL den "Clustern"-Button deaktivieren und einen Hinweis anzeigen.
7. WHEN das DBSCAN-Clustering abgeschlossen ist, THE DBSCAN_Engine SHALL jedem Punkt ein Cluster-Label zuweisen (Rauschen = -1).
8. WHEN das DBSCAN-Clustering abgeschlossen ist, THE Viz_Tool SHALL die Anzahl der gefundenen Cluster (ohne Rauschen) in der GUI anzeigen.
9. THE Viz_Tool SHALL Rauschpunkte (Label -1) im Scatter-Plot visuell von Cluster-Punkten unterscheiden (z.B. in Grau).

---

### Requirement 7: Viz_Tool – Scatter-Plot-Visualisierung

**User Story:** Als Nutzer möchte ich die 2D-UMAP-Projektion als interaktiven Scatter-Plot sehen, damit ich die Cluster-Struktur explorieren kann.

#### Acceptance Criteria

1. THE Viz_Tool SHALL die 2D-UMAP-Embeddings als Scatter-Plot mit `egui_plot` darstellen.
2. THE Viz_Tool SHALL jeden Punkt entsprechend seinem Cluster-Label einfärben; IF die Anzahl der Cluster die Palettengröße überschreitet, THEN SHALL die Farben zyklisch wiederholt werden; Rauschpunkte (Label -1) werden immer in Grau dargestellt.
3. WHEN der Nutzer mit der Maus innerhalb von 5 Pixeln eines Punktes fährt, THE Viz_Tool SHALL einen Tooltip mit `original_source_link` und der vollständigen Zusammenfassung anzeigen.
4. THE Viz_Tool SHALL Zoom und Pan im Scatter-Plot unterstützen.
5. WHEN Cluster-Titel vorhanden sind, THE Viz_Tool SHALL den Cluster-Titel am Zentroid (arithmetisches Mittel der 2D-Koordinaten aller Punkte des Clusters) des jeweiligen Clusters im Plot anzeigen.
6. WHEN noch keine UMAP-Berechnung durchgeführt wurde, THE Viz_Tool SHALL anstelle des Plots einen Hinweistext anzeigen.

---

### Requirement 8: Viz_Tool – Cluster-Betitelung via Gemini/Gemma

**User Story:** Als Nutzer möchte ich automatisch generierte Titel für jeden Cluster erhalten, damit ich die semantischen Themen der Cluster auf einen Blick verstehen kann.

#### Acceptance Criteria

1. WHEN der Nutzer auf "Cluster-Titel generieren" klickt, THE Cluster_Titler SHALL die Gemini/Gemma API aufrufen.
2. THE Cluster_Titler SHALL für jeden Cluster (Label ≠ -1) bis zu 3 Beispiel-Zusammenfassungen an die API senden; jede Zusammenfassung wird auf den Abstract-Block reduziert (Text zwischen dem ersten `**Abstract**`-Marker und dem nächsten `**`-Marker; fehlt der Marker, wird die gesamte Zusammenfassung verwendet); Cluster mit weniger als 3 Punkten senden alle verfügbaren Zusammenfassungen.
3. THE Cluster_Titler SHALL das erste Modell in der konfigurierten Modellliste verwenden, dessen `rpd_limit` dem höchsten `rpd_limit` aller konfigurierten Modelle entspricht.
4. THE Cluster_Titler SHALL alle Cluster-Prompts in einem einzigen API-Aufruf bündeln, sofern die Gesamtlänge 20.000 Wörter nicht überschreitet; andernfalls SHALL die Prompts in mehrere Aufrufe aufgeteilt werden.
5. THE Cluster_Titler SHALL den GEMINI_API_KEY aus der Umgebungsvariable lesen.
6. THE Cluster_Titler SHALL die API-Antwort als strukturiertes JSON mit Schema `[{"id": <cluster_id>, "title": "<titel>"}]` anfordern.
7. WHEN die Titel generiert wurden, THE Cluster_Titler SHALL die Titel persistent in einer JSON-Datei neben der Compact_DB speichern (Dateiname: `<compact_db_name>_cluster_titles.json`).
8. WHEN eine gespeicherte Titel-Datei existiert, THE Viz_Tool SHALL die Titel beim Start automatisch laden, ohne die API erneut aufzurufen.
9. IF die API nicht erreichbar ist, einen Nicht-2xx-Status zurückgibt, eine ungültige JSON-Antwort liefert oder der API-Key leer ist, THEN THE Cluster_Titler SHALL eine spezifische Fehlermeldung in der GUI anzeigen und die Visualisierung ohne Titel fortsetzen.

---

### Requirement 9: Viz_Tool – Parametric UMAP (NN_Mapper)

**User Story:** Als Entwickler möchte ich das UMAP-Mapping in ein neuronales Netz trainieren, damit neue Embeddings stabil in das bestehende 2D-Layout eingebettet werden können.

#### Acceptance Criteria

1. THE NN_Mapper SHALL `fast-umap` im parametric Modus verwenden.
2. IF kein 2D-UMAP-Layout berechnet wurde, THEN THE Viz_Tool SHALL den "NN-Mapper trainieren"-Button deaktivieren und einen Hinweis anzeigen.
3. WHEN der Nutzer auf "NN-Mapper trainieren" klickt und ein 2D-UMAP-Layout vorhanden ist, THE NN_Mapper SHALL das neuronale Netz auf den aktuellen Embeddings und dem aktuellen 2D-UMAP-Layout trainieren und einen Fortschrittsindikator anzeigen.
4. WHEN das Training abgeschlossen ist, THE NN_Mapper SHALL das trainierte Modell in einer Datei neben der Compact_DB speichern (Dateiname: `<compact_db_name>_nn_mapper.bin`).
5. WHEN ein gespeichertes NN-Mapper-Modell existiert, THE Viz_Tool SHALL das Modell beim Start automatisch laden.
6. IF eine gespeicherte Modelldatei nicht geladen werden kann (z.B. korrupt oder inkompatibel), THEN THE Viz_Tool SHALL eine Fehlermeldung anzeigen und ohne geladenes Modell fortfahren.
7. WHEN ein neues Embedding mit genau `embedding_dim` Float32-Werten übergeben wird, THE NN_Mapper SHALL die 2D-Koordinaten für diesen Punkt berechnen, ohne das gesamte UMAP neu zu berechnen.
8. IF ein Embedding mit einer anderen Länge als `embedding_dim` Float32-Werten übergeben wird, THEN THE NN_Mapper SHALL einen Fehler zurückgeben.
9. THE NN_Mapper SHALL eine Rust-Funktion `project(embedding: &[f32]) -> Result<(f32, f32), Error>` exportieren, die von anderen Komponenten (z.B. Web_Viz) genutzt werden kann.

---

### Requirement 10: Web-Frontend – 2D-UMAP-Karte bei Summary-Detail

**User Story:** Als Nutzer möchte ich bei der Anzeige einer Zusammenfassung eine interaktive 2D-UMAP-Karte sehen, die das aktuelle Video im Kontext aller anderen Videos und ihrer Cluster zeigt, damit ich thematisch verwandte Inhalte entdecken und direkt zu ihnen navigieren kann.

#### Acceptance Criteria

1. WHEN eine Summary-Detailseite geladen wird und ein NN_Mapper-Modell geladen ist, THE Web_Viz SHALL eine interaktive 2D-UMAP-Karte (SVG oder D3.js) rendern, die alle in der Datenbank gespeicherten 2D-Projektionen als Punkte darstellt.
2. WHEN die 2D-Karte gerendert wird, THE Web_Viz SHALL das Embedding des aktuellen Videos über den NN_Mapper auf 2D projizieren und diesen Punkt visuell hervorgehoben (z.B. größer, anderer Farbton) darstellen.
3. WHEN die 2D-Karte gerendert wird, THE Web_Viz SHALL alle Punkte entsprechend ihrem gespeicherten Cluster-Label einfärben (gleiche Farbpalette wie im Viz_Tool); Rauschpunkte werden in Grau dargestellt.
4. WHEN Cluster-Titel in der Datenbank gespeichert sind, THE Web_Viz SHALL die Cluster-Titel am jeweiligen Zentroid in der Karte anzeigen.
5. WHEN der Nutzer auf einen Punkt in der Karte klickt, THE Web_Viz SHALL zur Summary-Detailseite des entsprechenden Videos navigieren.
6. WHEN der Nutzer mit der Maus über einen Punkt fährt, THE Web_Viz SHALL einen Tooltip mit `original_source_link` und der Zusammenfassung des entsprechenden Videos anzeigen.
7. THE Web_Viz SHALL die 5 nächsten Nachbarn des aktuellen Videos (euklidische Distanz im 2D-Raum) visuell hervorgehoben darstellen und als klickbare Links unterhalb der Karte auflisten.
8. IF kein NN_Mapper-Modell geladen ist, THEN THE Web_Viz SHALL den Karten-Abschnitt nicht anzeigen.
9. IF das aktuelle Video kein Embedding hat, THEN THE Web_Viz SHALL den Karten-Abschnitt nicht anzeigen.

---

### Requirement 11: Web-Frontend – Suchergebnis-Visualisierung

**User Story:** Als Nutzer möchte ich Suchergebnisse im 2D-Embedding-Space visualisiert sehen, damit ich die semantische Verteilung der Treffer verstehen kann.

#### Acceptance Criteria

1. WHEN eine Suchanfrage Ergebnisse liefert und ein NN_Mapper-Modell geladen ist, THE Web_Viz SHALL einen 2D-Scatter-Plot der Suchergebnisse im UMAP-Raum anzeigen.
2. WHEN Suchergebnisse visualisiert werden, THE Web_Viz SHALL die Embeddings der Suchergebnisse über den NN_Mapper auf 2D projizieren.
3. THE Web_Viz SHALL die Suchergebnisse als Punkte im Plot darstellen, wobei die Farbe den Cosinus-Ähnlichkeitsabstand zur Suchanfrage kodiert (höhere Ähnlichkeit = wärmere Farbe).
4. THE Web_Viz SHALL den Plot als interaktives SVG-Element im HTMX-Frontend einbetten.
5. WHEN der Nutzer auf einen Punkt im Plot klickt, THE Web_Viz SHALL zur entsprechenden Summary-Detailseite navigieren.
6. IF kein NN_Mapper-Modell geladen ist, THEN THE Web_Viz SHALL den Visualisierungs-Abschnitt in den Suchergebnissen nicht anzeigen.
7. WHEN Suchergebnisse visualisiert werden, THE Web_Viz SHALL den Anfrage-Punkt (projiziertes Embedding der Suchanfrage) visuell hervorgehoben (z.B. als Stern oder größerer Punkt) im Plot darstellen.

---

### Requirement 12: Round-Trip-Korrektheit der Embedding-Serialisierung

**User Story:** Als Entwickler möchte ich sicherstellen, dass Embeddings verlustfrei zwischen SQLite-BLOB und Float32-Array konvertiert werden, damit keine Präzisionsfehler die Visualisierung verfälschen.

#### Acceptance Criteria

1. THE Viz_Tool SHALL Embedding-BLOBs als Little-Endian Float32-Arrays deserialisieren.
2. THE Viz_Tool SHALL sicherstellen, dass das Deserialisieren und anschließende Re-Serialisieren eines gültigen Embedding-BLOBs das identische Byte-Array erzeugt (Round-Trip-Eigenschaft).
3. THE Viz_Tool SHALL nach der Deserialisierung nur die ersten `embedding_dim` Elemente (konfigurierbar per CLI-Parameter, Standard: 768, Maximum: 3072) für alle weiteren Berechnungen verwenden.
4. IF ein BLOB eine Länge hat, die kein Vielfaches von 4 Bytes ist, THEN THE Viz_Tool SHALL diesen Eintrag überspringen, eine Warnung auf stderr ausgeben und das Laden der übrigen Einträge fortsetzen.
5. IF ein BLOB weniger als `embedding_dim * 4` Bytes enthält, THEN THE Viz_Tool SHALL diesen Eintrag überspringen, eine Warnung auf stderr ausgeben und das Laden der übrigen Einträge fortsetzen.
6. WHEN das Laden abgeschlossen ist, THE Viz_Tool SHALL die Anzahl der erfolgreich deserialisierten Embeddings anzeigen (übersprungene Einträge werden nicht mitgezählt).
