use reqwest::Client;
use tracing::debug;

use std::{convert::Infallible, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Endpoint {
    Shibes,
    Cats,
    Birds,
}

impl Default for Endpoint {
    fn default() -> Self {
        Self::Shibes
    }
}

impl Endpoint {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Shibes => "shibes",
            Self::Cats => "cats",
            Self::Birds => "birds",
        }
    }
}

impl FromStr for Endpoint {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ep = match s {
            "shibe" | "shibes" | "shiba" | "shibas" => Endpoint::Shibes,
            "cat" | "cats" => Endpoint::Cats,
            "bird" | "birds" => Endpoint::Birds,
            _ => return Err(()),
        };

        Ok(ep)
    }
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ep = match self {
            Endpoint::Shibes => "shibes",
            Endpoint::Cats => "cats",
            Endpoint::Birds => "birds",
        };
        f.write_str(ep)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Query {
    pub endpoint: Endpoint,
    pub count: usize,
}

impl Query {
    pub fn parse(query_str: &str) -> Self {
        let mut query = Self::default();

        let mut parts = query_str.split_whitespace().peekable();

        parts.next_if(|&s| match s.parse::<usize>() {
            Ok(count) => {
                query.count = count;
                true
            }
            Err(_) => false,
        });

        if let Some(endpoint) = parts.next().and_then(|ep| Endpoint::from_str(ep).ok()) {
            query.endpoint = endpoint;
        }

        query
    }
}

impl Default for Query {
    fn default() -> Self {
        Query {
            endpoint: Endpoint::default(),
            count: 5,
        }
    }
}

impl FromStr for Query {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Query::parse(s))
    }
}

const API_URL: &str = "https://shibe.online/api";

pub async fn request(mut query: Query, client: &Client) -> Result<Vec<String>, reqwest::Error> {
    query.count = query.count.min(25);

    debug!("Requesting {}", query.endpoint.as_str());

    let url = format!("{}/{}", API_URL, query.endpoint);

    client
        .get(url)
        .query(&[("count", query.count)])
        .send()
        .await?
        .json()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    mod query {
        use super::*;

        #[test]
        fn parse_full() {
            assert_eq!(
                "5 cats".parse(),
                Ok(Query {
                    count: 5,
                    endpoint: Endpoint::Cats
                })
            )
        }

        #[test]
        fn parse_omit_count() {
            assert_eq!(
                "cats".parse(),
                Ok(Query {
                    endpoint: Endpoint::Cats,
                    ..Query::default()
                })
            )
        }

        #[test]
        fn parse_fail_empty() {
            assert_eq!("".parse::<Query>(), Ok(Query::default()));
        }
    }
}
