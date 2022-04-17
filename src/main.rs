mod shibe_api;

use anyhow::{Context, Result};
use dptree::endpoint;
use teloxide::{
    prelude2::*,
    types::{InlineQueryResult, InlineQueryResultPhoto},
};
use tracing::{debug, info, warn};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use std::{borrow::Cow, time::Duration};

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

    let shibe_handler = ShibeInlineQueryHandler::new()?;

    let inline_handler = Update::filter_inline_query().branch(endpoint(
        |bot, update, shibe_handler: ShibeInlineQueryHandler| shibe_handler.handle(bot, update),
    ));

    let bot = Bot::from_env_with_client(shibe_handler.client.clone()).auto_send();

    let handler = dptree::entry().branch(inline_handler);

    info!("Dispatching requests...");
    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![shibe_handler])
        .build()
        .setup_ctrlc_handler()
        .dispatch()
        .await;
    Ok(())
}

#[derive(Debug, Clone)]
struct ShibeInlineQueryHandler {
    client: reqwest::Client,
}

impl ShibeInlineQueryHandler {
    fn new() -> Result<Self> {
        let connect_timeout = Duration::from_secs(5);
        let timeout = connect_timeout + Duration::from_secs(12);

        let client = reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(timeout)
            .tcp_nodelay(true)
            .user_agent(APP_USER_AGENT)
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client })
    }

    async fn handle(self, bot: AutoSend<Bot>, update: InlineQuery) -> Result<()> {
        info!(
            id = %update.id,
            "Inline query received from @{}",
            Self::extract_username(&update)
        );

        let query = shibe_api::Query::parse(&update.query);

        let shibes = self.fetch(query).await.context("Failed to fetch shibes")?;

        debug!(id = %update.id, "Sending answer...");

        bot.answer_inline_query(&update.id, shibes)
            .await
            .context("Failed to send answer for inline query")?;

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn fetch(self, query: shibe_api::Query) -> Result<Vec<InlineQueryResult>> {
        let urls = shibe_api::request(query, &self.client)
            .await
            .context("Failed to request shibes")?;

        debug!("Received API response...");

        let result: Vec<_> = urls
            .iter()
            .filter_map(|url| {
                let url = reqwest::Url::parse(url)
                    .map_err(|e| warn!("Skipping image: invalid image URL: {e}"))
                    .ok()?;
                let id = match Self::parse_id(&url) {
                    Some(id) => id,
                    None => {
                        warn!(url = %url, "Failed to parse image ID from URL, skipping image.");
                        return None;
                    }
                };

                let photo = InlineQueryResultPhoto::new(id, url.clone(), url);
                Some(InlineQueryResult::Photo(photo))
            })
            .collect();

        debug!(
            total = result.len(),
            "Finished fetching {}.", query.endpoint
        );

        Ok(result)
    }

    fn parse_id(url: &reqwest::Url) -> Option<String> {
        url.path_segments()?
            .last()
            .map(|path| path.trim_end_matches(".jpg").to_string())
    }

    fn extract_username(query: &InlineQuery) -> Cow<str> {
        match query.from.username.as_deref() {
            Some(username) => Cow::Borrowed(username),
            None => Cow::Owned(format!("id:{}", query.from.id)),
        }
    }
}
