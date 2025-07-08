use axum::{http::StatusCode, response::IntoResponse};
use tikv_jemallocator::Jemalloc;

#[global_allocator]
static ALLOC: Jemalloc = Jemalloc;

#[allow(non_upper_case_globals)]
#[unsafe(export_name = "malloc_conf")]
pub static malloc_conf: &[u8] = b"prof:true,prof_active:true,lg_prof_sample:19\0";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to access memory stack: {0}")]
    Jemalloc(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub async fn handle_get_heap() -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut prof_ctl = jemalloc_pprof::PROF_CTL.as_ref().unwrap().lock().await;
    require_profiling_activated(&prof_ctl)
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    let pprof = prof_ctl
        .dump_pprof()
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(pprof)
}

pub async fn get_heap<P>(dest: P) -> Result<(), Error>
where
    P: AsRef<std::path::Path>,
{
    let mut prof = jemalloc_pprof::PROF_CTL.as_ref().unwrap().lock().await;
    require_profiling_activated(&prof)?;

    std::fs::write(
        dest,
        prof.dump_flamegraph()
            .map_err(|e| Error::Jemalloc(e.to_string()))?,
    )?;

    Ok(())
}

/// Checks whether jemalloc profiling is activated an returns an error response if not.
fn require_profiling_activated(prof_ctl: &jemalloc_pprof::JemallocProfCtl) -> Result<(), Error> {
    if prof_ctl.activated() {
        Ok(())
    } else {
        Err(Error::Jemalloc("heap profiling not activated".into()))
    }
}

#[tokio::test]
pub async fn test_profiling() {
    jemalloc_pprof::activate_jemalloc_profiling().await;

    //get_heap("test1.svg").await.unwrap();

    let mut buffer = Vec::<usize>::new();
    buffer.reserve_exact(1024 * 1024 * 1024); // Reserve 1GB to trigger jemalloc profiling

    get_heap("test2.svg").await.unwrap();
}
