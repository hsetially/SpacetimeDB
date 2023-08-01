use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{FromRef, Path, State};
use axum::response::IntoResponse;
use http::StatusCode;
use serde::Deserialize;
use spacetimedb_lib::Identity;
use tempdir::TempDir;

use spacetimedb::address::Address;
use spacetimedb::database_instance_context::DatabaseInstanceContext;
use spacetimedb::hash::hash_bytes;
use spacetimedb::host::instance_env::InstanceEnv;
use spacetimedb::host::scheduler::Scheduler;
use spacetimedb::host::tracelog::replay::replay_report;

use crate::{log_and_500, ControlNodeDelegate, WorkerCtx};

#[derive(Deserialize)]
pub struct GetTraceParams {
    address: Address,
}
pub async fn get_tracelog(
    State(ctx): State<Arc<dyn WorkerCtx>>,
    Path(GetTraceParams { address }): Path<GetTraceParams>,
) -> axum::response::Result<impl IntoResponse> {
    let database = ctx
        .get_database_by_address(&address)
        .await
        .map_err(log_and_500)?
        .ok_or((StatusCode::NOT_FOUND, "No such database."))?;
    let database_instance = ctx.get_leader_database_instance_by_database(database.id).await;
    let instance_id = database_instance.unwrap().id;

    let host = ctx.host_controller();
    let trace = host.get_trace(instance_id).await.map_err(|e| {
        log::error!("Unable to retrieve tracelog {}", e);
        (StatusCode::SERVICE_UNAVAILABLE, "Database instance not ready.")
    })?;

    let trace = trace.ok_or(StatusCode::NOT_FOUND)?;

    Ok(trace)
}

#[derive(Deserialize)]
pub struct StopTraceParams {
    address: Address,
}
pub async fn stop_tracelog(
    State(ctx): State<Arc<dyn WorkerCtx>>,
    Path(StopTraceParams { address }): Path<StopTraceParams>,
) -> axum::response::Result<impl IntoResponse> {
    let database = ctx
        .get_database_by_address(&address)
        .await
        .map_err(log_and_500)?
        .ok_or((StatusCode::NOT_FOUND, "No such database."))?;
    let database_instance = ctx.get_leader_database_instance_by_database(database.id).await;
    let instance_id = database_instance.unwrap().id;

    let host = ctx.host_controller();
    host.stop_trace(instance_id).await.map_err(|e| {
        log::error!("Unable to retrieve tracelog {}", e);
        (StatusCode::SERVICE_UNAVAILABLE, "Database instance not ready.")
    })?;

    Ok(())
}

pub async fn perform_tracelog_replay(body: Bytes) -> axum::response::Result<impl IntoResponse> {
    // Build out a temporary database
    let tmp_dir = TempDir::new("stdb_test").expect("establish tmpdir");
    let db_path = tmp_dir.path();
    let logger_path = tmp_dir.path();
    let identity = Identity {
        data: hash_bytes(b"This is a fake identity.").data,
    };
    let address = Address::from_slice(&identity.as_slice()[0..16]);
    let dbic = DatabaseInstanceContext::new(0, 0, false, identity, address, db_path.to_path_buf(), logger_path);
    let iv = InstanceEnv::new(dbic, Scheduler::dummy(&tmp_dir.path().join("scheduler")), None);

    let tx = iv.dbic.relational_db.begin_tx();

    let (_, resp_body) = iv.tx.set(tx, || replay_report(&iv, &mut &body[..]));

    let resp_body = resp_body.map_err(log_and_500)?;

    Ok(axum::Json(resp_body))
}

pub fn router<S>() -> axum::Router<S>
where
    S: ControlNodeDelegate + Clone + 'static,
    Arc<dyn WorkerCtx>: FromRef<S>,
{
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/database/:address", get(get_tracelog))
        .route("/database/:address/stop", post(stop_tracelog))
        .route("/replay", post(perform_tracelog_replay))
}
