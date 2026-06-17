pub mod grpc_service;
pub mod registry;
pub mod sqlite_store;
pub mod store;

pub use sqlite_store::{SkillChangeNotification, SkillChangeType, SqliteSkillStore};
pub use store::{SkillStore, SkillStoreError, SkillStoreResult};

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use crate::grpc_service::SkillRegistryGrpcService;
use crate::registry::SkillRegistry;

/// Start the skill registry gRPC server with a SQLite-backed store.
///
/// `db_path` is the path to the SQLite database file. `skill_dirs` are
/// optional initial-import directories; if non-empty, all skills found
/// in these directories are loaded into the store before the server starts
/// serving requests.
pub async fn run(
    addr: SocketAddr,
    db_path: PathBuf,
    skill_dirs: Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    run_with_listener(listener, db_path, skill_dirs).await
}

/// Start the skill registry gRPC server bound to a pre-created listener.
///
/// Opens the SQLite store at `db_path`, imports `skill_dirs` into it, then
/// serves. Use [`run_with_store`] instead when the caller already owns a
/// shared store handle (e.g. the runtime, which shares the same store with
/// the control plane).
pub async fn run_with_listener(
    listener: tokio::net::TcpListener,
    db_path: PathBuf,
    skill_dirs: Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = listener.local_addr()?;
    tracing::info!(%addr, ?db_path, "starting skill registry server (sqlite backend)");

    let store = Arc::new(SqliteSkillStore::open(db_path).await?);
    if !skill_dirs.is_empty() {
        for dir in &skill_dirs {
            let imported = store.import_from_dir(dir).await?;
            tracing::info!(dir = %dir.display(), count = imported, "imported skills from dir");
        }
    }

    run_with_store_impl(listener, store).await
}

/// Start the skill registry gRPC server on a pre-created listener, using a
/// caller-provided store handle. The store is shared with whatever else the
/// caller wired it into (e.g. the runtime control plane), so there is a
/// single source of truth. Any directory imports must already have been done
/// by the caller.
pub async fn run_with_store(
    listener: tokio::net::TcpListener,
    store: Arc<SqliteSkillStore>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_with_store_impl(listener, store).await
}

async fn run_with_store_impl(
    listener: tokio::net::TcpListener,
    store: Arc<SqliteSkillStore>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = listener.local_addr()?;
    tracing::info!(%addr, "serving skill registry gRPC (shared store)");

    let registry = Arc::new(SkillRegistry::new(store.clone()));
    let change_rx = store.subscribe();
    let grpc_service = SkillRegistryGrpcService::new(registry, change_rx);

    let result = tonic::transport::Server::builder()
        .add_service(grpc_service.into_server())
        .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
        .await;
    tracing::info!(?result, "skill server exited");
    result?;
    Ok(())
}
