mod handlers;
mod shibe_api;

use crate::handlers::handle_inline_query;

use anyhow::{Context, Result};
use teloxide::prelude::{dptree, *};
use tracing::info;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use std::time::Duration;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

fn main() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("Failed to intialize tokio runtime")?;
    runtime.block_on(run())
}

async fn run() -> Result<()> {
    let fmt_subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(fmt_subscriber)
        .context("Failed to intialize logging")?;

    info!("Starting Shibe bot...");

    let http_client = make_client().context("Failed to create HTTP client")?;

    let inline_handler =
        Update::filter_inline_query().branch(dptree::endpoint(handle_inline_query));

    let bot = Bot::from_env_with_client(http_client.clone());

    let handler = dptree::entry().branch(inline_handler);

    info!("Dispatching requests...");
    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![http_client])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    Ok(())
}

fn make_client() -> Result<reqwest::Client> {
    let connect_timeout = Duration::from_secs(5);
    let timeout = connect_timeout + Duration::from_secs(12);

    reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(timeout)
        .tcp_nodelay(true)
        .user_agent(APP_USER_AGENT)
        .build()
        .context("Failed to create HTTP client")
}
