# AGENTS.md — Guide for working on the viz-tool (Embedding Visualization)

This document points contributors and automated agents to the authoritative specification, the most important source files, build & run instructions, known limitations, and suggested next tasks for the `viz-tool` project.

Read this first

- Specification (requirements, design, tasks):
  - `./.kiro/specs/embedding-visualization/requirements.md` — requirements and acceptance criteria (primary).
  - `./.kiro/specs/embedding-visualization/design.md` — design notes (backend choices, GPU vs CPU, important pitfalls).
  - `./.kiro/specs/embedding-visualization/tasks.md` — implementation plan and task checklist.

Primary repo files you must read

- `viz-tool/Cargo.toml`
  - Feature flags (`cpu`, `gpu`, `gui`) and dependency wiring. Default build is CPU+GUI for the minimal flow.

- `viz-tool/deps.md`
  - One-line list of GitHub projects used as dependencies (use this for DeepWiki lookups).

- `viz-tool/src/cli.rs`
  - CLI argument & subcommand definitions. Start here to understand how the binary is invoked.

- `viz-tool/src/cli_runner.rs`
  - CLI orchestration: headless "load" and "umap2d" flows, CSV/JSON outputs, and GUI launch.
  - This is a good place to add short reproducer commands.

- `viz-tool/src/data_loader.rs`
  - Loading compact SQLite DB, BLOB -> embedding deserialization, skipping invalid blobs.
  - Contains the logic that enforces `embedding_dim` truncation and counts skipped rows.

- `viz-tool/src/embedding.rs`
  - Byte-level embedding serialization/deserialization helpers. Tests removed for the minimal flow, but functions are authoritative.

- `viz-tool/src/umap_engine.rs`
  - UMAP driver: converts inputs, builds `UmapConfig` and calls `fast-umap`.
  - Key facts:
    - GPU parametric path (`#[cfg(feature = "gpu")]`) uses fast-umap parametric API and respects `UmapConfig` (n_neighbors, min_dist, n_epochs, learning_rate, hidden_sizes).
    - CPU path uses `fast-umap` CPU backend (`cpu_backend::api::fit_cpu`) but note: the current fast-umap CPU implementation may not fully implement every UmapConfig field — see "Known limitations" below.

- `viz-tool/src/viz_app.rs`
  - The egui GUI: controls, sliders, tooltips, auto-recompute, plotting.
  - Contains the auto-recompute behavior, parameter parsing, and plotting code using `egui_plot`.

- `viz-tool/src/errors.rs`
  - Central `VizError` enum (used across the crate).

Optional / advanced modules (exist in repo but are feature-gated or not exported by default)

- `viz-tool/src/cluster_titler.rs` — Gemini/Gemma API batch titling logic (uses `gemini-rust`).
- `viz-tool/src/nn_mapper.rs` — parametric UMAP wrapper (saving/loading `FittedUmap`).
- `viz-tool/src/dbscan_engine.rs` — DBSCAN clustering implementation (alternative to `linfa`).

Build & run (quick commands)

To build and run the minimal GUI + CPU-fast-umap flow (default features):

```/dev/null/commands.sh#L1-4
# build
cargo build --release

# run GUI and auto-load compact DB (positional database argument)
./target/release/viz-tool data/summaries.db
```

Headless CLI examples (load and 2D UMAP):

```/dev/null/commands.sh#L1-3
# load only
./target/release/viz-tool data/summaries.db load

# compute 2D UMAP headless (example subset, neighbors/min_dist/epochs)
./target/release/viz-tool data/summaries.db umap2-d --subset 500 --neighbors 5 --min-dist 0.01 --epochs 200
```

GPU parametric build (if you need full parametric UMAP and transform() support):

```/dev/null/commands.sh#L1-2
# builds parametric/gpu path (requires platform WGPU/drivers)
cargo build --release --features "gpu"
```

Important known limitations (read before changing code)

- fast-umap CPU backend limitation
  - The CPU backend used as the default fallback is a simplified implementation. In practice it may not respect all `UmapConfig` fields (notably `n_neighbors` and `min_dist`), because the fast-umap crate's CPU path is a lightweight fallback in some versions. If you need parameter-sensitive behavior, use the parametric GPU backend (build with `--features gpu`) or integrate a full CPU UMAP implementation.
  - This is why changing `n_neighbors`/`min_dist` on the CPU default sometimes has no visible effect.

- Parametric UMAP (GPU) vs Classical UMAP (CPU)
  - Parametric UMAP (GPU) trains a neural network and supports `transform()` for out-of-sample points. Parameters such as `hidden_sizes` and `learning_rate` are meaningful here.
  - Classical UMAP (CPU) does not train a neural network; `hidden_sizes` is meaningless for Classic mode.

What agents should do first (practical checklist)

1. Read the spec and the design notes:
   - `./.kiro/specs/embedding-visualization/requirements.md`
   - `./.kiro/specs/embedding-visualization/design.md`
   - `./.kiro/specs/embedding-visualization/tasks.md`

2. Reproduce the dev flow locally:
   - Build & run GUI as above, load a compact DB, test the sliders and auto-recompute on small subsets (set `Max points` to 500 for fast feedback).

3. If parameters don’t appear to affect the embedding, investigate backend choice:
   - Try a GPU build (`cargo build --release --features "gpu"`) and re-run the same commands to see if parameters now affect the result.
   - If GPU build fails on CI or machine, consider integrating a proper CPU UMAP (e.g., `umap-rs`) so Classic mode respects parameters.

4. Cleanups / recommended improvements (tickets for agents):
   - Replace deprecated egui Panel APIs with `Panel::top()` / `show_inside()` to remove warnings.
   - Improve the CPU path or add a proper CPU UMAP implementation that respects config fields.
   - Add epoch progress reporting for parametric training (if fast-umap exposes progress) or instrument the training loop.
   - Add presets and more optimizer controls (neg_sample_rate / repulsion_strength) as advanced toggles.
   - Add robust tests for parameter parsing (hidden_sizes) and db loader round-trip tests.

How to use DeepWiki MCP for dependency research

- For any dependency listed in `viz-tool/deps.md` (one-per-line), use DeepWiki to ask how to use that dependency. Example query pattern:
  - mcp_deepwiki_ask_question(
    repoName="eugenehp/fast-umap",
    question="How do I use the GPU backend for parametric UMAP?"
  )

- The `deps.md` file is the canonical mapping from crate to GitHub repo (use that repoName string when querying DeepWiki via MCP).

Dependency management (cargo workflow)

The project follows a tested cargo workflow that uses DeepWiki MCP for dependency research and `cargo-edit` for version upgrades. Read `/.kiro/skills/cargo-workflow/SKILL.md` and `/.kiro/skills/cargo-workflow.md` for the full policy. Key takeaways for agents:

1. DeepWiki MCP before adding a new dependency
   - Always perform a DeepWiki lookup for the GitHub repo (format: `<org>/<project>`) before adding or upgrading a dependency. This uncovers feature flags, backend requirements, examples, and known limitations.
   - Example: `mcp_deepwiki_ask_question(repoName="eugenehp/fast-umap", question="How do I use the GPU backend for parametric UMAP?")`

2. Choose versions deliberately
   - Prefer the latest stable release compatible with the project, but record the exact version in `Cargo.toml` (pin minor/patch version, e.g. `"1.7.1"`) so upgrades are explicit.
   - Use `cargo search <crate>` and check `crates.io` for stable versions. Use DeepWiki for guidance on feature flags and breaking changes.

3. Use `cargo-edit` to upgrade safely
   - Install: `cargo install cargo-edit` (once)
   - Use `cargo upgrade` to update dependencies, and `cargo upgrade --incompatible allow` only when you intentionally want major version bumps.
   - After running `cargo upgrade`, always run `cargo check` and `cargo test` to detect breaking changes.

4. Update `deps.md`
   - For every new external dependency, add a line to `viz-tool/deps.md` with the GitHub `org/project` string. This allows future DeepWiki lookups and documents dependency origins.

5. Handle conflicts & workspace notes
   - The workspace has a shared `Cargo.lock`. When adding dependencies that transitively depend on shared crates (e.g., `ndarray`, `sprs`), conflicts may occur. If you see conflicts, consult DeepWiki and the upstream repos for compatible versions or use `cargo upgrade --incompatible allow` followed by careful testing.
   - For `sqlx` compile-time query checking, use `SQLX_OFFLINE=true` or provide the development `DATABASE_URL` when preparing queries.

6. Commands (quick)
   - Install `cargo-edit`:
     ```sh
     cargo install cargo-edit
     ```
   - Preview updates:
     ```sh
     cargo update --dry-run --verbose
     ```
   - Apply semver-compatible updates:
     ```sh
     cargo update
     ```
   - Upgrade crate versions in Cargo.toml:
     ```sh
     cargo upgrade
     cargo upgrade --incompatible allow   # use with caution
     ```
   - After changes:
     ```sh
     cargo check
     cargo test
     ```

7. Document the decision
   - When you add or upgrade a dependency, leave a short note in the pull request and update `viz-tool/deps.md`. If the upgrade required incompatible changes, document the rationale and the tests run.

Files referenced for this policy

- `.kiro/skills/cargo-workflow/SKILL.md`
- `.kiro/skills/cargo-workflow.md`

These files contain the canonical instructions and examples for dependency research, `deps.md` conventions, and `cargo` usage.

(End of dependency management section)

Developer contact & context

- Current branch: `main` (local commits). The viz-tool binary built in the workspace root `./target/release/viz-tool`.
- If you break behavior or change features, update `viz-tool/Cargo.toml` and add entries to `deps.md` as appropriate.

Appendix — quick file pointers (read in this order)

1. Spec: `./.kiro/specs/embedding-visualization/requirements.md`
2. Design: `./.kiro/specs/embedding-visualization/design.md`
3. Core code: `viz-tool/src/data_loader.rs`, `viz-tool/src/embedding.rs`, `viz-tool/src/umap_engine.rs`
4. CLI & GUI: `viz-tool/src/cli.rs`, `viz-tool/src/cli_runner.rs`, `viz-tool/src/viz_app.rs`
5. Optional: `viz-tool/src/cluster_titler.rs`, `viz-tool/src/nn_mapper.rs`, `viz-tool/src/dbscan_engine.rs` (feature-gated / advanced)

If anything here is unclear or you want AGENTS.md extended with templates for automated agents (task templates + commands + expected outputs), tell me which format you prefer and I will add it.