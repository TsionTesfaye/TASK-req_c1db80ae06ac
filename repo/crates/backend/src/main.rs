//! TerraOps backend entrypoint.
//!
//! Subcommands:
//!   * `serve`   — run the HTTPS server
//!   * `migrate` — apply SQL migrations
//!   * `seed`    — seed the five canonical demo users (idempotent)
//!
//! All runtime values come from env vars; `.env` files are never read.

use clap::{Parser, Subcommand};
use terraops_backend::{app, config, db, seed};
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
    /// Apply pending SQL migrations.
    Migrate,
    /// Seed the five canonical demo users. Idempotent.
    Seed,
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));
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
        Cmd::Seed => {
            let pool = db::connect(&cfg).await?;
            let keys = terraops_backend::crypto::keys::RuntimeKeys::load_or_init(&cfg.runtime_dir)?;
            seed::seed_demo(&pool, &keys).await?;
            tracing::info!("demo users seeded");
            Ok(())
        }
        Cmd::Serve => app::run(cfg).await,
    }
}
