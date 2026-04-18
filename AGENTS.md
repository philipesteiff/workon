# AGENTS

This file defines how AI agents should work in this repo.

## Working Style

- Be concise.
- Prefer sharp wording over more wording.
- Do not expand scope without asking.
- Do not invent product complexity.
- Make the smallest useful next step.
- Keep artifacts readable by humans first.

## Product Bias

- Start with the CLI.
- Treat tools as adapters.
- Treat lifecycle steps as optional moments.
- Prefer portability before integration depth.

## Interface Architecture

Non-negotiable constraint:

- There is one execution path per feature.
- All interfaces must use it: CLI, TUI, and future adapters.
- Duplicating logic across interfaces is a design violation.
- The command layer is the source of truth for behavior.
- CLI and TUI can differ only in input/output presentation.
- Business logic must live below every interface.
- Tests should target the shared command layer first.
- Do not add TUI-only or CLI-only product behavior.

## Writing Rules

- Use simple language.
- Avoid filler.
- Avoid hype.
- Avoid long lists unless the structure earns it.
- If a sentence does not add clarity, remove it.
- If a concept needs many words, simplify the concept.

## Collaboration Rules

- Challenge vague ideas.
- Ask when a decision changes product direction.
- State trade-offs briefly.
- Keep decisions visible.
- Keep docs short enough to stay alive.

## Current Shape

- Workon owns context.
- Tools plug into context.
- Agents consume context.
- Engineers control the flow.
