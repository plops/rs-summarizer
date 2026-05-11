#!/usr/bin/env bash
set -euo pipefail

# collect_project_info.sh
# Usage: ./collect_project_info.sh [output-file]
# Default output: prompts/collected_project_info.txt (next to this script)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT="${1:-$SCRIPT_DIR/collected_project_info.txt}"

MAX_LINES=1000
HEAD_TAIL=250

echo "Collecting important files from $REPO_ROOT" > "$OUT"
echo "Generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")" >> "$OUT"
echo >> "$OUT"

append_file() {
  local file="$1"
  local rel
  # compute relative path (best-effort)
  if [[ "$file" == "$REPO_ROOT"* ]]; then
    rel="${file#$REPO_ROOT/}"
  else
    rel="$file"
  fi

  echo "// start von $rel" >> "$OUT"

  if [ ! -f "$file" ]; then
    echo "// file not found: $rel" >> "$OUT"
    echo >> "$OUT"
    return
  fi

  local lines
  lines=$(wc -l < "$file" 2>/dev/null || echo 0)
  if [ "$lines" -gt "$MAX_LINES" ]; then
    echo "// Datei ist gross ($lines Zeilen) — schreibe Kopf und Ende" >> "$OUT"
    head -n "$HEAD_TAIL" "$file" >> "$OUT"
    echo >> "$OUT"
    echo "// ... $(($lines - 2 * $HEAD_TAIL)) Zeilen in der Mitte ausgelassen ..." >> "$OUT"
    echo >> "$OUT"
    tail -n "$HEAD_TAIL" "$file" >> "$OUT"
    echo >> "$OUT"
  else
    cat "$file" >> "$OUT"
    echo >> "$OUT"
  fi
}

# Build a list of files to include (ordered preference)
files=()

# .kiro cargo-workflow skill(s)
if [ -f "$REPO_ROOT/.kiro/skills/cargo-workflow.md" ]; then
  files+=("$REPO_ROOT/.kiro/skills/cargo-workflow.md")
fi
if [ -f "$REPO_ROOT/.kiro/skills/cargo-workflow/SKILL.md" ]; then
  files+=("$REPO_ROOT/.kiro/skills/cargo-workflow/SKILL.md")
fi

# .kiro spec/docs for embedding visualization
if [ -d "$REPO_ROOT/.kiro/specs" ]; then
  while IFS= read -r f; do
    files+=("$f")
  done < <(find "$REPO_ROOT/.kiro/specs" -type f -maxdepth 4 2>/dev/null | sort)
fi

# workspace and viz-tool
for p in \
  "$REPO_ROOT/Cargo.toml" \
  "$REPO_ROOT/Cargo.lock" \
  "$REPO_ROOT/viz-tool/Cargo.toml" \
  "$REPO_ROOT/viz-tool/deps.md" \
  "$REPO_ROOT/viz-tool/AGENTS.md" \
  "$REPO_ROOT/viz-tool/src/viz_app.rs"; do
  if [ -f "$p" ]; then files+=("$p"); fi
done

# .github workflows
if [ -d "$REPO_ROOT/.github/workflows" ]; then
  while IFS= read -r f; do files+=("$f"); done < <(find "$REPO_ROOT/.github/workflows" -type f \( -name "*.yml" -o -name "*.yaml" \) 2>/dev/null | sort)
fi

# Grep for notable keywords (egui, egui_plot, Plot::new, on_hover_text, deepwiki, mcp, deps.md)
mapfile -t grep_files < <(grep -RIl --exclude-dir=.git --exclude-dir=target --exclude-dir=node_modules -e "egui_plot" -e "egui" -e "Plot::new" -e "on_hover_text" -e "label_formatter" -e "deepwiki" -e "DeepWiki" -e "\bmcp\b" -e "deps.md" "$REPO_ROOT" 2>/dev/null || true)
for g in "${grep_files[@]:-}"; do
  # avoid adding duplicates
  files+=("$g")
done

# Deduplicate while preserving order
uniq_files=()
declare -A seen
for f in "${files[@]}"; do
  if [ -z "${seen[$f]:-}" ]; then
    uniq_files+=("$f")
    seen[$f]=1
  fi
done

# Append found files
for f in "${uniq_files[@]}"; do
  append_file "$f"
done

# If there were no egui examples found, append small examples/templates
if ! grep -RIl --exclude-dir=.git --exclude-dir=target -e "egui" "$REPO_ROOT" >/dev/null 2>&1; then
  echo "// start von <generated>/egui_widget_example.rs" >> "$OUT"
  cat <<'EGUI_EXAMPLE' >> "$OUT"
// Minimal eframe/egui example showing widgets with tooltip
use eframe::egui;

pub fn ui_example(ui: &mut egui::Ui) {
    let n_neighbors = 12;
    let min_dist = 0.1_f32;

    ui.horizontal(|ui| {
        if ui.button("Compute").clicked() {
            // trigger compute
        }
        ui.label(egui::RichText::new(format!("n={} md={}", n_neighbors, min_dist)).size(12.0)).on_hover_text("Parameters: n_neighbors and min_dist\nClick Compute to run");
    });
}
EGUI_EXAMPLE
  echo >> "$OUT"
fi

# If no egui_plot usage found, append a short example
if ! grep -RIl --exclude-dir=.git --exclude-dir=target -e "egui_plot" "$REPO_ROOT" >/dev/null 2>&1; then
  echo "// start von <generated>/egui_plot_example.rs" >> "$OUT"
  cat <<'PLOT_EXAMPLE' >> "$OUT"
use egui_plot::{Plot, Points, PlotPoints, MarkerShape};
use egui::Color32;

pub fn plot_example(ui: &mut egui::Ui, points_2d: &[[f32;2]]) {
    Plot::new("example_plot")
        .label_formatter(|name, value| format!("{}\nx: {:.3}, y: {:.3}", name, value.x, value.y))
        .show(ui, |plot_ui| {
            let data: PlotPoints = points_2d.iter().map(|&(x,y)| [x as f64, y as f64]).collect();
            plot_ui.points(
                Points::new("cluster_0", data)
                      .shape(MarkerShape::Circle)
                      .color(Color32::BLUE)
                      .radius(3.0)
            );
        });
}
PLOT_EXAMPLE
  echo >> "$OUT"
fi

# Include a short summary/example of cargo commands and deepwiki MCP usage if the .kiro skill wasn't found
if [ ! -f "$REPO_ROOT/.kiro/skills/cargo-workflow.md" ]; then
  echo "// start von <generated>/cargo_workflow_summary.txt" >> "$OUT"
  cat <<'CARGO_SUMMARY' >> "$OUT"
# Cargo workflow summary (generated)

- Install or upgrade developer tools:
  - rustup update
  - cargo install cargo-edit   # provides `cargo add` / `cargo upgrade`

- Add or upgrade dependencies:
  - cargo add crate_name       # adds latest compatible version
  - cargo upgrade -p crate_name   # upgrade a specific package (requires cargo-edit)
  - cargo update                # update lockfile within version constraints

- DeepWiki MCP lookup (project-specific tool used in this repo):
  The repository uses a DeepWiki "MCP" tool for looking up GitHub-based dependencies and docs. The expected lookup format is <org>/<repo>. Example usage in the repo:

mcp_deepwiki_ask_question(
    repoName="emilk/egui",
    question="How to use Plot::new in egui_plot?"
)

CARGO_SUMMARY
  echo >> "$OUT"
fi

echo "Wrote: $OUT"

exit 0
