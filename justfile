set shell := ["zsh", "-cu"]
set positional-arguments := true

default:
    @just --list

build:
    cargo build

fmt-check:
    cargo fmt --check

test:
    cargo test

lint:
    cargo clippy --all-targets --all-features -- -D warnings

verify: fmt-check test lint

wo *args:
    #!/usr/bin/env zsh
    export WORKON_DEV_MANIFEST="{{justfile_directory()}}/Cargo.toml"
    cargo run -- "$@"

smoke:
    #!/usr/bin/env zsh
    set -euo pipefail

    goal="Just manual smoke $$"
    expected="just-manual-smoke-$$"
    switch_goal="Just manual smoke switch $$"
    switch_expected="just-manual-smoke-switch-$$"
    command_file="$(mktemp /tmp/workon-smoke.XXXXXX)"

    printf '%s\n' \
        "test \"\${PWD##*/}\" = \"$expected\"" \
        "test -f AGENTS.md" \
        "test -f CLAUDE.md" \
        "test -f workon.meta" \
        "grep -q \"Investigate (investigate)\" AGENTS.md" \
        "wo --intent investigate \"$switch_goal\"" \
        "test \"\${PWD##*/}\" = \"$switch_expected\"" \
        > "$command_file"

    WORKON_SKIP_USER_ZSHRC=1 WORKON_SHELL_COMMAND="source '$command_file'" just wo --intent investigate "$goal"
