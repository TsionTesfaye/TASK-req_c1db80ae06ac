//! TerraOps backend entrypoint.
//!
//! Scaffold-level responsibilities:
//! - Parse CLI: `serve` | `migrate`.
//! - Load config (env-only; no `.env`).
//! - Bring up TLS with `rustls`, serve the Yew SPA from `dist/` and the REST
//!   API under `/api/v1/**`, on a single port.
//!
//! Feature handlers arrive in P1 and beyond per `plan.md`.

use clap::{Parser, Subcommand};
use terraops_backend::{app, config, db};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Debug, Parser)]
#[command(name = "terraops-backend", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Run the HTTPS server.
    Serve,
    /// Apply pending SQL migrations from `crates/backend/migrations/`.
    Migrate,
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().json())
        .init();
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let cli = Cli::parse();
    let cfg = config::Config::from_env()?;
    match cli.cmd {
        Cmd::Migrate => {
            let pool = db::connect(&cfg).await?;
            db::run_migrations(&pool).await?;
            tracing::info!("migrations applied");
            Ok(())
        }
        Cmd::Serve => app::run(cfg).await,
    }
}
