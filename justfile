set dotenv-load := true

dev:
    cargo watch -x run

create-migration name:
    sqlx migrate add {{name}}

reset-db:
    sqlx database reset

# Build the Linux plugin binary (Docker on macOS, native cargo on Linux/CI).
build:
    #!/usr/bin/env bash
    set -euo pipefail
    # macOS can't link the x86_64-unknown-linux-gnu target with its own toolchain,
    # so build inside an amd64 container (deps cached in a named volume).
    if [ "$(uname)" = "Darwin" ]; then
        docker run --rm --platform linux/amd64 \
            -v "$PWD":/app -w /app \
            -v site-search-cargo-registry:/usr/local/cargo/registry \
            rustlang/rust:nightly \
            cargo build --release --target x86_64-unknown-linux-gnu
    else
        cargo build --release --target=x86_64-unknown-linux-gnu
    fi

# Package always builds first, so the archive can never ship without the binary.
package: build
    plugin-cli package

create-release version:
    just build
    plugin-cli package
