# Lustre Collector

![rust](https://github.com/whamcloud/lustre-collector/workflows/rust/badge.svg?branch=master)

![Crates.io](https://img.shields.io/crates/v/lustre_collector) ![docs.rs](https://docs.rs/lustre_collector/badge.svg)

This repo provides a parsed representation of common Lustre statistics.

It is provided as a standalone binary that can be called to retrieve stats in the desired output (Currently either JSON | YAML).

## Installation

A `lustre_collector` musl binary is provided with the latest release. It can be downloaded and run
directly on an x86_64 linux machine.

## Usage

```bash
# Will return stats in JSON format
lustre_collector

# Will return stats in YAML format
lustre_collector --format=yaml
```

## Stats sample (subject to change)

<details>
    <summary>MGT (click to expand)</summary>
    <p>

```json
[
  {
    "param": "memused",
    "value": 3380079
  },
  {
    "param": "memused_max",
    "value": 90844376
  },
  {
    "param": "lnet_memused",
    "value": 4859173
  },
  {
    "param": "health_check",
    "value": "healthy"
  },
  {
    "kind": "MGT",
    "param": "stats",
    "target": "MGS",
    "value": [
      {
        "name": "req_waittime",
        "units": "usec",
        "samples": 29966,
        "min": 7,
        "max": 76004,
        "sum": 3870437,
        "sumsquare": 8782075471
      },
      {
        "name": "req_qdepth",
        "units": "reqs",
        "samples": 29966,
        "min": 0,
        "max": 0,
        "sum": 0,
        "sumsquare": 0
      },
      {
        "name": "req_active",
        "units": "reqs",
        "samples": 29966,
        "min": 1,
        "max": 2,
        "sum": 29967,
        "sumsquare": 29969
      },
      {
        "name": "req_timeout",
        "units": "sec",
        "samples": 29966,
        "min": 1,
        "max": 10,
        "sum": 29975,
        "sumsquare": 30065
      },
      {
        "name": "reqbuf_avail",
        "units": "bufs",
        "samples": 86826,
        "min": 62,
        "max": 64,
        "sum": 5496947,
        "sumsquare": 348029843
      },
      {
        "name": "ldlm_plain_enqueue",
        "units": "reqs",
        "samples": 106,
        "min": 1,
        "max": 1,
        "sum": 106,
        "sumsquare": 106
      },
      {
        "name": "mgs_connect",
        "units": "usec",
        "samples": 6,
        "min": 1257,
        "max": 2192,
        "sum": 10564,
        "sumsquare": 19332578
      },
      {
        "name": "mgs_disconnect",
        "units": "usec",
        "samples": 1,
        "min": 59,
        "max": 59,
        "sum": 59,
        "sumsquare": 3481
      },
      {
        "name": "mgs_target_reg",
        "units": "usec",
        "samples": 32,
        "min": 723,
        "max": 16708,
        "sum": 132858,
        "sumsquare": 1025414534
      },
      {
        "name": "mgs_config_read",
        "units": "usec",
        "samples": 43,
        "min": 19,
        "max": 7283,
        "sum": 25406,
        "sumsquare": 65059670
      },
      {
        "name": "obd_ping",
        "units": "usec",
        "samples": 29556,
        "min": 2,
        "max": 32142,
        "sum": 1220258,
        "sumsquare": 1812590362
      },
      {
        "name": "llog_origin_handle_open",
        "units": "usec",
        "samples": 58,
        "min": 15,
        "max": 65,
        "sum": 2431,
        "sumsquare": 108817
      },
      {
        "name": "llog_origin_handle_next_block",
        "units": "usec",
        "samples": 112,
        "min": 11,
        "max": 9046,
        "sum": 40978,
        "sumsquare": 174867542
      },
      {
        "name": "llog_origin_handle_read_header",
        "units": "usec",
        "samples": 52,
        "min": 16,
        "max": 14357,
        "sum": 65467,
        "sumsquare": 532063427
      }
    ]
  },
  {
    "kind": "MGT",
    "param": "threads_max",
    "target": "MGS",
    "value": 32
  },
  {
    "kind": "MGT",
    "param": "threads_min",
    "target": "MGS",
    "value": 3
  },
  {
    "kind": "MGT",
    "param": "num_exports",
    "target": "MGS",
    "value": 5
  },
  {
    "nid": "0@lo",
    "param": "send_count",
    "value": 12952
  },
  {
    "nid": "0@lo",
    "param": "recv_count",
    "value": 12952
  },
  {
    "nid": "0@lo",
    "param": "drop_count",
    "value": 12952
  },
  {
    "nid": "10.73.20.11@tcp",
    "param": "send_count",
    "value": 26190
  },
  {
    "nid": "10.73.20.11@tcp",
    "param": "recv_count",
    "value": 26101
  },
  {
    "nid": "10.73.20.11@tcp",
    "param": "drop_count",
    "value": 26101
  }
]
```

</p>
</details>

<details>
    <summary>MDTs - DNE setup (click to expand)</summary>
    <p>

```json
[
  [
    {
      "param": "memused",
      "value": 93124510
    },
    {
      "param": "memused_max",
      "value": 94897674
    },
    {
      "param": "lnet_memused",
      "value": 127107077
    },
    {
      "param": "health_check",
      "value": "healthy"
    },
    {
      "kind": "MDT",
      "param": "contended_locks",
      "target": "fs-MDT0000",
      "value": 32
    },
    {
      "kind": "MDT",
      "param": "contended_locks",
      "target": "fs-MDT0001",
      "value": 32
    },
    {
      "kind": "MDT",
      "param": "contended_locks",
      "target": "fs-MDT0002",
      "value": 32
    },
    {
      "kind": "MDT",
      "param": "contention_seconds",
      "target": "fs-MDT0000",
      "value": 2
    },
    {
      "kind": "MDT",
      "param": "contention_seconds",
      "target": "fs-MDT0001",
      "value": 2
    },
    {
      "kind": "MDT",
      "param": "contention_seconds",
      "target": "fs-MDT0002",
      "value": 2
    },
    {
      "kind": "MDT",
      "param": "ctime_age_limit",
      "target": "fs-MDT0000",
      "value": 10
    },
    {
      "kind": "MDT",
      "param": "ctime_age_limit",
      "target": "fs-MDT0001",
      "value": 10
    },
    {
      "kind": "MDT",
      "param": "ctime_age_limit",
      "target": "fs-MDT0002",
      "value": 10
    },
    {
      "kind": "MDT",
      "param": "early_lock_cancel",
      "target": "fs-MDT0000",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "early_lock_cancel",
      "target": "fs-MDT0001",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "early_lock_cancel",
      "target": "fs-MDT0002",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_count",
      "target": "fs-MDT0000",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_count",
      "target": "fs-MDT0001",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_count",
      "target": "fs-MDT0002",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_timeouts",
      "target": "fs-MDT0000",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_timeouts",
      "target": "fs-MDT0001",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_timeouts",
      "target": "fs-MDT0002",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_unused_count",
      "target": "fs-MDT0000",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_unused_count",
      "target": "fs-MDT0001",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_unused_count",
      "target": "fs-MDT0002",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lru_max_age",
      "target": "fs-MDT0000",
      "value": 3900000
    },
    {
      "kind": "MDT",
      "param": "lru_max_age",
      "target": "fs-MDT0001",
      "value": 3900000
    },
    {
      "kind": "MDT",
      "param": "lru_max_age",
      "target": "fs-MDT0002",
      "value": 3900000
    },
    {
      "kind": "MDT",
      "param": "lru_size",
      "target": "fs-MDT0000",
      "value": 400
    },
    {
      "kind": "MDT",
      "param": "lru_size",
      "target": "fs-MDT0001",
      "value": 400
    },
    {
      "kind": "MDT",
      "param": "lru_size",
      "target": "fs-MDT0002",
      "value": 400
    },
    {
      "kind": "MDT",
      "param": "max_nolock_bytes",
      "target": "fs-MDT0000",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "max_nolock_bytes",
      "target": "fs-MDT0001",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "max_nolock_bytes",
      "target": "fs-MDT0002",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "max_parallel_ast",
      "target": "fs-MDT0000",
      "value": 1024
    },
    {
      "kind": "MDT",
      "param": "max_parallel_ast",
      "target": "fs-MDT0001",
      "value": 1024
    },
    {
      "kind": "MDT",
      "param": "max_parallel_ast",
      "target": "fs-MDT0002",
      "value": 1024
    },
    {
      "kind": "MDT",
      "param": "resource_count",
      "target": "fs-MDT0000",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "resource_count",
      "target": "fs-MDT0001",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "resource_count",
      "target": "fs-MDT0002",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "md_stats",
      "target": "fs-MDT0000",
      "value": [
        {
          "name": "open",
          "units": "reqs",
          "samples": 149,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        },
        {
          "name": "close",
          "units": "reqs",
          "samples": 149,
          "min": 1,
          "max": 1,
          "sum": 149,
          "sumsquare": null
        },
        {
          "name": "mknod",
          "units": "reqs",
          "samples": 26,
          "min": 1,
          "max": 1,
          "sum": 26,
          "sumsquare": null
        },
        {
          "name": "unlink",
          "units": "reqs",
          "samples": 1026,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        },
        {
          "name": "mkdir",
          "units": "reqs",
          "samples": 7,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        },
        {
          "name": "rmdir",
          "units": "reqs",
          "samples": 5,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        },
        {
          "name": "getattr",
          "units": "reqs",
          "samples": 2755,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        },
        {
          "name": "setattr",
          "units": "reqs",
          "samples": 1,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        },
        {
          "name": "getxattr",
          "units": "reqs",
          "samples": 31,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        },
        {
          "name": "statfs",
          "units": "reqs",
          "samples": 62955,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        }
      ]
    },
    {
      "kind": "MDT",
      "param": "md_stats",
      "target": "fs-MDT0001",
      "value": [
        {
          "name": "statfs",
          "units": "reqs",
          "samples": 56621,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        }
      ]
    },
    {
      "kind": "MDT",
      "param": "md_stats",
      "target": "fs-MDT0002",
      "value": [
        {
          "name": "statfs",
          "units": "reqs",
          "samples": 56604,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        }
      ]
    },
    {
      "kind": "MDT",
      "param": "num_exports",
      "target": "fs-MDT0000",
      "value": 17
    },
    {
      "kind": "MDT",
      "param": "num_exports",
      "target": "fs-MDT0001",
      "value": 14
    },
    {
      "kind": "MDT",
      "param": "num_exports",
      "target": "fs-MDT0002",
      "value": 14
    },
    {
      "kind": "MDT",
      "param": "filesfree",
      "target": "fs-MDT0000",
      "value": 2096839
    },
    {
      "kind": "MDT",
      "param": "filesfree",
      "target": "fs-MDT0001",
      "value": 2096876
    },
    {
      "kind": "MDT",
      "param": "filesfree",
      "target": "fs-MDT0002",
      "value": 2096876
    },
    {
      "kind": "MDT",
      "param": "filestotal",
      "target": "fs-MDT0000",
      "value": 2097152
    },
    {
      "kind": "MDT",
      "param": "filestotal",
      "target": "fs-MDT0001",
      "value": 2097152
    },
    {
      "kind": "MDT",
      "param": "filestotal",
      "target": "fs-MDT0002",
      "value": 2097152
    },
    {
      "kind": "MDT",
      "param": "kbytesavail",
      "target": "fs-MDT0000",
      "value": 2646343680
    },
    {
      "kind": "MDT",
      "param": "kbytesavail",
      "target": "fs-MDT0001",
      "value": 2695688192
    },
    {
      "kind": "MDT",
      "param": "kbytesavail",
      "target": "fs-MDT0002",
      "value": 2695688192
    },
    {
      "kind": "MDT",
      "param": "kbytesfree",
      "target": "fs-MDT0000",
      "value": 2913165312
    },
    {
      "kind": "MDT",
      "param": "kbytesfree",
      "target": "fs-MDT0001",
      "value": 2962509824
    },
    {
      "kind": "MDT",
      "param": "kbytesfree",
      "target": "fs-MDT0002",
      "value": 2962509824
    },
    {
      "kind": "MDT",
      "param": "kbytestotal",
      "target": "fs-MDT0000",
      "value": 2983231488
    },
    {
      "kind": "MDT",
      "param": "kbytestotal",
      "target": "fs-MDT0001",
      "value": 2983231488
    },
    {
      "kind": "MDT",
      "param": "kbytestotal",
      "target": "fs-MDT0002",
      "value": 2983231488
    }
  ],
  [
    {
      "nid": "0@lo",
      "param": "send_count",
      "value": 394632
    },
    {
      "nid": "0@lo",
      "param": "recv_count",
      "value": 394632
    },
    {
      "nid": "0@lo",
      "param": "drop_count",
      "value": 394632
    },
    {
      "nid": "10.73.20.12@tcp",
      "param": "send_count",
      "value": 1092353
    },
    {
      "nid": "10.73.20.12@tcp",
      "param": "recv_count",
      "value": 1093126
    },
    {
      "nid": "10.73.20.12@tcp",
      "param": "drop_count",
      "value": 1093126
    }
  ]
]
```

</p>
</details>

<details>
    <summary>OSTs (click to expand)</summary>
    <p>

```json
[
  [
    {
      "param": "memused",
      "value": 93124510
    },
    {
      "param": "memused_max",
      "value": 94897674
    },
    {
      "param": "lnet_memused",
      "value": 127107077
    },
    {
      "param": "health_check",
      "value": "healthy"
    },
    {
      "kind": "MDT",
      "param": "contended_locks",
      "target": "fs-MDT0000",
      "value": 32
    },
    {
      "kind": "MDT",
      "param": "contended_locks",
      "target": "fs-MDT0001",
      "value": 32
    },
    {
      "kind": "MDT",
      "param": "contended_locks",
      "target": "fs-MDT0002",
      "value": 32
    },
    {
      "kind": "MDT",
      "param": "contention_seconds",
      "target": "fs-MDT0000",
      "value": 2
    },
    {
      "kind": "MDT",
      "param": "contention_seconds",
      "target": "fs-MDT0001",
      "value": 2
    },
    {
      "kind": "MDT",
      "param": "contention_seconds",
      "target": "fs-MDT0002",
      "value": 2
    },
    {
      "kind": "MDT",
      "param": "ctime_age_limit",
      "target": "fs-MDT0000",
      "value": 10
    },
    {
      "kind": "MDT",
      "param": "ctime_age_limit",
      "target": "fs-MDT0001",
      "value": 10
    },
    {
      "kind": "MDT",
      "param": "ctime_age_limit",
      "target": "fs-MDT0002",
      "value": 10
    },
    {
      "kind": "MDT",
      "param": "early_lock_cancel",
      "target": "fs-MDT0000",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "early_lock_cancel",
      "target": "fs-MDT0001",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "early_lock_cancel",
      "target": "fs-MDT0002",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_count",
      "target": "fs-MDT0000",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_count",
      "target": "fs-MDT0001",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_count",
      "target": "fs-MDT0002",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_timeouts",
      "target": "fs-MDT0000",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_timeouts",
      "target": "fs-MDT0001",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_timeouts",
      "target": "fs-MDT0002",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_unused_count",
      "target": "fs-MDT0000",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_unused_count",
      "target": "fs-MDT0001",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lock_unused_count",
      "target": "fs-MDT0002",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "lru_max_age",
      "target": "fs-MDT0000",
      "value": 3900000
    },
    {
      "kind": "MDT",
      "param": "lru_max_age",
      "target": "fs-MDT0001",
      "value": 3900000
    },
    {
      "kind": "MDT",
      "param": "lru_max_age",
      "target": "fs-MDT0002",
      "value": 3900000
    },
    {
      "kind": "MDT",
      "param": "lru_size",
      "target": "fs-MDT0000",
      "value": 400
    },
    {
      "kind": "MDT",
      "param": "lru_size",
      "target": "fs-MDT0001",
      "value": 400
    },
    {
      "kind": "MDT",
      "param": "lru_size",
      "target": "fs-MDT0002",
      "value": 400
    },
    {
      "kind": "MDT",
      "param": "max_nolock_bytes",
      "target": "fs-MDT0000",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "max_nolock_bytes",
      "target": "fs-MDT0001",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "max_nolock_bytes",
      "target": "fs-MDT0002",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "max_parallel_ast",
      "target": "fs-MDT0000",
      "value": 1024
    },
    {
      "kind": "MDT",
      "param": "max_parallel_ast",
      "target": "fs-MDT0001",
      "value": 1024
    },
    {
      "kind": "MDT",
      "param": "max_parallel_ast",
      "target": "fs-MDT0002",
      "value": 1024
    },
    {
      "kind": "MDT",
      "param": "resource_count",
      "target": "fs-MDT0000",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "resource_count",
      "target": "fs-MDT0001",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "resource_count",
      "target": "fs-MDT0002",
      "value": 0
    },
    {
      "kind": "MDT",
      "param": "md_stats",
      "target": "fs-MDT0000",
      "value": [
        {
          "name": "open",
          "units": "reqs",
          "samples": 149,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        },
        {
          "name": "close",
          "units": "reqs",
          "samples": 149,
          "min": 1,
          "max": 1,
          "sum": 149,
          "sumsquare": null
        },
        {
          "name": "mknod",
          "units": "reqs",
          "samples": 26,
          "min": 1,
          "max": 1,
          "sum": 26,
          "sumsquare": null
        },
        {
          "name": "unlink",
          "units": "reqs",
          "samples": 1026,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        },
        {
          "name": "mkdir",
          "units": "reqs",
          "samples": 7,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        },
        {
          "name": "rmdir",
          "units": "reqs",
          "samples": 5,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        },
        {
          "name": "getattr",
          "units": "reqs",
          "samples": 2755,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        },
        {
          "name": "setattr",
          "units": "reqs",
          "samples": 1,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        },
        {
          "name": "getxattr",
          "units": "reqs",
          "samples": 31,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        },
        {
          "name": "statfs",
          "units": "reqs",
          "samples": 62955,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        }
      ]
    },
    {
      "kind": "MDT",
      "param": "md_stats",
      "target": "fs-MDT0001",
      "value": [
        {
          "name": "statfs",
          "units": "reqs",
          "samples": 56621,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        }
      ]
    },
    {
      "kind": "MDT",
      "param": "md_stats",
      "target": "fs-MDT0002",
      "value": [
        {
          "name": "statfs",
          "units": "reqs",
          "samples": 56604,
          "min": null,
          "max": null,
          "sum": null,
          "sumsquare": null
        }
      ]
    },
    {
      "kind": "MDT",
      "param": "num_exports",
      "target": "fs-MDT0000",
      "value": 17
    },
    {
      "kind": "MDT",
      "param": "num_exports",
      "target": "fs-MDT0001",
      "value": 14
    },
    {
      "kind": "MDT",
      "param": "num_exports",
      "target": "fs-MDT0002",
      "value": 14
    },
    {
      "kind": "MDT",
      "param": "filesfree",
      "target": "fs-MDT0000",
      "value": 2096839
    },
    {
      "kind": "MDT",
      "param": "filesfree",
      "target": "fs-MDT0001",
      "value": 2096876
    },
    {
      "kind": "MDT",
      "param": "filesfree",
      "target": "fs-MDT0002",
      "value": 2096876
    },
    {
      "kind": "MDT",
      "param": "filestotal",
      "target": "fs-MDT0000",
      "value": 2097152
    },
    {
      "kind": "MDT",
      "param": "filestotal",
      "target": "fs-MDT0001",
      "value": 2097152
    },
    {
      "kind": "MDT",
      "param": "filestotal",
      "target": "fs-MDT0002",
      "value": 2097152
    },
    {
      "kind": "MDT",
      "param": "kbytesavail",
      "target": "fs-MDT0000",
      "value": 2646343680
    },
    {
      "kind": "MDT",
      "param": "kbytesavail",
      "target": "fs-MDT0001",
      "value": 2695688192
    },
    {
      "kind": "MDT",
      "param": "kbytesavail",
      "target": "fs-MDT0002",
      "value": 2695688192
    },
    {
      "kind": "MDT",
      "param": "kbytesfree",
      "target": "fs-MDT0000",
      "value": 2913165312
    },
    {
      "kind": "MDT",
      "param": "kbytesfree",
      "target": "fs-MDT0001",
      "value": 2962509824
    },
    {
      "kind": "MDT",
      "param": "kbytesfree",
      "target": "fs-MDT0002",
      "value": 2962509824
    },
    {
      "kind": "MDT",
      "param": "kbytestotal",
      "target": "fs-MDT0000",
      "value": 2983231488
    },
    {
      "kind": "MDT",
      "param": "kbytestotal",
      "target": "fs-MDT0001",
      "value": 2983231488
    },
    {
      "kind": "MDT",
      "param": "kbytestotal",
      "target": "fs-MDT0002",
      "value": 2983231488
    }
  ],
  [
    {
      "nid": "0@lo",
      "param": "send_count",
      "value": 394632
    },
    {
      "nid": "0@lo",
      "param": "recv_count",
      "value": 394632
    },
    {
      "nid": "0@lo",
      "param": "drop_count",
      "value": 394632
    },
    {
      "nid": "10.73.20.12@tcp",
      "param": "send_count",
      "value": 1092353
    },
    {
      "nid": "10.73.20.12@tcp",
      "param": "recv_count",
      "value": 1093126
    },
    {
      "nid": "10.73.20.12@tcp",
      "param": "drop_count",
      "value": 1093126
    }
  ]
]
```

</p>
</details>
