for i in Cargo.toml \
	     src/*/*.rs \
	     src/*.rs \
	     config/models.json \
	     templates/*.html \
	     migrations/*.sql \
	     tests/*.rs \
	     doc/rate*.md \
	     R*.md
do
    echo "// start of "$i
    cat $i
done

cat <<EOF

schreibe plan/20261115_01_update/plan.md um eine (duemmere) AI zu
instruieren, den quellcode von rs-summarizer upzudaten. im folgenden
sind einige informationen, die du dem ai agenten mitteilen sollst.

nach der aenderung und tests soll ein walkthrough dokument geschrieben
werden und die commits sollen mittels des dokumentierten release
protokolls auf github veroeffentlicht werden.

ich moechte auch dass von nun an ein CHANGELOG.md gefuehrt wird und
ueber den artefakten die jeweiligen neuerungen in jeder release
aufgelistet werden. der release workflow soll dementsprechend
angepasst und dokumentiert werden.

ich habe die folgende tabelle von hier bezogen: 
https://aistudio.google.com/rate-limit?timeRange=last-1-day&project=noted-processor-418406

Hier ist die gefilterte Markdown-Tabelle mit allen Modellen der
Kategorie **Text-out models**, die ein tägliches Quota (`RPD > 0`)
haben:

| Model | Category | RPM | TPM | RPD |
| :--- | :--- | :--- | :--- | :--- |
| Gemini 2.5 Flash | Text-out models | 0 / 5 | 0 / 250K | 0 / 20 |
| Gemini 2.5 Flash Lite | Text-out models | 0 / 10 | 0 / 250K | 0 / 20 |
| Gemini 3 Flash | Text-out models | 0 / 5 | 0 / 250K | 0 / 20 |
| Gemini 3.1 Flash Lite | Text-out models | 6 / 15 | 32.76K / 250K | 0 / 500 |
| Gemini 3.5 Flash Lite | Text-out models | 1 / 15 | 27.32K / 250K | 4 / 500 |
| Gemini 3.6 Flash | Text-out models | 0 / 5 | 0 / 250K | 0 / 20 |
| Gemini 3.8 Flash | Text-out models | 0 / 5 | 0 / 250K | 0 / 20 |
| Gemini 4 Hydrogen | Text-out models | 0 / 5 | 0 / 250K | 0 / 20 |

Hier sind alle übrigen Modelle (außer *Text-out models*) aus der
Tabelle, die ein tägliches Quota (`RPD > 0` oder `Unlimited`) haben:

### Übrige Modelle

| Model | Category | RPM | TPM | RPD |
| :--- | :--- | :--- | :--- | :--- |
| Antigravity | Agents | 0 / 60 | 0 / 100K | 0 / 100 |
| Gemini 2.5 Flash Native Audio Dialog | Live API | 0 / Unlimited | 0 / 1M | 0 / Unlimited |
| Gemini 2.5 Flash TTS | Multi-modal generative models | 0 / 3 | 0 / 10K | 0 / 10 |
| Gemini 3 Flash Live | Live API | 0 / Unlimited | 0 / 65K | 0 / Unlimited |
| Gemini 3.1 Flash TTS | Multi-modal generative models | 0 / 3 | 0 / 10K | 0 / 10 |
| Gemini 3.5 Live Translate | Live API | 0 / Unlimited | 0 / 20K | 0 / Unlimited |
| Gemini 3.5 Transcribe | Live API | 0 / 3 | 0 / 10K | 0 / 25 |
| Gemini 3.5 Transcribe Live | Live API | 0 / Unlimited | 0 / 20K | 0 / Unlimited |
| Gemini 3.8 Flash Lite TTS | Multi-modal generative models | 0 / 3 | 0 / 10K | 0 / 10 |
| Gemini 3.8 Flash TTS | Multi-modal generative models | 0 / 3 | 0 / 10K | 0 / 10 |
| Gemini 3.8 Live | Live API | 0 / Unlimited | 0 / 65K | 0 / Unlimited |
| Gemini 3.8 Live Extended Thinking | Live API | 0 / Unlimited | 0 / 65K | 0 / Unlimited |
| Gemini Decision 1 | Decisions API | 0 / 200 | 0 / 50K | 0 / 10K |
| Gemini Embedding 1 | Other models | 1 / 100 | 1.34K / 30K | 3 / 1K |
| Gemini Embedding 2 | Other models | 0 / 100 | 0 / 30K | 0 / 1K |
| Gemini Robotics ER 2 Preview | Other models | 0 / 5 | 0 / 250K | 0 / 20 |
| Gemma 4 26B | Other models | 0 / 30 | 0 / 16K | 0 / 14.4K |
| Gemma 4 31B | Other models | 0 / 30 | 0 / 16K | 0 / 14.4K |

### Tools (mit täglichem Quota)

| Model | Category | RPM | TPM | RPD |
| :--- | :--- | :--- | :--- | :--- |
| Default | Search grounding | - | - | 0 / 1.5K |
| Gemini 2 | Search grounding | - | - | 0 / 1.5K |
| Gemini 2.5 | Search grounding | - | - | 0 / 1.5K |

Hier sind die zur zeit auf dem hetzner experiment gehosteten modelle:

{"object":"list","data":[{"id":"Qwen/Qwen3.6-35B-A3B","object":"model","created":1791470824,"owned_by":"hetzner","root":"s3://models/qwen36-35b-a3b-fp8","parent":null,"max_model_len":262144},{"id":"Qwen/Qwen3.6-35B-A3B-FP8","object":"model","created":1791470824,"owned_by":"hetzner","root":"s3://models/qwen36-35b-a3b-fp8","parent":null,"max_model_len":262144},{"id":"Qwen/Qwen3.8-27B","object":"model","created":1791274843,"owned_by":"hetzner","root":"s3://models/qwen38-27b-fp8","parent":null,"max_model_len":262144},{"id":"Qwen3.8-27B","object":"model","created":1791274843,"owned_by":"hetzner","root":"s3://models/qwen38-27b-fp8","parent":null,"max_model_len":262144}]}


In der Webanwendung `rs-summarizer` (Produktivinstanz: https://rocketrecap.com)
sollen die verfuegbaren modelle entsprechend upgedated werden


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
