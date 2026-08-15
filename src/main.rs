//! Hangar binary: boots the database, runs migrations, and serves the API
//! plus the built frontend on a single port.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use hangar::service::Service;
use hangar::{router, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hangar=info,tower_http=info".into()),
        )
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .filter(|p| *p > 0)
        .unwrap_or(8080);

    let data_dir = std::env::var("DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data"));
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("creating data dir {}", data_dir.display()))?;

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| format!("sqlite://{}?mode=rwc", data_dir.join("hangar.db").display()));

    let static_dir = std::env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("static"));

    // Parse the URL into options (preserving any query params in a custom
    // DATABASE_URL), then layer on our house rules.
    let options: sqlx::sqlite::SqliteConnectOptions = db_url
        .parse()
        .with_context(|| format!("parsing database URL {db_url}"))?;
    let options = options
        .create_if_missing(true)
        .foreign_keys(true)
        .pragma("journal_mode", "WAL");
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .with_context(|| format!("connecting to database at {db_url}"))?;
    sqlx::migrate!()
        .run(&pool)
        .await
        .context("running database migrations")?;

    let state = AppState {
        service: Arc::new(Service::from_sqlite(pool)),
        static_dir: Some(static_dir.clone()),
    };

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding to {addr}"))?;

    tracing::info!(
        port,
        data_dir = %data_dir.display(),
        static_dir = %static_dir.display(),
        "hangar starting"
    );
    axum::serve(listener, router(state))
        .await
        .context("server error")?;
    Ok(())
}
