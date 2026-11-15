for i in Cargo.toml \
	     src/*/*.rs \
	     src/*.rs \
	     config/models.json \
	     templates/*.html \
	     migrations/*.sql \
	     tests/*.rs \
	     doc/rate*.md \
	     R*.md \
	     templates/*.html
do
    echo "// start of "$i
    cat $i
done

cat <<'EOF'

In der Webanwendung `rs-summarizer` (Produktivinstanz: https://rocketrecap.com)
sollen die verfuegbaren modelle entsprechend upgedated werden


Ich habe einen backup der datenbank vom produktivsystem abgezogen und
unter /workspace/src/rs-summarizer/summaries_20261115.db.zst
abgelegt. eine weitere aufgabe fuer den agenten ist die anzahl der
bisher nachgetragenen youtube video titel zu bestimmen

---

## Rolle & Umgebung

Du läufst in einem Docker-Image mit Ubuntu auf einem Linux-Host.
Dieses ist definiert in
/workspace/src/cl-cl-generator/example/05_dockerfile_meta/source01/examples/03_ai_env/Dockerfile

Falls dir Pakete oder CLI-Tools fehlen, installiere sie
selbstständig. Du arbeitest im Auftrag von wol pumba
(wolpumba@gmail.com).

Validiere alle Ergebnisse und führe falls nötig Benchmarkmessungen durch.

---

## Code-Anforderungen & Architektur

* **Sprache:** Idiomatisches, modernes Rust (Edition 2024).
* **Qualitätsstandards:** Führe `cargo fmt` und `cargo clippy --all-targets --all-features` aus. Es dürfen keine Warnings verbleiben.
* **Abhängigkeiten:** Keine unnötigen neuen Crates einführen.

---

## Externe Dokumentation & DeepWiki MCP

* Führe eine `deps.md`, falls du Abhängigkeiten einführst, in der Notation
  `<organization>/<project>`.
* Für das aktuelle Repository findest du Kontext und Architekturdetails im DeepWiki MCP unter `plops/rs-summarizer`.

---

## Tests

[...]
---

## Plan

Erstelle vor der Implementierung einen detaillierten Implementierungsplan unter `plan/20261115_01_update/plan.md`.
Dieser Plan soll:
1. Eine präzise Liste aller beteiligten Dateien enthalten, inklusive einer kurzen Beschreibung der geplanten Änderungen pro Datei.
2. Den schrittweisen Ablauf der Implementierung und Verifikation beschreiben.
3. Die Git-Commit-Strategie definieren: Alle Commits müssen als **Conventional Commits** mit aussagekräftigem Body formatiert sein.

---

## Walkthrough

Nach erfolgreicher Implementierung und Bestehen aller Tests erstellst
du ein Walkthrough-Dokument unter
`plan/20261024_01_titles/walkthrough.md`.

**Strikte Regeln für das Walkthrough:**
* **Sprache & Stil:** Zwingend auf Deutsch verfasst, verständlich, didaktisch hochwertig und flüssig lesbar.
* **Erklärungen:** Fachbegriffe kurz und prägnant erklären.
* **Visualisierung:** Nutze Code-Beispiele und **Mermaid-Diagramme** (z. B. zur Veranschaulichung von Titel-Extraktion, Budget und Backfill-Loop).
* **Inhaltliche Struktur:**
  1. Was exakt implementiert und behoben wurde.
  2. Architektur- und Designentscheidungen (insbesondere Abweichungen vom ursprünglichen Plan).
  3. Learnings und Vorschläge für zukünftige Erweiterungen.
  4. Liste eventuell neu benötigter Programme/Pakete für das Dockerfile.

---

## Aufwand

Halte den Token-Verbrauch und Aufwand der Session in
`plan/20261115_01_update/plan_effort.md` fest
(Input/Output/Cached/Total Tokens, Turns, Subagents), um die Kosten
für spätere Durchläufe transparent zu dokumentieren.


EOF
