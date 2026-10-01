# lustrefs-exporter

Prometheus exporter for lustre

## Options

| flag | environment | effect |
|---|---|---|
| `--port <port>` | `LUSTREFS_EXPORTER_PORT` | Port to listen on (default 32221) |
| `--client-histograms` | `LUSTREFS_EXPORTER_CLIENT_HISTOGRAMS` | Export the bucketed client tables (`rpc_stats`, `io_latency_stats`, `extents_stats`) as Prometheus histograms (`_bucket{le}`, `_sum`, `_count`) instead of one counter per bucket with a `size` label. `le` is the inclusive upper bound of each kernel bucket, and the kernel's overflow bucket is `+Inf`. The latency families become `..._latency_microseconds`, in the kernel's binary microseconds (1024 ns). `_sum` is count times upper bound, since the kernel keeps no sum. The variable takes the same values as `--aggregate-client-metrics`. |
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
