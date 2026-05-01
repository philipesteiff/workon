#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
audit_root="$(mktemp -d "${TMPDIR:-/tmp}/workon-flow-audit.XXXXXX")"
audit_root="$(cd "$audit_root" && pwd)"
workon_root="$audit_root/root with spaces"
evidence_dir="${1:-$audit_root/evidence}"
tui_capture_dir="$evidence_dir/tui"
fake_bin="$audit_root/fake-bin"
auth_failure_bin="$audit_root/auth-failure-bin"
missing_tool_bin="$audit_root/missing-tools"
fake_log="$evidence_dir/fake-tools.log"
repo_workspace="$audit_root/repo workspace"
unused_repo_workspace="$audit_root/unused repo workspace"
discover_checkout="$repo_workspace/discovered checkout"
local_checkout="$audit_root/existing checkout"
shell_home="$audit_root/shell user's home"
shell_cwd="$audit_root/shell cwd"
bash_home="$audit_root/bash user's home"
bash_cwd="$audit_root/bash cwd"
dev_shell_home="$audit_root/dev shell user's home"
fish_home="$audit_root/fish user's home"
no_shell_home="$audit_root/no shell home"
real_cargo_home="${CARGO_HOME:-$HOME/.cargo}"
real_rustup_home="${RUSTUP_HOME:-$HOME/.rustup}"

mkdir -p "$workon_root" "$evidence_dir" "$tui_capture_dir" "$fake_bin" "$auth_failure_bin" "$missing_tool_bin" "$repo_workspace" "$unused_repo_workspace" "$discover_checkout" "$local_checkout" "$shell_home" "$shell_cwd" "$bash_home" "$bash_cwd" "$dev_shell_home" "$fish_home" "$no_shell_home"
mkdir -p "$workon_root/.workon/intents/custom"
cat >"$workon_root/.workon/intents/custom/debug-production.yaml" <<'YAML'
id: debug-production
name: Debug Production
summary: Diagnose production behavior from evidence.
skill_weights:
  - systematic-debugging
mcp_weights:
  - github
  - logs
instructions:
  - Reproduce before changing code.
  - Keep rollback risk visible.
YAML
printf 'feature/discovered\n' >"$discover_checkout/.workon-current-branch"
printf 'https://github.com/openai/workon.git\n' >"$discover_checkout/.workon-origin-url"
printf 'feature/local-link\n' >"$local_checkout/.workon-current-branch"
printf 'https://github.com/openai/workon.git\n' >"$local_checkout/.workon-origin-url"

cd "$repo_root"
cargo build >/dev/null

wo="$repo_root/target/debug/wo"

run_ok() {
  local name="$1"
  shift
  {
    printf '$'
    printf ' %q' "$@"
    printf '\n\n'
    WORKON_ROOT="$workon_root" "$@"
  } >"$evidence_dir/$name.txt" 2>&1
}

run_fail() {
  local name="$1"
  shift
  local status=0
  {
    printf '$'
    printf ' %q' "$@"
    printf '\n\n'
    WORKON_ROOT="$workon_root" "$@"
  } >"$evidence_dir/$name.txt" 2>&1 || status=$?

  if [ "$status" -eq 0 ]; then
    printf 'expected failure but command succeeded: %s\n' "$name" >&2
    return 1
  fi
}

write_fake_tools() {
  cat >"$fake_bin/gh" <<'SH'
#!/bin/sh
printf 'gh %s\n' "$*" >> "$WORKON_FAKE_LOG"
if [ "$1" = "repo" ] && [ "$2" = "view" ]; then
  repo="$3"
  printf '{"nameWithOwner":"%s","defaultBranchRef":{"name":"main"},"url":"https://github.com/%s","sshUrl":"git@github.com:%s.git"}\n' "$repo" "$repo" "$repo"
  exit 0
fi
if [ "$1" = "repo" ] && [ "$2" = "clone" ]; then
  mkdir -p "$4"
  exit 0
fi
exit 2
SH

  cat >"$fake_bin/git" <<'SH'
#!/bin/sh
printf 'git %s\n' "$*" >> "$WORKON_FAKE_LOG"
if [ "$1" = "-C" ]; then
  cwd="$2"
  shift 2
fi
if [ "$1" = "fetch" ]; then
  exit 0
fi
if [ "$1" = "branch" ] && [ "$2" = "--list" ]; then
  exit 0
fi
if [ "$1" = "branch" ]; then
  if [ "$2" = "--show-current" ]; then
    if [ -n "${cwd:-}" ] && [ -f "$cwd/.workon-current-branch" ]; then
      cat "$cwd/.workon-current-branch"
    fi
    exit 0
  fi
  exit 0
fi
if [ "$1" = "remote" ] && [ "$2" = "get-url" ] && [ "$3" = "origin" ]; then
  if [ -n "${cwd:-}" ] && [ -f "$cwd/.workon-origin-url" ]; then
    cat "$cwd/.workon-origin-url"
  fi
  exit 0
fi
if [ "$1" = "worktree" ] && [ "$2" = "add" ]; then
  mkdir -p "$3"
  printf '%s\n' "$4" > "$3/.workon-current-branch"
  printf 'https://github.com/openai/workon.git\n' > "$3/.workon-origin-url"
  exit 0
fi
exit 0
SH

  chmod +x "$fake_bin/gh" "$fake_bin/git"
  : >"$fake_log"
}

write_auth_failure_tools() {
  cat >"$auth_failure_bin/gh" <<'SH'
#!/bin/sh
printf 'gh %s\n' "$*" >&2
printf 'The token in default is invalid.\n' >&2
printf 'To re-authenticate, run: gh auth login -h github.com\n' >&2
exit 4
SH

  chmod +x "$auth_failure_bin/gh"
}

run_repo_ok() {
  local name="$1"
  shift
  {
    printf '$'
    printf ' %q' "$@"
    printf '\n\n'
    WORKON_ROOT="$workon_root" \
      WORKON_FAKE_LOG="$fake_log" \
      PATH="$fake_bin:$PATH" \
      "$@"
  } >"$evidence_dir/$name.txt" 2>&1
}

require_capture() {
  local name="$1"
  local expected="$2"
  if ! grep -Fq -- "$expected" "$evidence_dir/$name.txt"; then
    printf 'missing expected text in %s: %s\n' "$name" "$expected" >&2
    return 1
  fi
}

require_tui_capture() {
  local name="$1"
  local expected="$2"
  if ! grep -Fq -- "$expected" "$tui_capture_dir/$name.txt"; then
    printf 'missing expected text in tui/%s: %s\n' "$name" "$expected" >&2
    return 1
  fi
}

reject_tui_capture() {
  local name="$1"
  local unexpected="$2"
  if grep -Fq -- "$unexpected" "$tui_capture_dir/$name.txt"; then
    printf 'unexpected text in tui/%s: %s\n' "$name" "$unexpected" >&2
    return 1
  fi
}

run_ok "01-empty-list" "$wo" list
require_capture "01-empty-list" "No active work."
run_ok "01b-help-command" "$wo" help
require_capture "01b-help-command" "wo - start, open, and archive work folders"
run_ok "01c-repos-nested-help" "$wo" repos add --help
require_capture "01c-repos-nested-help" "wo repos - link repositories to a Work"
run_ok "01d-intent-nested-help" "$wo" intent switch --help
require_capture "01d-intent-nested-help" "wo intent - define and switch reusable Work intentions"
run_ok "01e-version-command" "$wo" version
require_capture "01e-version-command" "wo "
run_ok "01f-version-option" "$wo" --version
require_capture "01f-version-option" "wo "
run_ok "02-create-investigate" "$wo" --machine --intent investigate "Investigate payment retry latency"
require_capture "02-create-investigate" "work created: Investigate payment retry latency"
require_capture "02-create-investigate" "__WORKON_CD="
run_ok "03-create-review" "$wo" --machine --intent review-pr "Review cache invalidation PR"
require_capture "03-create-review" "intent: review-pr"
run_ok "03b-create-unicode" "$wo" --machine --intent investigate "Réparer déploiement côté client"
require_capture "03b-create-unicode" "work created: Réparer déploiement côté client"
require_capture "03b-create-unicode" "slug: réparer-déploiement-côté-client"
run_ok "03c-create-cjk" "$wo" --machine --intent investigate "部署修复"
require_capture "03c-create-cjk" "work created: 部署修复"
require_capture "03c-create-cjk" "slug: 部署修复"
run_ok "03d-create-archive-target" "$wo" --intent investigate "Archive successful path"
require_capture "03d-create-archive-target" "work created: Archive successful path"
run_ok "03e-create-ambiguous-billing-question" "$wo" --intent investigate "Answer billing question"
require_capture "03e-create-ambiguous-billing-question" "work created: Answer billing question"
run_ok "03f-create-ambiguous-billing-issue" "$wo" --intent investigate "Investigate billing issue"
require_capture "03f-create-ambiguous-billing-issue" "work created: Investigate billing issue"
run_ok "04-list-active" "$wo" list
require_capture "04-list-active" "Investigate payment retry latency"
require_capture "04-list-active" "Review cache invalidation PR"
require_capture "04-list-active" "Réparer déploiement côté client"
require_capture "04-list-active" "部署修复"
require_capture "04-list-active" "Archive successful path"
require_capture "04-list-active" "Answer billing question"
require_capture "04-list-active" "Investigate billing issue"
run_ok "05-open-existing" "$wo" --machine payment
require_capture "05-open-existing" "work opened: Investigate payment retry latency"
run_fail "05a-open-ambiguous-query" "$wo" billing
require_capture "05a-open-ambiguous-query" 'work query `billing` matched multiple works'
require_capture "05a-open-ambiguous-query" "Use a slug:"
require_capture "05a-open-ambiguous-query" "wo answer-billing-question"
require_capture "05a-open-ambiguous-query" "wo investigate-billing-issue"
run_ok "06-intent-list" "$wo" intent list
require_capture "06-intent-list" "investigate"
require_capture "06-intent-list" "debug-production"
run_ok "07-intent-show" "$wo" intent show investigate
require_capture "07-intent-show" "id: investigate"
run_ok "07b-intent-show-custom" "$wo" intent show debug-production
require_capture "07b-intent-show-custom" "source: custom"
require_capture "07b-intent-show-custom" "Debug Production"
run_ok "07c-create-custom-intent" "$wo" --intent debug-production "Debug production checkout"
require_capture "07c-create-custom-intent" "intent: debug-production"
grep -Fq "Debug Production (debug-production)" "$workon_root/.workon/work/debug-production-checkout/AGENTS.md"
run_ok "08-intent-switch" "$wo" intent switch payment brainstorm
require_capture "08-intent-switch" "to: brainstorm"
run_ok "08b-intent-switch-multiword-query" "$wo" intent switch payment retry review-pr
require_capture "08b-intent-switch-multiword-query" "to: review-pr"
run_ok "09-repos-workspace-list-empty" "$wo" repos workspace list
require_capture "09-repos-workspace-list-empty" "No repository workspaces configured."
run_ok "10-repos-list-empty" "$wo" repos list payment
require_capture "10-repos-list-empty" "No repositories attached."
run_fail "11-archive-missing-error" "$wo" archive missing-work
require_capture "11-archive-missing-error" 'work not found: `missing-work`'
run_ok "11a-archive-existing" "$wo" archive archive-successful-path
require_capture "11a-archive-existing" "work archived: Archive successful path"
require_capture "11a-archive-existing" "next: wo list"
run_ok "11aa-list-after-archive" "$wo" list
if grep -Fq -- "Archive successful path" "$evidence_dir/11aa-list-after-archive.txt"; then
  printf 'archived work remained active after archive\n' >&2
  exit 1
fi
run_fail "11b-unknown-option-error" "$wo" --machien list
require_capture "11b-unknown-option-error" 'unknown option `--machien`'
run_ok "11bb-double-dash-help-work-goal" "$wo" -- --help
require_capture "11bb-double-dash-help-work-goal" "work created: --help"
run_fail "11c-list-extra-error" "$wo" list active
require_capture "11c-list-extra-error" "list does not accept extra arguments"
run_fail "11d-version-extra-error" "$wo" version extra
require_capture "11d-version-extra-error" "version does not accept extra arguments"
run_fail "11e-install-shell-extra-error" "$wo" install-shell now
require_capture "11e-install-shell-extra-error" "install-shell does not accept extra arguments"
run_fail "11f-install-dev-shell-extra-error" "$wo" install-dev-shell now
require_capture "11f-install-dev-shell-extra-error" "install-dev-shell does not accept extra arguments"
run_fail "11g-help-repos-extra-error" "$wo" help repos extra
require_capture "11g-help-repos-extra-error" "help repos does not accept extra arguments"
run_fail "11h-help-intent-extra-error" "$wo" help intent extra
require_capture "11h-help-intent-extra-error" "help intent does not accept extra arguments"
run_fail "11i-help-unknown-error" "$wo" help unknown
require_capture "11i-help-unknown-error" 'unknown help topic `unknown`'
run_fail "11j-intent-command-option-error" "$wo" --intent investigate list
require_capture "11j-intent-command-option-error" "--intent can only be used when creating or opening Work"
run_fail "11ja-intent-missing-goal-error" "$wo" --intent investigate
require_capture "11ja-intent-missing-goal-error" "--intent requires a work goal"
run_fail "11k-repos-help-extra-error" "$wo" repos help extra
require_capture "11k-repos-help-extra-error" "repos help does not accept extra arguments"
run_fail "11l-intent-help-extra-error" "$wo" intent help extra
require_capture "11l-intent-help-extra-error" "intent help does not accept extra arguments"
run_fail "12-fixed-arity-error" "$wo" intent show investigate extra
require_capture "12-fixed-arity-error" "intent show accepts exactly one intent id"
run_fail "12b-intent-list-extra-error" "$wo" intent list extra
require_capture "12b-intent-list-extra-error" "intent list does not accept extra arguments"
run_fail "12c-repos-unknown-option-error" "$wo" repos add --workspcae "$repo_workspace" payment openai/workon
require_capture "12c-repos-unknown-option-error" 'unknown repos add option `--workspcae`'
run_ok "13-repos-workspace-add" "$wo" repos workspace add "$repo_workspace" "$unused_repo_workspace"
require_capture "13-repos-workspace-add" "repository workspaces:"
require_capture "13-repos-workspace-add" "$repo_workspace"
require_capture "13-repos-workspace-add" "$unused_repo_workspace"
run_ok "13a-repos-workspace-remove-unused" "$wo" repos workspace remove "$unused_repo_workspace"
require_capture "13a-repos-workspace-remove-unused" "repository workspaces:"
require_capture "13a-repos-workspace-remove-unused" "$repo_workspace"
if grep -Fq -- "$unused_repo_workspace" "$evidence_dir/13a-repos-workspace-remove-unused.txt"; then
  printf 'unused workspace remained after remove: %s\n' "$unused_repo_workspace" >&2
  exit 1
fi
PATH="$missing_tool_bin" run_fail "13b-repos-add-missing-gh-error" "$wo" repos add --workspace "$repo_workspace" payment openai/workon
require_capture "13b-repos-add-missing-gh-error" 'required command `gh` was not found in PATH'
write_auth_failure_tools
PATH="$auth_failure_bin:$PATH" run_fail "13c-repos-add-gh-auth-error" "$wo" repos add --workspace "$repo_workspace" payment openai/workon
require_capture "13c-repos-add-gh-auth-error" "gh auth login"
require_capture "13c-repos-add-gh-auth-error" "repository context command failed: gh repo view openai/workon"
write_fake_tools
run_repo_ok "13d-repos-discover-local-checkout" "$wo" repos discover payment retry
require_capture "13d-repos-discover-local-checkout" "repository candidates for work: Investigate payment retry latency"
require_capture "13d-repos-discover-local-checkout" "openai/workon"
require_capture "13d-repos-discover-local-checkout" "feature/discovered"
run_repo_ok "14-repos-add-fake-tools" "$wo" repos add --workspace "$repo_workspace" payment retry -- openai/workon
require_capture "14-repos-add-fake-tools" "repositories added: Investigate payment retry latency"
require_capture "14-repos-add-fake-tools" "openai/workon"
run_repo_ok "15-repos-list-attached" "$wo" repos list payment retry
require_capture "15-repos-list-attached" "repositories for work: Investigate payment retry latency"
require_capture "15-repos-list-attached" "workon/investigate-payment-retry-latency"
run_fail "15a-repos-workspace-remove-in-use" "$wo" repos workspace remove "$repo_workspace"
require_capture "15a-repos-workspace-remove-in-use" "cannot remove repo workspace"
require_capture "15a-repos-workspace-remove-in-use" "investigate-payment-retry-latency"
require_capture "15a-repos-workspace-remove-in-use" "openai/workon"
run_repo_ok "16-repos-remove-force" "$wo" repos remove --force payment retry -- openai/workon
require_capture "16-repos-remove-force" "repositories removed: Investigate payment retry latency"
run_repo_ok "16b-repos-link-local-checkout" "$wo" repos link payment retry -- "$local_checkout"
require_capture "16b-repos-link-local-checkout" "repositories added: Investigate payment retry latency"
require_capture "16b-repos-link-local-checkout" "openai/workon"
run_repo_ok "16c-repos-list-local-link" "$wo" repos list payment retry
require_capture "16c-repos-list-local-link" "feature/local-link"
grep -Fq "gh repo view openai/workon" "$fake_log"
grep -Fq "git -C" "$fake_log"
grep -Fq "remote get-url origin" "$fake_log"

WORKON_TUI_CAPTURE_DIR="$tui_capture_dir" cargo test capture_tui_user_flow_frames --quiet \
  >"$evidence_dir/17-tui-capture-test.txt" 2>&1
require_capture "17-tui-capture-test" "test result: ok"
require_tui_capture "01-work-queue" "WORK QUEUE"
require_tui_capture "02-create-work" "CREATE"
reject_tui_capture "02-create-work" "queue rollout."
require_tui_capture "03-archive-work" "ARCHIVE"
reject_tui_capture "03-archive-work" "queue rollout."
require_tui_capture "04-intent-context" "INTENTS"
require_tui_capture "05-repository-context" "REPOSITORIES"

if command -v zsh >/dev/null 2>&1; then
  {
    printf '$ HOME=%q SHELL=zsh %q install-shell\n\n' "$shell_home" "$wo"
    HOME="$shell_home" SHELL="zsh" "$wo" install-shell
  } >"$evidence_dir/18-zsh-install-shell.txt" 2>&1
  require_capture "18-zsh-install-shell" "startup file updated:"
  require_capture "18-zsh-install-shell" ".zshrc"
  require_capture "18-zsh-install-shell" "  source '"
  require_capture "18-zsh-install-shell" ".workon/shell/wo'"

  zsh_script="$shell_home/.workon/shell/wo"
  zsh_expected="$workon_root/.workon/work/zsh-hook-smoke"
  {
    printf '$ HOME=%q WORKON_ROOT=%q zsh -f -c %q zsh %q %q %q\n\n' \
      "$shell_home" \
      "$workon_root" \
      'source "$1"; cd "$2"; wo --intent investigate "Zsh hook smoke"; printf "PWD:%s\n" "$PWD"; test "$PWD" = "$3"' \
      "$zsh_script" \
      "$shell_cwd" \
      "$zsh_expected"
    HOME="$shell_home" WORKON_ROOT="$workon_root" zsh -f -c \
      'source "$1"; cd "$2"; wo --intent investigate "Zsh hook smoke"; printf "PWD:%s\n" "$PWD"; test "$PWD" = "$3"' \
      zsh "$zsh_script" "$shell_cwd" "$zsh_expected"
  } >"$evidence_dir/19-zsh-shell-navigation.txt" 2>&1
  require_capture "19-zsh-shell-navigation" "work created: Zsh hook smoke"
  require_capture "19-zsh-shell-navigation" "PWD:"
  require_capture "19-zsh-shell-navigation" "zsh-hook-smoke"
else
  printf 'zsh not found; skipped zsh shell-hook audit\n' >"$evidence_dir/18-zsh-install-shell.txt"
fi

if command -v zsh >/dev/null 2>&1; then
  {
    printf '$ HOME=%q WORKON_DEV_MANIFEST=%q %q install-dev-shell\n\n' \
      "$dev_shell_home" \
      "$repo_root/Cargo.toml" \
      "$wo"
    HOME="$dev_shell_home" \
      CARGO_HOME="$real_cargo_home" \
      RUSTUP_HOME="$real_rustup_home" \
      WORKON_DEV_MANIFEST="$repo_root/Cargo.toml" \
      "$wo" install-dev-shell
  } >"$evidence_dir/19c-dev-shell-install.txt" 2>&1
  require_capture "19c-dev-shell-install" "dev shell integration installed:"
  require_capture "19c-dev-shell-install" ".workon/shell/wo-dev"

  dev_script="$dev_shell_home/.workon/shell/wo-dev"
  dev_expected="$dev_shell_home/.workon/work/dev-just-hook-smoke"
  {
    printf '$ HOME=%q zsh -f -c %q zsh %q %q %q\n\n' \
      "$dev_shell_home" \
      'source "$1"; cd "$2"; just wo --intent investigate "Dev just hook smoke"; printf "PWD:%s\n" "$PWD"; test "$PWD" = "$3"' \
      "$dev_script" \
      "$repo_root" \
      "$dev_expected"
    HOME="$dev_shell_home" \
      CARGO_HOME="$real_cargo_home" \
      RUSTUP_HOME="$real_rustup_home" \
      zsh -f -c \
      'source "$1"; cd "$2"; just wo --intent investigate "Dev just hook smoke"; printf "PWD:%s\n" "$PWD"; test "$PWD" = "$3"' \
      zsh "$dev_script" "$repo_root" "$dev_expected"
  } >"$evidence_dir/19d-dev-shell-just-wo.txt" 2>&1
  require_capture "19d-dev-shell-just-wo" "work created: Dev just hook smoke"
  require_capture "19d-dev-shell-just-wo" "PWD:"
  require_capture "19d-dev-shell-just-wo" "dev-just-hook-smoke"
else
  printf 'zsh not found; skipped dev shell-hook audit\n' >"$evidence_dir/19c-dev-shell-install.txt"
fi

if command -v bash >/dev/null 2>&1; then
  {
    printf '$ HOME=%q SHELL=bash %q install-shell\n\n' "$bash_home" "$wo"
    HOME="$bash_home" SHELL="bash" "$wo" install-shell
  } >"$evidence_dir/19a-bash-install-shell.txt" 2>&1
  require_capture "19a-bash-install-shell" "startup file updated:"
  require_capture "19a-bash-install-shell" ".bashrc"
  require_capture "19a-bash-install-shell" "  source '"
  require_capture "19a-bash-install-shell" ".workon/shell/wo'"

  bash_script="$bash_home/.workon/shell/wo"
  bash_expected="$workon_root/.workon/work/bash-hook-smoke"
  {
    printf '$ HOME=%q WORKON_ROOT=%q bash --noprofile --norc -c %q bash %q %q %q\n\n' \
      "$bash_home" \
      "$workon_root" \
      'source "$1"; cd "$2"; wo --intent investigate "Bash hook smoke"; printf "PWD:%s\n" "$PWD"; test "$PWD" = "$3"' \
      "$bash_script" \
      "$bash_cwd" \
      "$bash_expected"
    HOME="$bash_home" WORKON_ROOT="$workon_root" bash --noprofile --norc -c \
      'source "$1"; cd "$2"; wo --intent investigate "Bash hook smoke"; printf "PWD:%s\n" "$PWD"; test "$PWD" = "$3"' \
      bash "$bash_script" "$bash_cwd" "$bash_expected"
  } >"$evidence_dir/19b-bash-shell-navigation.txt" 2>&1
  require_capture "19b-bash-shell-navigation" "work created: Bash hook smoke"
  require_capture "19b-bash-shell-navigation" "PWD:"
  require_capture "19b-bash-shell-navigation" "bash-hook-smoke"
else
  printf 'bash not found; skipped bash shell-hook audit\n' >"$evidence_dir/19a-bash-install-shell.txt"
fi

{
  printf '$ HOME=%q SHELL=/usr/local/bin/fish %q install-shell\n\n' "$fish_home" "$wo"
  HOME="$fish_home" SHELL="/usr/local/bin/fish" "$wo" install-shell
} >"$evidence_dir/20-fish-install-shell.txt" 2>&1
require_capture "20-fish-install-shell" "startup file updated:"
require_capture "20-fish-install-shell" ".config/fish/config.fish"
fish_script="$fish_home/.workon/shell/wo"
fish_startup="$fish_home/.config/fish/config.fish"
grep -Fq "function wo" "$fish_script"
grep -Fq "env WORKON_HOOK_ACTIVE=1" "$fish_script"
grep -Fq "test -f " "$fish_startup"
grep -Fq "; and source " "$fish_startup"

status=0
{
  printf '$ env -u SHELL HOME=%q %q install-shell\n\n' "$no_shell_home" "$wo"
  env -u SHELL HOME="$no_shell_home" "$wo" install-shell
} >"$evidence_dir/21-missing-shell-error.txt" 2>&1 || status=$?
if [ "$status" -eq 0 ]; then
  printf 'expected missing SHELL failure but command succeeded\n' >&2
  exit 1
fi
require_capture "21-missing-shell-error" "SHELL is required to install shell integration"
require_capture "21-missing-shell-error" "Supported shells: bash, zsh, fish."

{
  printf 'Workon user-flow audit evidence\n'
  printf 'workon root: %s\n' "$workon_root"
  printf 'evidence: %s\n' "$evidence_dir"
  printf '\nCLI captures:\n'
  find "$evidence_dir" -maxdepth 1 -type f -name '*.txt' -print | sort
  printf '\nTool log:\n'
  printf '%s\n' "$fake_log"
  printf '\nTUI captures:\n'
  find "$tui_capture_dir" -maxdepth 1 -type f -name '*.txt' -print | sort
} | tee "$evidence_dir/00-index.txt"
