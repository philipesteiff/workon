# AGENTS

How AI agents should work in this repo.

## Principles

- Be concise.
- Prefer sharp wording over more wording.
- Make the smallest useful next step.
- Do not expand scope or product complexity without asking.
- Keep artifacts readable by humans first.
- Challenge vague ideas and state trade-offs briefly.

## Product Shape

- Workon owns context.
- Engineers control the flow.
- Tools and agents consume context through adapters.
- Start with the CLI.
- Treat lifecycle steps as optional moments.
- Prefer portability before integration depth.

## Interface Architecture

- There is one execution path per feature.
- The command layer is the source of truth for behavior.
- CLI, TUI, and future adapters must call the same command path.
- CLI and TUI may differ only in input/output presentation.
- Do not add TUI-only or CLI-only product behavior.
- Keep shell behavior above the shared command layer.

## TUI Direction

- Keep the TUI compact by default.
- Treat it as an inline command panel, not a full-screen app.
- Preserve terminal context; avoid alternate-screen takeover unless explicitly needed.
- Use progressive disclosure: essentials first, details on demand.
- Keep `OPERATOR COMMAND` as the interaction source of truth.
- Avoid duplicate chrome; every label should earn its space.
- Use Ratatui-native blocks, lists, overlays, status rows, and key strips.
- Keep the visual style restrained: amber terminal, sharp labels, practical density.

## Shell Navigation

- Workon must not spawn a Work subshell.
- Folder changes happen through a thin shell function.
- The Rust binary returns machine signals for navigation.
- The shell function consumes those signals and runs `cd`.
- Keep one user shell: no stacked shells, no `exit` side effect.
- Local development uses `just install-dev-shell`.

## Code Maintainability

- Organize code by responsibility, not by interface.
- Keep CLI and TUI thin: parse input, render output, call commands.
- Put product behavior below every interface.
- Keep domain types separate from presentation types.
- Keep adapters behind narrow interfaces.
- Inject filesystem, process, and network dependencies.
- Prefer short functions with one job and clear names.
- Make state changes explicit: plan, apply, report.
- Return typed errors from core code; format messages at the interface edge.
- Split modules before they become hard to scan.

## Testing And Verification

- Test the command layer first, adapters second, interface wiring last.
- Every implementation must be tested through the real `wo` binary.
- Run `cargo fmt --check`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`.
- Build once with `cargo build`.
- Verify user-facing changes from a disposable temp directory, not the repo root.
- Report commands run, expected result, actual result, and any gap.
- Do not claim completion without verification.

## Collaboration

- Ask when a decision changes product direction.
- Keep decisions visible.
- Keep docs short enough to stay alive.
- Use simple language; avoid filler and hype.
- Use the Worktrunk `wt` CLI for git worktree operations.
- Do not create, switch, merge, remove, or inspect worktrees with raw `git worktree` commands unless `wt` cannot perform the needed operation.
- Commit only when asked.
- Keep commits narrow and named for the user-facing change.
