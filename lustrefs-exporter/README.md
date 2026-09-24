# lustrefs-exporter

Prometheus exporter for lustre

## Options

| flag | environment | effect |
|---|---|---|
| `--port <port>` | `LUSTREFS_EXPORTER_PORT` | Port to listen on (default 32221) |
| `--aggregate-client-metrics` | `LUSTREFS_EXPORTER_AGGREGATE_CLIENT_METRICS` | Sum the `lustre_client_osc_*` and `lustre_client_mdc_*` families per filesystem, dropping the `target` label, and report `lustre_osc_state` as the number of OSCs in each state per filesystem. Minima and maxima are merged and start times keep the earliest. All mounts of a filesystem on the host are summed together; the per-mount `lustre_client_llite_*` families are unchanged. The variable takes `1`/`true`/`yes`/`on` or `0`/`false`/`no`/`off`. |

## Building Packages For Musl

Building packages with `musl` creates statically-linked binaries that can run on any
Linux platform without dependencies. This is especially useful when testing new features while
developing on macOS.

### Building

Build the desired binary by running one of the following commands:

1. `cargo build-musl-lustrefs-exporter` - Builds the lustrefs-export binary
1. `cargo build-musl-lustre-collector` - Buildes the lustre-collector

The compile binaries will be located under: `target/x86_64-unknown-linux-musl/release/`
