// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
};
use lustrefs_exporter::{
    remote_cmd::{Child, test_utils::TestCmd},
    routes::{self, AppState},
};
use std::{
    os::unix::process::ExitStatusExt as _,
    process::{ExitStatus, Output},
    sync::Arc,
    time::Duration,
};
use tokio::{task::JoinSet, time::Instant};
use tower::ServiceExt as _;

/// Create a new Axum app with the provided state and a Request
/// to scrape the metrics endpoint.
fn get_app(state: AppState) -> (Request<Body>, Router) {
    let app = routes::app(Arc::new(state));

    let request = Request::builder()
        .uri("/metrics?jobstats=true")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    (request, app)
}

// Create the shared state and wrap it in an Arc so that it can be reused
// across requests without building it each time.
pub fn create_app_state() -> AppState {
    let cmd_hdl = TestCmd::default()
        .set_spawn("lctl get_param obdfilter.*OST*.job_stats mdt.*.job_stats", vec![
            Child {
                stdout:include_str!("../../fixtures/jobstats_only/2.14.0_162.txt").as_bytes().to_vec(),
                stderr:b"".to_vec()
            },
            Child {
                stdout: include_str!("../../fixtures/jobstats_only/2.14.0_164.txt").as_bytes().to_vec(),
                stderr: b"".to_vec(),
            }
        ])
        .set_output("lctl get_param memused memused_max lnet_memused health_check mdt.*.exports.*.uuid osd-*.*.filesfree osd-*.*.filestotal osd-*.*.fstype osd-*.*.kbytesavail osd-*.*.kbytesfree osd-*.*.kbytestotal osd-*.*.brw_stats osd-*.*.quota_slave.acct_group osd-*.*.quota_slave.acct_user osd-*.*.quota_slave.acct_project mgs.*.mgs.stats mgs.*.mgs.threads_max mgs.*.mgs.threads_min mgs.*.mgs.threads_started mgs.*.num_exports obdfilter.*OST*.stats obdfilter.*OST*.num_exports obdfilter.*OST*.tot_dirty obdfilter.*OST*.tot_granted obdfilter.*OST*.tot_pending obdfilter.*OST*.exports.*.stats ost.OSS.ost.stats ost.OSS.ost_io.stats ost.OSS.ost_create.stats ost.OSS.ost_out.stats ost.OSS.ost_seq.stats mds.MDS.mdt.stats mds.MDS.mdt_fld.stats mds.MDS.mdt_io.stats mds.MDS.mdt_out.stats mds.MDS.mdt_readpage.stats mds.MDS.mdt_seqm.stats mds.MDS.mdt_seqs.stats mds.MDS.mdt_setattr.stats mdt.*.md_stats mdt.*MDT*.num_exports mdt.*MDT*.exports.*.stats ldlm.namespaces.{mdt-,filter-}*.contended_locks ldlm.namespaces.{mdt-,filter-}*.contention_seconds ldlm.namespaces.{mdt-,filter-}*.ctime_age_limit ldlm.namespaces.{mdt-,filter-}*.early_lock_cancel ldlm.namespaces.{mdt-,filter-}*.lock_count ldlm.namespaces.{mdt-,filter-}*.lock_timeouts ldlm.namespaces.{mdt-,filter-}*.lock_unused_count ldlm.namespaces.{mdt-,filter-}*.lru_max_age ldlm.namespaces.{mdt-,filter-}*.lru_size ldlm.namespaces.{mdt-,filter-}*.max_nolock_bytes ldlm.namespaces.{mdt-,filter-}*.max_parallel_ast ldlm.namespaces.{mdt-,filter-}*.resource_count ldlm.services.ldlm_canceld.stats ldlm.services.ldlm_cbd.stats llite.*.stats mdd.*.changelog_users qmt.*.*.glb-usr qmt.*.*.glb-prj qmt.*.*.glb-grp".to_string(), vec![
            Output {
                status: ExitStatus::from_raw(0),
                stdout: include_str!("../../../lustre-collector/src/fixtures/valid/lustre-2.14.0_ddn133/2.14.0_ddn133_quota.txt").as_bytes().to_vec(),
                stderr: b"".to_vec(),
            },
            Output {
                status: ExitStatus::from_raw(0),
                stdout: include_str!("../../../lustre-collector/src/fixtures/valid/valid_mds.txt").as_bytes().to_vec(),
                stderr: b"".to_vec(),
            }
        ])
        .set_output("lnetctl net show -v 4".to_string(), vec![
            Output {
                status: ExitStatus::from_raw(0),
                stdout: include_str!("../../fixtures/lnetctl_net_show.txt").as_bytes().to_vec(),
                stderr: b"".to_vec(),
            },
            Output {
                status: ExitStatus::from_raw(0),
                stdout: include_str!("../../fixtures/lnetctl_net_show.txt").as_bytes().to_vec(),
                stderr: b"".to_vec(),
            }
        ])
        .set_output("lnetctl stats show".to_string(), vec![
            Output {
                status: ExitStatus::from_raw(0),
                stdout: include_str!("../../fixtures/lnetctl_stats.txt").as_bytes().to_vec(),
                stderr: b"".to_vec(),
            },
            Output {
                status: ExitStatus::from_raw(0),
                stdout: include_str!("../../fixtures/lnetctl_stats.txt").as_bytes().to_vec(),
                stderr: b"".to_vec(),
            }
        ]);

    AppState {
        cmd_hdl: Arc::new(cmd_hdl),
    }
}

// Create a single request using `oneshot`. This is equivalent to hitting the
// `/scrape` endpoint if the http service was running.
async fn make_single_request(
    state: AppState,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let (request, app) = get_app(state);
    let resp = app.oneshot(request).await?;
    let body = to_bytes(resp.into_body(), usize::MAX).await?;
    let body_str = std::str::from_utf8(&body)?;

    Ok(body_str.to_string())
}

// Use a JoinSet to make `concurrency` requests at a time, waiting for each batch to complete before
// starting the next batch.
pub async fn load_test_concurrent(concurrency: usize, total_requests: usize) -> Duration {
    let start = Instant::now();

    let mut spawned_requests = 0;
    let mut successful_requests = 0;
    let mut failed_requests = 0;

    let mut join_set = JoinSet::new();

    // Initially spawn `concurrency` requests
    for _ in 0..concurrency {
        join_set.spawn(async move { make_single_request(create_app_state()).await });

        spawned_requests += 1;
    }

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(_)) => successful_requests += 1,
            Ok(Err(_)) => failed_requests += 1,
            Err(_) => failed_requests += 1,
        }

        // Immediately spawn a new request if there are more to be made
        if spawned_requests < total_requests {
            join_set.spawn(async move { make_single_request(create_app_state()).await });

            spawned_requests += 1;
        }
    }

    let elapsed = start.elapsed();

    println!(
        "Load test completed: {} successful, {} failed requests in {:?}",
        successful_requests, failed_requests, elapsed
    );

    elapsed
}
