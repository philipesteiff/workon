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

## Shell Navigation

- Workon must not spawn a Work subshell for navigation.
- Folder changes happen through a thin shell function.
- The Rust binary returns machine signals for navigation.
- The shell function consumes those signals and runs `cd`.
- Keep one user shell: no stacked shells, no `exit` side effect.
- Local development uses `just install-dev-shell`.
- Keep shell behavior above the shared command layer; command outputs stay interface-neutral.

## Code Maintainability

- Organize code by responsibility, not by interface.
- Keep CLI and TUI thin: parse input, render output, call commands.
- Put product behavior in the shared command layer.
- A command should read as orchestration: validate input, load state, call services, persist changes, return a result.
- Keep domain types separate from presentation types.
- Keep adapters behind narrow interfaces: git, GitHub, Jira, Slack, Notion, editors, motors.
- Inject filesystem, process, and network dependencies so core logic can be tested without real tools.
- Prefer short functions with one job and clear names.
- Extract helpers when a block needs a comment to explain what it does.
- Make state changes explicit: plan, apply, report.
- Return typed errors from core code; format user-facing messages at the interface edge.
- Test the command layer first, adapters second, interface wiring last.
- If a module becomes hard to scan, split it before adding more behavior.

## Manual Verification

- Every implementation must be tested through the real `wo` binary.
- Run automated checks first: `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`.
- Build once, then test from a disposable temp directory, not the repo root.
- Use real terminal commands and inspect real output, files, and state.
- Cover the user-facing path changed by the implementation.
- Report the commands run, what was expected, what happened, and any gap.
- Do not claim completion without this verification.

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
