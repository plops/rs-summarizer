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
kiel@e14:~/stage/rs-summarizer$ ls scripts/
apply_submodule_patches.sh  migrate.py
build_extension.py          release-check.sh
create_synthetic_db.py      release.sh
export_viewer               run_release_gpu_gui.sh
install_po_provider.sh      sqlite-backup.sh
kiel@e14:~/stage/rs-summarizer$ ls static/
htmx.min.js  pico.min.css  ratings.css
kiel@e14:~/stage/rs-summarizer$ ls extension/
README.md  images  manifest.json  popup.html  popup.js
kiel@e14:~/stage/rs-summarizer$ ls doc
api_experiments.md  rate_limiting_error_handling_architecture.md
qwen_eval.md        subtitles.md
kiel@e14:~/stage/rs-summarizer$ ls plan
20260513_01_port_from_python  20260817_01_more_hetzner_models
20260619_01_review_vps        20260818_01_po_provider
20260720_01_hn                20260822_01_hetzner_reduce
20260721_01_gemini36          20260830_01_archify
20260722_01_hn_batch          20260904_01_thinking_level
20260722_02_code_review       20260904_02_aborts
20260723_01_improve_prompt    20260904_03_version
20260724_01_star_rating       20260920_01_doc
20260724_02_split_multi       20261006_01_review
20260726_01_hetzner           20261008_01_local_storage
20260726_02_loadbalance       20261023_01_all_entries_browser
20260801_01_glossary          20261024_01_titles
20260813_01_gemini37
kiel@e14:~/stage/rs-summarizer$ ls prompts
collect_project_info.sh  example_output_abstract.txt
example_input.txt        system_instruction.txt
example_output.txt       visual.txt
kiel@e14:~/stage/rs-summarizer$ ls templates/
browse.html        generation_partial.html  search_results.html
entries.html       index.html
entry_detail.html  rating_partial.html

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
