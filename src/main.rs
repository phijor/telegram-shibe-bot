use anyhow::{Context, Result};
use futures::{future::BoxFuture, FutureExt};
use teloxide::{
    dispatching::DispatcherHandler,
    prelude::*,
    types::{InlineQueryResult, InlineQueryResultPhoto},
};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::{debug, error, info, warn};
use tracing_subscriber::FmtSubscriber;

use std::borrow::Cow;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[tokio::main]
async fn main() -> Result<()> {
    run().await
}

async fn run() -> Result<()> {
    let fmt_subscriber = FmtSubscriber::new();
    tracing::subscriber::set_global_default(fmt_subscriber)
        .context("Failed to intialize logging")?;

    info!("Starting Shibe bot...");

    let bot = Bot::from_env().auto_send();

    let shibe_handler = ShibeInlineQueryHandler::new()?;

    Dispatcher::new(bot)
        .inline_queries_handler(shibe_handler)
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
        let client = reqwest::Client::builder()
            .user_agent(APP_USER_AGENT)
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client })
    }

    async fn handle_inline_query(self, cx: UpdateWithCx<AutoSend<Bot>, InlineQuery>) -> Result<()> {
        let update = &cx.update;

        info!(
            id = %update.id,
            "Inline query received from @{}",
            Self::extract_username(update)
        );
        let count: usize = update.query.parse().unwrap_or(5);

        let shibes = self
            .fetch_shibes(count)
            .await
            .context("Failed to fetch shibes")?;

        debug!(id = %update.id, "Sending answer...");
        cx.requester
            .answer_inline_query(&update.id, shibes)
            .await
            .context("Failed to send answer for inline query")?;

        Ok(())
    }

    const API_URL: &'static str = "https://shibe.online/api/shibes";

    #[tracing::instrument(level = "debug", skip(self))]
    async fn fetch_shibes(self, count: usize) -> Result<Vec<InlineQueryResult>> {
        debug!("Fetching shibes");

        let response = self
            .client
            .get(Self::API_URL)
            .query(&[("count", count)])
            .send()
            .await
            .context("Failed to request shibes")?;

        debug!("Received API response...");

        let urls: Vec<String> = response
            .json()
            .await
            .context("Failed to parse shibe result")?;

        debug!("Parsed result JSON...");

        let result: Vec<_> = urls
            .iter()
            .filter_map(|url| {
                let id = match Self::parse_id(url) {
                    Some(id) => id,
                    None => {
                        warn!(url = %url, "Failed to parse image ID from URL, skipping image.");
                        return None;
                    }
                };

                let photo = InlineQueryResultPhoto::new(id, url.as_str(), url);
                Some(InlineQueryResult::Photo(photo))
            })
            .collect();

        debug!(total = result.len(), "Finished fetching shibes.");

        Ok(result)
    }

    fn parse_id(url: &str) -> Option<String> {
        let url = reqwest::Url::parse(url).ok()?;
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

impl DispatcherHandler<AutoSend<Bot>, InlineQuery> for ShibeInlineQueryHandler {
    fn handle(
        self,
        updates: DispatcherHandlerRx<AutoSend<Bot>, InlineQuery>,
    ) -> BoxFuture<'static, ()> {
        UnboundedReceiverStream::new(updates)
            .for_each_concurrent(None, move |cx| {
                let this = self.clone();
                async {
                    let id = cx.update.query.clone();
                    if let Err(err) = this.handle_inline_query(cx).await {
                        error!(%id, "Failed to handle inline query: {}", err);

                        for cause in err.chain() {
                            error!(%id, "- caused by: {}", cause);
                        }
                    }
                }
            })
            .boxed()
    }
}
