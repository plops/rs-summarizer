# SKILL.md Format Guide

## File Structure

```markdown
---
name: my-skill-name
description: What this skill does and when to use it. Use when...
---

## Instructions here...
```

## Frontmatter Fields

| Field          | Required | Constraints                                                              |
|----------------|----------|--------------------------------------------------------------------------|
| `name`         | Yes      | Max 64 chars. Lowercase `a-z`, numbers, hyphens. Must match folder name. No leading/trailing/consecutive hyphens. |
| `description`  | Yes      | Max 1024 chars. What the skill does + when to activate.                  |
| `license`      | No       | License name or reference to bundled LICENSE file.                        |
| `compatibility`| No       | Max 500 chars. Environment requirements (tools, network, OS).            |
| `metadata`     | No       | Arbitrary key-value map (e.g., author, version).                         |
| `allowed-tools`| No       | Space-separated pre-approved tools (experimental).                       |

## Body Content

Keep under 500 lines (~5000 tokens). Recommended structure:

1. **Workflow/Steps** — The main procedure
2. **Examples** — Inputs/outputs
3. **Gotchas** — Non-obvious pitfalls
4. **Validation** — How to verify correctness

## Progressive Disclosure

Move detailed content to `references/`, `scripts/`, or `assets/`. Reference with relative paths and state WHEN to load:

```markdown
If the API returns a non-200 status, read `references/error-handling.md`.
```

## Scripts

Must be non-interactive. Should support `--help`, use structured output (JSON to stdout, diagnostics to stderr), be idempotent, and pin dependency versions.

Self-contained options: `uv run` (Python PEP 723), `npx`/`bunx` (Node), `deno run` (Deno).

## Example

```markdown
---
name: cdk-deploy
description: Deploy AWS CDK stacks with best practices. Use when deploying infrastructure, running cdk deploy, or troubleshooting CDK issues.
---

## Deployment workflow

1. Run `cdk synth` to validate templates
2. Use `cdk diff` to preview changes
3. Run `cdk deploy` and review IAM changes

## Gotchas

- `cdk deploy --all` can timeout on large stacks
- IAM changes require `--require-approval never` in CI
```
