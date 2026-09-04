# Serial execution checklist: abort-safe Gemini generation

Execute in this order.  Do not skip a validation gate; commit only after its
listed gates pass.  Never print `/workspace/src/.env` or the Gemini key.

1. **Establish the baseline and dependency decision.** Read every file in the
   context map in `plan.md`; run `cargo search gemini-rust --limit 5`, `cargo
   info gemini-rust`, `cargo tree -i gemini-rust`, `cargo fmt -- --check`, and
   `cargo test --all-targets`. Record pre-existing failures separately. Confirm
   current APIs from installed 2.0.0 source and DeepWiki; update `deps.md` only
   if the version/dependency decision changes.

2. **Specify and test the domain state machine first.** Add a generation status
   and public error-code model with legal transitions, retry policy, terminal
   predicate, output-completeness validator, and provider-neutral outcomes.
   Unit-test every legal/illegal transition, output class, retryable 503/network
   error, 429 fallback, safety/invalid-request terminal error, exhausted retry,
   and EOF without completion. Run focused module tests plus `cargo fmt --
   --check` and `cargo clippy --all-targets -- -D warnings`. Commit with an
   explanatory Conventional Commit body.

3. **Migrate storage safely.** Add the next numbered additive migration and
   update `Summary`/repository methods. Implement atomic CAS state updates and
   epoch-bound chunk appends; define and test legacy backfill. Test migrations
   on an empty DB and a fixture representing pre-migration rows, including index
   creation and preserved old browse/export queries. Run `cargo test
   --all-targets`, `sqlx migrate run` on a disposable DB, formatting, and
   clippy. Commit separately.

4. **Migrate the Gemini adapter.** Update to the latest stable `gemini-rust`
   version only after repeating Step 1; use `cargo upgrade --incompatible allow`
   for an intentional upgrade. Replace legacy generation with Interactions
   streaming, preserving system instruction, Gemini-3 thinking, search, URL
   context, streaming text, thought capture, usage, and cost. Add a fake event
   stream test seam. Unit-test delta/usage/completed success, failed terminal
   status, missing terminal event, stream error after partial text, and output
   completion rejection. Keep Hetzner behavior covered. Run `cargo check
   --all-targets`, tests, format, and all-feature clippy. Commit separately.

5. **Make orchestration finite and recoverable.** Integrate named outcomes into
   `run_model_pipeline` and `process_summary`; persist attempts/next retry and
   use bounded jittered delays. Prevent cross-attempt output concatenation;
   add startup stale-running recovery and an idempotent user retry operation.
   Test paused-time retry scheduling, fallback, retry exhaustion, crash/stale
   recovery, multi-URL parent aggregation, and no task can leave an untracked
   non-terminal state. Run the full Rust suite and lint gates. Commit separately.

6. **Finish the UI/API contract.** Extend route view models and Askama partials
   to render queued/running/retry, succeeded, failed, and partial-failed states;
   add accessible retry control and safe public messages. Poll only nonterminal
   rows. Add route tests for attributes/content and a WebDriver integration test
   that injects a provider abort, observes polling stop, sees the error card,
   then retries successfully. Run the focused browser command and all Rust gates.
   Commit separately.

7. **Run opt-in live and operational checks.** Only if `GEMINI_LIVE_TEST=1` is
   explicitly set, load `/workspace/src/.env` without echoing it, issue one
   minimal bounded test request for each relevant Gemini model/capability, and
   record redacted status/finish/usage. Verify a simulated 503 uses retry-wait
   UI and no retry exhaustion spins forever. Run `git diff --check`, full test
   suite, all-feature clippy, release checks, and a manual browser inspection.

8. **Document and hand off.** Re-read this plan against the resulting diff.
   Write `plan/20260904_02_aborts/walkthrough.md` only after all implementation
   tests pass. It must list actual files/behavior, deviations, exact commands
   and results, migration/rollout/rollback guidance, learnings, extensions, and
   container programs added (or `None`). Make the documentation commit using
   the Conventional Commit policy. Confirm `git status --short` is clean and
   provide commit SHAs plus verification evidence.
