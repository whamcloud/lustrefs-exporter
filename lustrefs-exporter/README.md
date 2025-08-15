# lustrefs-exporter

Prometheus exporter for lustre

## Building Packages For Musl

Building packages with `musl` creates statically-linked binaries that can run on any
Linux platform without dependencies. This is especially useful when testing new features while
developing on macOS.

### Prerequisites

Before building the binary, ensure that the musl target has been added and that `musl-cross` is installed:

1. `just install-musl-cross`

**Note** - *This only needs to be done once on your development machine.*

### Building

Build the desired binary by running one of the following commands:

1. `just build-musl-lustrefs-exporter` - Builds the lustrefs-export binary
1. `just build-musl-lustre-collector` - Buildes the lustre-collector

The compile binaries will be located under: `target/x86_64-unknown-linux-musl/release/`

### Verification (Optional)

Verify that the file is statically linked:

```bash
file target/x86_64-unknown-linux-musl/release/lustrefs-exporter
```

This should show that the binary is statically linked and should look something like this:

```text
target/x86_64-unknown-linux-musl/release/lustrefs-exporter: ELF 64-bit LSB pie executable, x86-64, version 1 (SYSV), static-pie linked, not stripped
```

### Troubleshooting

If you encounter a linking error ensure that the latest Rust musl target has been added:

```bash
just add-musl-target
```
