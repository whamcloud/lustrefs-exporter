# Add musl target
add-musl-target:
    rustup target add x86_64-unknown-linux-musl

# Install musl-cross on OS X (Requires Homebrew)
install-musl-cross: add-musl-target
    brew tap FiloSottile/musl-cross
    brew install musl-cross

# Build lustrefs-exporter with musl
build-musl-lustrefs-exporter:
    cargo build -p lustrefs-exporter --release --target x86_64-unknown-linux-musl

# Build lustre-collector with musl
build-musl-lustre-collector:
    cargo build -p lustre_collector --release --target x86_64-unknown-linux-musl
