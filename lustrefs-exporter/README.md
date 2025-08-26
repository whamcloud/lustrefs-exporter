# lustrefs-exporter

Prometheus exporter for lustre

## Building Packages For Musl

Building packages with `musl` creates statically-linked binaries that can run on any
Linux platform without dependencies. This is especially useful when testing new features while
developing on macOS.

### Building

Build the desired binary by running one of the following commands:

1. `cargo build-musl-lustrefs-exporter` - Builds the lustrefs-export binary
1. `cargo build-musl-lustre-collector` - Buildes the lustre-collector

The compile binaries will be located under: `target/x86_64-unknown-linux-musl/release/`
