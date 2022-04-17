use crate::shibe_api::{self, Query};

use anyhow::{Context, Result};
use teloxide::{
    prelude2::*,
    types::{InlineQueryResult, InlineQueryResultPhoto},
};
use tracing::{debug, info, warn};

use std::borrow::Cow;

pub async fn handle_inline_query(
    bot: AutoSend<Bot>,
    update: InlineQuery,
    http_client: reqwest::Client,
) -> Result<()> {
    info!(
        id = %update.id,
        "Inline query received from @{}",
        extract_username(&update)
    );

    let query = Query::parse(&update.query);

    debug!(query = ?query, "Query parsed");

    let urls = shibe_api::request(query, &http_client)
        .await
        .context("Failed to request shibes")?;

    let results: Vec<_> = urls.into_iter().filter_map(url_to_query_result).collect();

    debug!(id = %update.id, "Sending answer...");

    bot.answer_inline_query(&update.id, results)
        .await
        .context("Failed to send answer for inline query")?;

    Ok(())
}

fn url_to_query_result(url: String) -> Option<InlineQueryResult> {
    let url = reqwest::Url::parse(&url)
        .map_err(|e| warn!("Skipping image: invalid image URL: {e}"))
        .ok()?;
    let id = match parse_id(&url) {
        Some(id) => id,
        None => {
            warn!(url = %url, "Failed to parse image ID from URL, skipping image.");
            return None;
        }
    };

    let photo_url = url.clone();
    let thumb_url = url;
    let photo = InlineQueryResultPhoto::new(id, photo_url, thumb_url);
    Some(InlineQueryResult::Photo(photo))
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
