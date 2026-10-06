for i in Cargo.toml \
	     src/*/*.rs \
	     src/*.rs \
	     config/models.json \
	     templates/*.html \
	     migrations/*.sql \
	     tests/{*.rs,*/*.vtt} 
do
    echo "// start of "$i
    cat $i
done

cat <<EOF

review this code. suggest improvements and
simplifications. schreibe das folgende prompt / vervollstaendige es,
so dass ein agent die aufgabe uebernehmen kann.

ich habe bereits ein finding gelistet, dass ich gefunden habe.

behalte die prompt struktur mit spezifisch aufgabe und
wiederverwendbaren generellem teil bei.

ich bin mir mit dem nummerieren der quelldateien nicht sicher, ich
moechte keinen grossen umbau hervorrufen.

## Bemerkungen:
Der hardcoded hetzner key in src/services/summary.rs kann ignoriert
werden. Im produktivsystem wird er ueber env variable uebermittelt.
Lass die hardcoded variante trotzdem noch da stehen.

## Aufgabe

Ich habe festgestellt, dass die Summaries auf
https://rocketrecap.com/browse verdoppelt dargestellt werden.  Einmal
reicht. Du koenntest dort auch noch die Bearbeitungszeit darstellen
und den Preis auf 2 Stellen runden. Vielleicht koennte man noch mehr
nuetzliche informationen anzeigen (aber z.b. nicht die eingehende IP).

Wenn die resultate auf rocketrecap.com/ direkt angezeigt werden fehlen
bisher der originale link und es ist auch nicht moeglich eine
Bewertung vorzunehmen. Das sollte dort genauso gehen wie auf
/browse. Dass die Reihenfolge der resultate dort umgekehrt ist, ist
akzeptierbar.

## Daten

In /workspace/src/rs-summarizer/summaries.db kannst du eine Kopie vom
Zustand der Datenbank des Produktivsystems sehen.

## Rolle & Umgebung:

Du läufst in einem Docker-Image mit Ubuntu auf einem Linux-Host.
Falls dir Pakete fehlen, installiere sie selbstständig. Du arbeitest
im Auftrag von wol pumba (wolpumba@gmail.com).


* Docker-Referenz: /workspace/src/cl-cl-generator/example/05_dockerfile_meta/source01/examples/03_ai_env/Dockerfile.

Validiere die Ergebnisse und fuehre falls noetig Benchmarkmessungen
durch.


## Code-Anforderungen

Ich moechte kleinen/uebersichtlichen und effizienten Code und
moeglichst wenig Abhaengigkeiten. Du sollst in dieser Aufgabe direkt
Rust-Code erzeugen.

Vorgehensweise: Arbeite mit aktuellem Rust (2024). Nutze Tools um
konsistenten und aufgeraeumten Quellcode zu gewaehrleisten (also
"cargo fmt" um Formatierung zu normalisieren, "cargo clippy" um
moegliche Fehler oder Verbesserungen zu finden, "cargo upgrade" um bei
den Abhaengigkeiten die neueste Version zu verwenden, ...).

Jedes Beispiel-Crate in diesem Repo steht fuer sich (kein Workspace-
"Cargo.toml" im Repo-Root); "target/" und "*.lock" sind per
".gitignore" ausgeschlossen.

## Externe Dokumentation & DeepWiki MCP

* Führe eine "deps.md", in der alle Abhängigkeiten in der Notation "<organization>/<project>" erfasst sind.
* Nutze das DeepWiki MCP (z. B. für "NVIDIA/cuda-rust" oder GPU-Crates), um gezielt nach Dokumentation und Architektur-Details zu suchen.
* Fuer das akutelle Repo kannst du informationen in deepwiki mcp finden, wenn du nach "plops/rs-summarizer" fragst.

## Tests

Neue Unit-Tests und Integration-Tests sollen wie erforderlich
eingefuehrt werden. Diese sollen auch ausgefuehrt werden. Zusaetzlich
soll es Regressionstests auf dem echten Datensatz geben.

Du kannst auch Puppetteer oder Browser Automation Crates installieren,
um die Webseitenfunktionalitaet zu testen.

## Plan

Schreibe einen Implementierungsplan. Dieser Implementierungsplan soll
einen unabhaengigen AI-Agenten dazu befaehigen, selbstaendig einen
Kontext aufzubauen, der die wesentlichen Quellen beinhaltet. Das
heisst, der Implementierungsplan hat eine Liste von Dateien, die der
Agent sich anschauen sollte, mit einer kurzen Beschreibung fuer jede
Datei. Weiterhin soll der Implementierungsplan erklaeren, wie Commit
Messages gesendet werden sollen. Und zwar sollen diese mit einer
umfassenden Beschreibung und als Conventional Commit formatiert
ausgefuehrt werden.

Lege den Plan als "plan.md" neben diese "prompt.txt" in
"plan/2026.../".

## Walkthrough

Nach der vollstaendigen Implementierung, wenn alle Tests abgeschlossen
sind, soll auch ein Walkthrough-Dokument geschrieben und als
"walkthrough.md" im selben Ordner (wo prompt.txt ist) abgelegt
werden. Dieses soll zusammenfassen, was wirklich implementiert wurde,
welche Stellen aufgrund der Tests anders gemacht wurden als geplant,
dazu Learnings und moegliche Erweiterungen. Liste auch die neuen
Programme, die mit in den Docker-Container aufgenommen werden sollten.

Lege fuer den ausfuehrenden Agenten folgende strikte Regeln fuer das
Schreiben dieses Dokuments fest:
- **Sprache und Stil:** Das Dokument *muss zwingend auf Deutsch*
  verfasst sein. Es soll den Leser didaktisch gut abholen, mitnehmen
  und fluessig lesbar sein.
- **Erklaerungen:** Fachbegriffe muessen kurz und verstaendlich
  erklaert werden.
- **Visualisierung:** Verwende reichlich Code-Beispiele und nutze
  **Mermaid-Diagramme**, um die Architektur, den Datenfluss und
  komplexe Konzepte visuell zu veranschaulichen.
- **Inhaltliche Struktur:**
  1. Was exakt implementiert wurde.
  2. Welche Architektur-Entscheidungen aufgrund von Tests spontan
     geaendert werden mussten.
  3. Learnings und moegliche zukuenftige Erweiterungen.
  4. Eine Liste der neuen Programme/Pakete, die dauerhaft in das
     Dockerfile aufgenommen werden sollten.

## Aufwand

Halte den Token-Verbrauch der Session in "plan_effort.md" im selben
Ordner fest (Input/Output/Cached/Total, Turns, Subagents), damit
spaeter abschaetzbar ist, was ein solcher Port kostet.


EOF
