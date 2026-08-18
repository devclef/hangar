//! Hangar binary: boots the database, runs migrations, imports the parts
//! catalog from `catalog-data/`, and serves the API plus the built frontend
//! on a single port.
//!
//! Subcommands:
//!   hangar                              # serve (default)
//!   hangar import-catalog [path]        # import one catalog file or directory
//!                                       # (no server); default dir is the
//!                                       # CATALOG_DIR env var or ./catalog-data

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use hangar::catalog;
use hangar::service::{Service, ServiceApi};
use hangar::{router, AppState};

/// Connects the pool + runs migrations; shared by serve and subcommands.
async fn connect_pool() -> anyhow::Result<sqlx::sqlite::SqlitePool> {
    let data_dir = std::env::var("DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data"));
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("creating data dir {}", data_dir.display()))?;

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| format!("sqlite://{}?mode=rwc", data_dir.join("hangar.db").display()));

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
    Ok(pool)
}

/// `hangar import-catalog [path]`: imports one catalog file, or every file
/// under a directory (default: CATALOG_DIR / ./catalog-data). Exits non-zero
/// when any file fails, so it doubles as a validator for new files.
async fn run_import_catalog(args: &[String]) -> anyhow::Result<()> {
    let pool = connect_pool().await?;
    let service = Service::from_sqlite(pool);

    match args.first() {
        Some(path) => {
            let path = PathBuf::from(path);
            if path.is_dir() {
                let summary = service.import_catalog_dir(&path).await?;
                print_summary(&summary);
            } else {
                match service.import_catalog_file(&path).await {
                    Ok(result) => {
                        for (id, name) in &result.orphaned_parts {
                            eprintln!(
                                "warning: catalog part {name:?} (id={id}) no longer present in {} — left in place, review manually",
                                result.source_file
                            );
                        }
                        println!("{}", result.summary_line());
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(1);
                    }
                }
            }
        }
        None => {
            let dir = catalog::default_catalog_dir();
            let summary = service.import_catalog_dir(&dir).await?;
            print_summary(&summary);
        }
    }

    // Non-zero exit when anything failed: this command is also the pre-commit
    // validator for new catalog files.
    Ok(())
}

fn print_summary(summary: &catalog::CatalogImportSummary) {
    println!(
        "catalog import: {} file(s) — {} created, {} updated, {} unchanged, {} failed",
        summary.files,
        summary.created,
        summary.updated,
        summary.unchanged,
        summary.failed.len()
    );
    for (file, err) in &summary.failed {
        eprintln!("  FAILED {file}: {err}");
    }
    if !summary.ok() {
        std::process::exit(1);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hangar=info,tower_http=info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("import-catalog") => return run_import_catalog(&args[1..]).await,
        Some(other) => {
            eprintln!("unknown command: {other}\nusage: hangar [import-catalog [path]]");
            std::process::exit(2);
        }
        None => {}
    }

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

    let static_dir = std::env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("static"));

    let pool = connect_pool().await?;

    // Import/refresh the reference catalog from catalog-data/. Invalid files
    // are logged and skipped; a missing directory is not an error.
    let service = Service::from_sqlite(pool.clone());
    match service
        .import_catalog_dir(&catalog::default_catalog_dir())
        .await
    {
        Ok(summary) => summary.log_summary(),
        Err(e) => tracing::error!(error = ?e, "catalog import failed"),
    }

    let state = AppState {
        service: Arc::new(service),
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
