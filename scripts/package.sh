#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cargo build --release

mkdir -p dist

case "$(uname -s)" in
  Darwin) OS="darwin" ;;
  Linux) OS="linux" ;;
  *)
    echo "Unsupported OS: $(uname -s)" >&2
    exit 1
    ;;
esac

case "$(uname -m)" in
  arm64 | aarch64) ARCH="arm64" ;;
  x86_64 | amd64) ARCH="x86_64" ;;
  *)
    echo "Unsupported architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

BINARY="target/release/wo"
ARCHIVE="dist/workon-${OS}-${ARCH}.tar.gz"

if [ ! -f "$BINARY" ]; then
  echo "Missing $BINARY. Did the build succeed?" >&2
  exit 1
fi

tar -czf "$ARCHIVE" -C target/release wo

echo "Wrote $ARCHIVE"
