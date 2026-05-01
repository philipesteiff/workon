# User Flow Audit

This audit maps Workon user journeys to concrete evidence. It is not a product command; it is a development gate for checking that the CLI, shell adapter, command layer, and compact TUI stay wired together.

Run it with:

```sh
just flow-audit
```

The command creates a disposable Workon root and writes evidence under a temporary `evidence/` folder. The output prints the exact path.

## Scenario Matrix

| Area | Scenario | Evidence |
| --- | --- | --- |
| CLI entry | Empty active Work list gives next step | `01-empty-list.txt` |
| CLI entry | `wo help` shows general command help instead of creating Work | `01b-help-command.txt` |
| CLI entry | Nested `--help` routes to repository and intent help instead of generic help | `01c-repos-nested-help.txt`, `01d-intent-nested-help.txt` |
| CLI entry | Version command and option print the binary version | `01e-version-command.txt`, `01f-version-option.txt` |
| Work lifecycle | Create Work with selected intent through real `wo` binary, using a Workon root path with spaces | `02-create-investigate.txt`, `03-create-review.txt` |
| Work lifecycle | Create Work with non-ASCII title and slug text, including wide CJK display columns | `03b-create-unicode.txt`, `03c-create-cjk.txt`, `04-list-active.txt` |
| Work lifecycle | List active Work after creation | `04-list-active.txt` |
| Work lifecycle | Archive an existing Work through the real `wo` binary and verify it leaves the active list | `03d-create-archive-target.txt`, `11a-archive-existing.txt`, `11aa-list-after-archive.txt` |
| Shell navigation signal | Machine mode emits `__WORKON_CD`, `__WORKON_ROOT`, and `__WORKON_TITLE` | `02-create-investigate.txt`, `05-open-existing.txt` |
| Shell integration | Install and source the zsh shell hook, then verify `wo` changes the current shell directory | `18-zsh-install-shell.txt`, `19-zsh-shell-navigation.txt` |
| Shell integration | Install and source the dev shell hook, then verify `just wo` routes through the local Workon command and changes the current shell directory | `19c-dev-shell-install.txt`, `19d-dev-shell-just-wo.txt` |
| Shell integration | Install and source the bash shell hook, then verify `wo` changes the current shell directory | `19a-bash-install-shell.txt`, `19b-bash-shell-navigation.txt` |
| Shell integration | Generate fish shell hook and fish startup source line | `20-fish-install-shell.txt` |
| Shell integration | Missing `SHELL` reports supported shells instead of an implicit `sh` target | `21-missing-shell-error.txt` |
| Matching | Open existing Work by partial query | `05-open-existing.txt` |
| Matching | Ambiguous partial Work query fails with slug-specific recovery commands | `03e-create-ambiguous-billing-question.txt`, `03f-create-ambiguous-billing-issue.txt`, `05a-open-ambiguous-query.txt` |
| Intent catalog | List and show reusable intents | `06-intent-list.txt`, `07-intent-show.txt` |
| Intent catalog | Load a custom YAML intent, show it as custom, create Work with it, and verify generated agent files use the custom profile | `06-intent-list.txt`, `07b-intent-show-custom.txt`, `07c-create-custom-intent.txt` |
| Intent switching | Switch a Work intent through the command path | `08-intent-switch.txt` |
| Intent switching | Switch a Work intent using an unquoted multiword Work query | `08b-intent-switch-multiword-query.txt` |
| Repository context | Empty workspace and attachment states are actionable | `09-repos-workspace-list-empty.txt`, `10-repos-list-empty.txt` |
| Errors | Missing Work gives a recovery path | `11-archive-missing-error.txt` |
| Errors | Unknown dash-prefixed options do not create accidental Work folders | `11b-unknown-option-error.txt` |
| Command escape hatch | `--` lets a Work goal start with help-like options | `11bb-double-dash-help-work-goal.txt` |
| Errors | Top-level list command misuse does not create accidental Work folders | `11c-list-extra-error.txt` |
| Errors | Reserved top-level command misuse does not create accidental Work folders | `11d-version-extra-error.txt`, `11e-install-shell-extra-error.txt`, `11f-install-dev-shell-extra-error.txt` |
| Errors | Help command misuse does not create accidental Work folders | `11g-help-repos-extra-error.txt`, `11h-help-intent-extra-error.txt`, `11i-help-unknown-error.txt` |
| Errors | Help alias misuse does not silently ignore extra arguments | `11k-repos-help-extra-error.txt`, `11l-intent-help-extra-error.txt` |
| Errors | `--intent` on command invocations is rejected instead of being silently ignored | `11j-intent-command-option-error.txt` |
| Errors | `--intent <id>` without a Work goal gives an explicit recovery command | `11ja-intent-missing-goal-error.txt` |
| Errors | Fixed-arity commands reject ignored extra arguments | `12-fixed-arity-error.txt`, `12b-intent-list-extra-error.txt` |
| Errors | Repository subcommands reject misspelled dash options before matching Work | `12c-repos-unknown-option-error.txt` |
| Errors | Missing GitHub CLI reports the required command instead of a raw IO failure | `13b-repos-add-missing-gh-error.txt` |
| Errors | GitHub CLI auth failures preserve `gh auth login` guidance | `13c-repos-add-gh-auth-error.txt` |
| Repository context | Add multiple repo workspaces, then remove an unused workspace path with spaces through the real `wo` binary | `13-repos-workspace-add.txt`, `13a-repos-workspace-remove-unused.txt` |
| Repository context | Discover an existing local checkout under a configured workspace through the real `wo` binary | `13d-repos-discover-local-checkout.txt`, `fake-tools.log` |
| Repository context | Block removal of a repo workspace while active Work has an attached repository inside it | `15a-repos-workspace-remove-in-use.txt` |
| Repository context | Add a repo workspace path with spaces, create a GitHub worktree through fake `gh`/`git` using a multiword Work query and command-local `--` separator, list it, then remove it with the same separator shape | `13-repos-workspace-add.txt`, `14-repos-add-fake-tools.txt`, `15-repos-list-attached.txt`, `16-repos-remove-force.txt`, `fake-tools.log` |
| Repository context | Link an existing local checkout using a multiword Work query and command-local `--`, then list the attached repository branch | `16b-repos-link-local-checkout.txt`, `16c-repos-list-local-link.txt`, `fake-tools.log` |
| TUI visual states | Render compact queue, create, archive, intent, and repository context frames | `tui/*.txt`, `17-tui-capture-test.txt` |

## Completion Audit

| Criterion | Evidence | Status |
| --- | --- | --- |
| CLI and command surface are wired through the real `wo` binary | `just flow-audit`, scenario matrix above | Covered |
| TUI reachable states are investigated with reviewable evidence | `17-tui-capture-test.txt`, `tui/*.txt` | Covered |
| Shell navigation preserves one user shell across zsh, bash, dev shell, and fish generation | `18-zsh-install-shell.txt` through `20-fish-install-shell.txt` | Covered |
| Repository context covers local checkout, fake GitHub, missing `gh`, and auth-failure paths | Repository context and error rows above | Covered |
| Unicode and wide-character Work titles stay usable in create/list flows | `03b-create-unicode.txt`, `03c-create-cjk.txt`, `04-list-active.txt` | Covered |
| Standard development gates pass | `cargo fmt --check`, `cargo test`, `cargo build`, `just lint`, `just smoke`, `just flow-audit` | Covered when run locally |

## Existing Gates

Keep using the normal gates after changing product behavior:

```sh
cargo fmt --check
cargo test
cargo build
just lint
```

For shell integration, run:

```sh
just smoke
just flow-audit
```

## Notes

- The command layer remains the source of truth. CLI, TUI, and shell flows in this audit all go through `wo` or existing command-layer tests.
- TUI captures are text-frame renders from Ratatui's test backend. They are meant to be reviewable evidence for layout, labels, and reachable states without depending on a particular terminal emulator.
- Real terminal startup is still covered separately by the fallback path: if the inline TUI cannot read cursor position, bare `wo` falls back to the CLI list instead of failing.
- GitHub repository creation is covered with fake `gh`/`git` tools, missing-tool behavior, and auth-failure preservation.
