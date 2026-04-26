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

## Package Map

- `src/domain/` contains product language and rules that should not know about filesystems, processes, terminals, `gh`, or Git worktree commands.
- `src/domain/work/` owns Work concepts, work summaries, archive/create/open domain types, and work naming rules such as title and slug generation.
- `src/domain/intent/` owns intent profiles and the default intent catalog.
- `src/domain/repository_context/` owns repository context types, repo workspace path policy, attachment/link naming, cache path policy, work branch naming, and repository name validation.
- `src/domain/agent_context/` owns agent-facing context status types and other pure context concepts.
- `src/application/` is the command layer and source of truth for behavior. Add new product features here first, then wire interfaces to it.
- `src/application/work/` owns Work use cases such as create, list, open, archive, and open-or-create.
- `src/application/repository_context/` owns repo workspace management, discovery, linking, attaching, listing, and removing repository context for a Work.
- `src/application/shell/` owns shell-install use cases, while shell script details stay in `src/interfaces/shell/`.
- `src/application/context_status/` owns the context status command until context editing becomes a fuller feature.
- `src/infrastructure/` contains adapters for external systems. It may depend on domain/application contracts, but domain must not depend on it.
- `src/infrastructure/storage/` owns Work storage, repo workspace config, minimal repository metadata JSON, and bare repository cache persistence. Do not persist live branch state that can be reconstructed from Git.
- `src/infrastructure/filesystem/` owns filesystem traits and standard filesystem implementations used for testable storage.
- `src/infrastructure/process/` owns command execution abstractions for adapters that call external tools.
- `src/infrastructure/github/` owns the `gh` CLI adapter and GitHub response parsing.
- `src/infrastructure/git_worktree/` owns raw Git worktree creation/inspection and symlink link management for repository context. Use it behind application traits.
- `src/infrastructure/agent_files/` owns rendering and rewriting `AGENTS.md` and agent-specific projections inside Work folders.
- `src/interfaces/` contains user-facing adapters only. It should parse input, render output, and call `src/application/`.
- `src/interfaces/cli/` owns CLI parsing, help text, prompting, and command output formatting.
- `src/interfaces/tui/` owns Ratatui state, key handling, rendering, animation, and TUI job orchestration. It must not introduce product behavior that bypasses `src/application/`.
- `src/interfaces/shell/` owns shell integration scripts, shell environment detection, and current-shell folder switching support.
- `src/shared/` contains narrow cross-cutting primitives only. Keep it small; do not create a generic utilities dumping ground.
- `src/shared/error.rs` owns `WorkonError` and the crate-wide `Result` alias. Add errors here only when more local typed errors would not fit.
- `src/lib.rs` should stay as the public re-export surface. Preserve stable exports unless an API change is intentional.
- `src/main.rs` should stay minimal and delegate to the CLI interface.

## Feature Placement

- For new behavior, start in `src/application/`, add or update domain types in `src/domain/`, add adapters in `src/infrastructure/`, then expose it through `src/interfaces/`.
- For new CLI or TUI affordances, first verify the command already exists in `src/application/`; if it does not, add the command layer path before UI wiring.
- For new repository-context behavior, keep GitHub discovery in `src/infrastructure/github/`, worktree inspection/linking in `src/infrastructure/git_worktree/`, workspace metadata/cache persistence in `src/infrastructure/storage/`, and orchestration in `src/application/repository_context/`.
- For new Work lifecycle behavior, keep product flow in `src/application/work/`, storage mechanics in `src/infrastructure/storage/`, and naming rules in `src/domain/work/`.
- For shared helpers, place them next to the domain or adapter that owns the concept. Use `src/shared/` only for truly cross-cutting types with clear names.

## Testing And Verification

- Test the command layer first, adapters second, interface wiring last.
- Every implementation must be tested through the real `wo` binary.
- Run `cargo fmt --check`, `cargo test`, and `just lint`.
- Build once with `cargo build`.
- Verify user-facing changes from a disposable temp directory, not the repo root.
- Report commands run, expected result, actual result, and any gap.
- Do not claim completion without verification.

## Collaboration

- Ask when a decision changes product direction.
- Keep decisions visible.
- Keep docs short enough to stay alive.
- Use simple language; avoid filler and hype.
- Repository context worktrees use raw `git worktree` through `src/infrastructure/git_worktree/`; do not add Worktrunk as a product dependency for this path.
- Commit only when asked.
- Keep commits narrow and named for the user-facing change.
