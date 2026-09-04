# rs-summarizer version provenance walkthrough

## Delivered behavior

`APP_VERSION` is a single compile-time constant backed by
`env!("CARGO_PKG_VERSION")`. It is copied into `AppState` when the binary
starts. There is intentionally no environment or manifest-file runtime
override: a persisted value must identify the binary that actually produced a
summary.

Migration `009_add_rs_summarizer_version.sql` adds the additive
`rs_summarizer_version TEXT NOT NULL DEFAULT ''` column. New summaries write
the running application's version at creation time. Retry and generation-state
updates do not write that column, so provenance remains immutable. Existing
rows keep `''`; the UI deliberately interprets this as **Version unbekannt**
instead of guessing a current version.

The index and browse documents now have semantic, visible footers reading
`rs-summarizer v<running-version>`. Each browse card separately reports its
stored generating version. The reduced `export-db` schema, its SELECT, and its
INSERT retain the version regardless of `--include-embeddings`.

## Evidence and tests

`cargo metadata --no-deps --format-version 1` reported package version
`1.7.6` in this worktree. The following gates passed:

```text
cargo fmt --check
cargo test --lib                       # 147 passed (before feature tests)
cargo test db::                        # 12 passed
cargo test routes::                    # 9 passed
cargo test commands::export_db::       # 4 passed
cargo test --test integration_ratings  # 4 passed
cargo test --test integration_pipeline # 15 ignored, no failures
cargo test --all-targets               # 150 unit tests + 4 ratings tests passed
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features # 151 unit tests + 4 ratings tests passed
git diff --check
```

The database tests cover a fresh migrated database, applying 009 to a real
001--008 legacy schema, insert/fetch round-trip, and retry immutability. The
route test covers the runtime footer, a deliberately different stored version,
and the legacy unknown display. The export round-trip confirms provenance is
retained when embeddings are omitted.

The ignored browser suite was not run: the requested executable
`/opt/archify-browser/chromedriver/chromedriver-linux64/chromedriver` is not
installed. Deterministic handler/template coverage was run instead.

`cargo clippy --all-targets --all-features -- -D warnings` exited successfully;
the vendored `third_party/fast-umap` emitted existing future-incompatibility
warnings about float-literal fallback.

## Commits

- `177698a feat(version): establish compile-time application version`
- `4de40de feat(version): persist summary generator version`
- `2272b59 feat(version): display application and summary versions`
- `fdd73c9 feat(export): retain summary generator version`
- `docs(version): document delivered version provenance` (this document)

## Follow-ups and container programs

No dependency or container program was introduced. Install Chromium and the
expected ChromeDriver only if the ignored browser integration suite must be run
in this container. Possible future extensions are Git revision/build-time
metadata and a health or JSON metadata endpoint; historical rows should remain
unknown unless supported by trustworthy external evidence.
