# rs-summarizer — first-checkout instructions

This repository contains the `rs-summarizer` workspace and a bundled copy of `third_party/fast-umap` used for experiments.

Follow these steps on first checkout to prepare the workspace and apply local patches used by our build/run scripts.

1. Clone the repository

```sh
git clone https://github.com/plops/rs-summarizer.git
cd rs-summarizer
```

2. Apply third_party patches

We keep small fixes to vendored third-party code in `patches/` so that fresh checkouts can apply them reproducibly.

Run:

```sh
./scripts/apply_submodule_patches.sh
```

This will apply `patches/fast-umap-mod.patch` into `third_party/fast-umap` and attempt to commit the changes locally in that directory if it is a git repo.

3. Build & run the viz GUI (GPU)

Run the helper which builds and launches the GUI (release build, GPU+GUI features):

```sh
./scripts/run_release_gpu_gui.sh path/to/your/db.db
```

Notes:
- The release runner will automatically set `RAYON_NUM_THREADS` to the number of logical CPU cores on the machine to accelerate CPU-bound phases (k-NN / nn-descent). You can override the value by setting the environment variable before launching the script.

- The `FAST_UMAP_NN_DESCENT_THRESHOLD` environment variable is used by our fast-umap patch to control when nn-descent is used. You can set this environment variable to a lower value on machines where you want nn-descent to kick in earlier.

4. Cleaning up large artifacts

It looks like the vendor tree `third_party/fast-umap` contains documentation figures and generated assets (e.g. `third_party/fast-umap/figures/`) which are not required for building/running and bloat the repository. We have added those paths to `.gitignore` and removed them from the tracked files in this branch. If you need them for development, re-generate them in the vendor directory or fetch upstream from the original project.

If the repository is still large on GitHub due to files in the history, you can rewrite history (carefully) using `git filter-repo` or `BFG` to remove big files — coordinate with the team before doing this on a shared repo.

5. Useful dev scripts

- `./scripts/run_release_gpu_gui.sh [db_path]` — Build & run the viz GUI with GPU+GUI features (defaults to `data/summaries.db`).
- `./scripts/apply_submodule_patches.sh` — Apply patches from `patches/` into third_party directories.
- `./scripts/release.sh <version>` — Prepare and tag a release (follows the repo's release process).

If anything in the above process fails (patch application, missing third_party content), open an issue or ask in the team chat with the output of the failing command and I will help fix it.
