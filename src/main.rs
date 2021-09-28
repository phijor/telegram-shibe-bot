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

use std::{borrow::Cow, str::FromStr, time::Duration};

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

    let shibe_handler = ShibeInlineQueryHandler::new()?;
    let bot = Bot::from_env_with_client(shibe_handler.client.clone()).auto_send();

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

    async fn handle_inline_query(self, cx: UpdateWithCx<AutoSend<Bot>, InlineQuery>) -> Result<()> {
        let update = &cx.update;

        info!(
            id = %update.id,
            "Inline query received from @{}",
            Self::extract_username(update)
        );

        let query: ShibeQuery = update.query.parse().unwrap_or_default();

        let shibes = self
            .fetch(query.ep, query.count)
            .await
            .context("Failed to fetch shibes")?;

        debug!(id = %update.id, "Sending answer...");
        cx.requester
            .answer_inline_query(&update.id, shibes)
            .await
            .context("Failed to send answer for inline query")?;

        Ok(())
    }

    const API_URL: &'static str = "https://shibe.online/api";

    #[tracing::instrument(level = "debug", skip(self))]
    async fn fetch(self, ep: ApiEndpoint, count: Option<usize>) -> Result<Vec<InlineQueryResult>> {
        let ep = ep.as_str();
        debug!("Fetching {}", ep);

        let count = count.unwrap_or(5).min(25);

        let response = self
            .client
            .get(format!("{}/{}", Self::API_URL, ep))
            .query(&[("count", count)])
            .send()
            .await
            .context("Failed to request shibes")?;

        debug!("Received API response...");

        let urls: Vec<String> = response.json().await.context("Failed to parse response")?;

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

        debug!(total = result.len(), "Finished fetching {}.", ep);

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

#[derive(Debug, Clone, Copy, PartialEq)]
enum ApiEndpoint {
    Shibes,
    Cats,
    Birds,
}

impl Default for ApiEndpoint {
    fn default() -> Self {
        Self::Shibes
    }
}

impl ApiEndpoint {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Shibes => "shibes",
            Self::Cats => "cats",
            Self::Birds => "birds",
        }
    }
}

impl FromStr for ApiEndpoint {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ep = match s {
            "shibe" | "shibes" | "shiba" | "shibas" => ApiEndpoint::Shibes,
            "cat" | "cats" => ApiEndpoint::Cats,
            "bird" | "birds" => ApiEndpoint::Birds,
            _ => return Err(()),
        };

        Ok(ep)
    }
}

#[derive(Debug, PartialEq)]
struct ShibeQuery {
    count: Option<usize>,
    ep: ApiEndpoint,
}

impl Default for ShibeQuery {
    fn default() -> Self {
        Self {
            count: None,
            ep: ApiEndpoint::Shibes,
        }
    }
}

impl FromStr for ShibeQuery {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split_whitespace();

        let query = match (parts.next(), parts.next()) {
            (Some(count), Some(ep)) => {
                let count = count.parse().ok();
                let ep = ApiEndpoint::from_str(ep).map_err(drop)?;

                Self { count, ep }
            }
            (Some(ep), None) => Self {
                count: None,
                ep: ApiEndpoint::from_str(ep).unwrap_or_default(),
            },
            _ => return Err(()),
        };

        Ok(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod shibe_query {
        use super::*;

        #[test]
        fn parse_full() {
            assert_eq!(
                "5 cats".parse(),
                Ok(ShibeQuery {
                    count: Some(5),
                    ep: ApiEndpoint::Cats
                })
            )
        }

        #[test]
        fn parse_omit_count() {
            assert_eq!(
                "cats".parse(),
                Ok(ShibeQuery {
                    count: None,
                    ep: ApiEndpoint::Cats
                })
            )
        }

        #[test]
        fn parse_fail_empty() {
            assert_eq!("".parse::<ShibeQuery>(), Err(()));
        }
    }
}
