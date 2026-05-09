---
name: skill-creator
description: Create new Kiro skills (Agent Skills). Use when asked to create a skill, write a SKILL.md, add a new workflow to .kiro/skills/, or package agent instructions as a reusable skill.
---

## Workflow

1. **Clarify scope** — Ask what task the skill should handle and when it should activate. A skill is one coherent unit of work (like a function — not too narrow, not too broad).

2. **Choose location**:
   - `.kiro/skills/<name>/` — project-specific (committed to repo, team-shared)
   - `~/.kiro/skills/<name>/` — personal (available across all projects)

3. **Create directory structure**:
   ```
   <skill-name>/
   ├── SKILL.md           # Required
   ├── references/        # Optional: detailed docs loaded on demand
   ├── scripts/           # Optional: executable code
   └── assets/            # Optional: templates, resources
   ```

4. **Write SKILL.md** — see `references/format-guide.md` for the full format specification and examples.

5. **Validate** against the checklist below.

## Validation Checklist

- [ ] Frontmatter has `name` and `description` (both required)
- [ ] `name`: lowercase letters, numbers, hyphens only; matches folder name; max 64 chars; no leading/trailing/consecutive hyphens
- [ ] `description`: max 1024 chars; states what skill does AND when to use it; includes trigger keywords
- [ ] Body is actionable (procedures over declarations)
- [ ] SKILL.md under 500 lines / ~5000 tokens; detailed content in `references/`
- [ ] Instructions focus on what the agent wouldn't know without the skill
- [ ] Gotchas section included if there are non-obvious pitfalls
- [ ] Scripts (if any) are non-interactive, have `--help`, use structured output

## Description Writing Guidelines

- Use imperative phrasing: "Use when..." not "This skill does..."
- Focus on user intent, not implementation details
- Include keywords matching how users phrase requests
- Be specific: "Review pull requests for security vulnerabilities and test coverage. Use when reviewing PRs or preparing code for review." not "Helps with code review"
- Mention adjacent triggers: "even if they don't explicitly mention X"
- Err on the side of being pushy about when to activate

## Gotchas

- `name` field MUST match the parent directory name exactly
- Skills activate via description matching — a vague description means the skill never triggers
- The entire SKILL.md body loads into context on activation — every token competes for attention
- Reference files load only when instructions direct the agent to read them
- Don't explain things the agent already knows (what HTTP is, what a database does)
- Provide defaults, not menus — pick one recommended approach, mention alternatives briefly
- Favor procedures over declarations — teach *how to approach* a class of problems
- Scripts must be non-interactive (agents can't respond to TTY prompts)
- Use relative paths from skill root for file references: `references/foo.md`, `scripts/bar.py`
- Skills in `.kiro/skills/` are also available as `/skill-name` slash commands
