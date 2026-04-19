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

install-dev-shell:
    #!/usr/bin/env zsh
    export WORKON_DEV_MANIFEST="{{justfile_directory()}}/Cargo.toml"
    cargo run -- install-dev-shell

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
    smoke_home="$(mktemp -d)"
    smoke_root="$(mktemp -d)"
    real_cargo_home="${CARGO_HOME:-$HOME/.cargo}"
    real_rustup_home="${RUSTUP_HOME:-$HOME/.rustup}"
    command_file="$(mktemp /tmp/workon-smoke.XXXXXX)"
    script="$smoke_home/.workon/shell/zsh/wo-dev.zsh"

    printf '%s\n' \
        "source '$script'" \
        "export WORKON_DEV_ROOT='$smoke_root'" \
        "cd '$smoke_root'" \
        "wo --intent investigate '$goal'" \
        "test \"\${PWD##*/}\" = \"$expected\"" \
        "test -f AGENTS.md" \
        "test -f CLAUDE.md" \
        "test -f workon.meta" \
        "grep -q \"Investigate (investigate)\" AGENTS.md" \
        "just wo --intent investigate '$switch_goal'" \
        "test \"\${PWD##*/}\" = \"$switch_expected\"" \
        > "$command_file"

    HOME="$smoke_home" CARGO_HOME="$real_cargo_home" RUSTUP_HOME="$real_rustup_home" WORKON_DEV_MANIFEST="{{justfile_directory()}}/Cargo.toml" cargo run -- install-dev-shell
    zsh -f -c "source '$command_file'"
