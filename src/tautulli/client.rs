use crate::error::{AppError, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::Duration;

/// Tautulli API client
pub struct TautulliClient {
    base_url: String,
    api_key: String,
    client: Client,
}

impl TautulliClient {
    /// Create new Tautulli client
    pub fn new(base_url: String, api_key: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::Io(std::io::Error::other(e)))?;

        // Normalize base_url: ensure it ends with /
        let base_url = if base_url.ends_with('/') {
            base_url
        } else {
            format!("{base_url}/")
        };

        Ok(Self {
            base_url,
            api_key,
            client,
        })
    }

    /// Health check - verify Tautulli is reachable and API key is valid
    pub fn health_check(&self) -> Result<()> {
        log::info!("Performing Tautulli health check: {}", self.base_url);

        let url = format!(
            "{}api/v2?apikey={}&cmd=get_server_info",
            self.base_url, self.api_key
        );

        let response = self.client.get(&url).send().map_err(|e| {
            AppError::External(format!(
                "Failed to connect to Tautulli at {}: {}",
                self.base_url, e
            ))
        })?;

        if !response.status().is_success() {
            return Err(AppError::External(format!(
                "Tautulli API returned error status: {}",
                response.status()
            )));
        }

        let api_response: TautulliResponse<ServerInfo> = response
            .json()
            .map_err(|e| AppError::External(format!("Failed to parse Tautulli response: {e}")))?;

        match api_response.response.result {
            ResponseResult::Success => {
                log::info!(
                    "Tautulli health check passed: {} (version {})",
                    api_response.response.data.pms_name,
                    api_response.response.data.pms_version
                );
                Ok(())
            }
            ResponseResult::Error => Err(AppError::External(format!(
                "Tautulli API returned error: {}",
                api_response
                    .response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))),
        }
    }

    /// Get viewing history from Tautulli
    pub fn get_history(&self, length: u32) -> Result<Vec<HistoryItem>> {
        log::debug!("Fetching Tautulli history (length: {length})");

        let url = format!(
            "{}api/v2?apikey={}&cmd=get_history&length={}",
            self.base_url, self.api_key, length
        );

        let response =
            self.client.get(&url).send().map_err(|e| {
                AppError::External(format!("Failed to fetch Tautulli history: {e}"))
            })?;

        if !response.status().is_success() {
            return Err(AppError::External(format!(
                "Tautulli API returned error status: {}",
                response.status()
            )));
        }

        // Get raw response text for debugging
        let response_text = response.text().map_err(|e| {
            AppError::External(format!("Failed to read Tautulli response body: {e}"))
        })?;

        log::trace!("Tautulli history response: {response_text}");

        let api_response: TautulliResponse<HistoryResponse> = serde_json::from_str(&response_text)
            .map_err(|e| {
                log::error!("Failed to parse Tautulli history response. Error: {e}");
                log::debug!(
                    "Response body (first 1000 chars): {}",
                    &response_text[..response_text.len().min(1000)]
                );
                AppError::External(format!("Failed to parse Tautulli history response: {e}"))
            })?;

        match api_response.response.result {
            ResponseResult::Success => {
                log::debug!(
                    "Fetched {} history items",
                    api_response.response.data.data.len()
                );
                Ok(api_response.response.data.data)
            }
            ResponseResult::Error => Err(AppError::External(format!(
                "Tautulli API returned error: {}",
                api_response
                    .response
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))),
        }
    }
}

// API Response structures

#[derive(Debug, Deserialize)]
struct TautulliResponse<T> {
    response: ResponseWrapper<T>,
}

#[derive(Debug, Deserialize)]
struct ResponseWrapper<T> {
    result: ResponseResult,
    message: Option<String>,
    data: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ResponseResult {
    Success,
    Error,
}

// Server Info (for health check)

#[derive(Debug, Deserialize)]
struct ServerInfo {
    pms_name: String,
    pms_version: String,
}

// History structures

#[derive(Debug, Deserialize)]
struct HistoryResponse {
    data: Vec<HistoryItem>,
}

/// History item from Tautulli API
#[derive(Debug, Clone, Deserialize)]
pub struct HistoryItem {
    /// Username
    pub user: String,

    /// Plex rating key for the episode
    #[serde(deserialize_with = "deserialize_flexible_string")]
    pub rating_key: String,

    /// Show name (`grandparent_title` in Plex)
    #[serde(default)]
    pub grandparent_title: String,

    /// Season number (`parent_media_index`)
    #[serde(deserialize_with = "deserialize_string_to_u32")]
    pub parent_media_index: u32,

    /// Episode number (`media_index`)
    #[serde(deserialize_with = "deserialize_string_to_u32")]
    pub media_index: u32,

    /// Percent complete (0-100)
    #[serde(deserialize_with = "deserialize_string_to_u8")]
    pub percent_complete: u8,

    /// Timestamp when stopped
    #[serde(deserialize_with = "deserialize_string_to_u64")]
    pub stopped: u64,
}

// Custom deserializers (Tautulli may return numbers as strings or actual numbers)

fn deserialize_flexible_string<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct StringVisitor;

    impl Visitor<'_> for StringVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or number")
        }

        fn visit_str<E>(self, value: &str) -> std::result::Result<String, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_u64<E>(self, value: u64) -> std::result::Result<String, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_i64<E>(self, value: i64) -> std::result::Result<String, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }
    }

    deserializer.deserialize_any(StringVisitor)
}

fn deserialize_string_to_u32<'de, D>(deserializer: D) -> std::result::Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct U32Visitor;

    impl Visitor<'_> for U32Visitor {
        type Value = u32;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or number representing a u32")
        }

        fn visit_u64<E>(self, value: u64) -> std::result::Result<u32, E>
        where
            E: de::Error,
        {
            u32::try_from(value).map_err(de::Error::custom)
        }

        fn visit_i64<E>(self, value: i64) -> std::result::Result<u32, E>
        where
            E: de::Error,
        {
            u32::try_from(value).map_err(de::Error::custom)
        }

        fn visit_str<E>(self, value: &str) -> std::result::Result<u32, E>
        where
            E: de::Error,
        {
            if value.is_empty() {
                Ok(0)
            } else {
                value.parse::<u32>().map_err(de::Error::custom)
            }
        }
    }

    deserializer.deserialize_any(U32Visitor)
}

fn deserialize_string_to_u8<'de, D>(deserializer: D) -> std::result::Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct U8Visitor;

    impl Visitor<'_> for U8Visitor {
        type Value = u8;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or number representing a u8")
        }

        fn visit_u64<E>(self, value: u64) -> std::result::Result<u8, E>
        where
            E: de::Error,
        {
            u8::try_from(value).map_err(de::Error::custom)
        }

        fn visit_i64<E>(self, value: i64) -> std::result::Result<u8, E>
        where
            E: de::Error,
        {
            u8::try_from(value).map_err(de::Error::custom)
        }

        fn visit_str<E>(self, value: &str) -> std::result::Result<u8, E>
        where
            E: de::Error,
        {
            if value.is_empty() {
                Ok(0)
            } else {
                value.parse::<u8>().map_err(de::Error::custom)
            }
        }
    }

    deserializer.deserialize_any(U8Visitor)
}

fn deserialize_string_to_u64<'de, D>(deserializer: D) -> std::result::Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct U64Visitor;

    impl Visitor<'_> for U64Visitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or number representing a u64")
        }

        fn visit_u64<E>(self, value: u64) -> std::result::Result<u64, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> std::result::Result<u64, E>
        where
            E: de::Error,
        {
            u64::try_from(value).map_err(de::Error::custom)
        }

        fn visit_str<E>(self, value: &str) -> std::result::Result<u64, E>
        where
            E: de::Error,
        {
            if value.is_empty() {
                Ok(0)
            } else {
                value.parse::<u64>().map_err(de::Error::custom)
            }
        }
    }

    deserializer.deserialize_any(U64Visitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_history_item() {
        let json = r#"{
            "user": "alice",
            "rating_key": "12345",
            "grandparent_title": "Breaking Bad",
            "parent_media_index": "1",
            "media_index": "5",
            "percent_complete": "95",
            "stopped": "1234567890"
        }"#;

        let item: HistoryItem = serde_json::from_str(json).expect("Should deserialize");
        assert_eq!(item.user, "alice");
        assert_eq!(item.rating_key, "12345");
        assert_eq!(item.grandparent_title, "Breaking Bad");
        assert_eq!(item.parent_media_index, 1);
        assert_eq!(item.media_index, 5);
        assert_eq!(item.percent_complete, 95);
        assert_eq!(item.stopped, 1234567890);
    }

    #[test]
    fn test_deserialize_history_item_missing_grandparent() {
        let json = r#"{
            "user": "alice",
            "rating_key": "12345",
            "parent_media_index": "1",
            "media_index": "5",
            "percent_complete": "95",
            "stopped": "1234567890"
        }"#;

        let item: HistoryItem = serde_json::from_str(json).expect("Should deserialize");
        assert_eq!(item.grandparent_title, "");
    }

    #[test]
    fn test_url_normalization_with_trailing_slash() {
        let client =
            TautulliClient::new("http://localhost:8181/".to_string(), "test-key".to_string())
                .expect("Should create client");
        assert_eq!(client.base_url, "http://localhost:8181/");
    }

    #[test]
    fn test_url_normalization_without_trailing_slash() {
        let client =
            TautulliClient::new("http://localhost:8181".to_string(), "test-key".to_string())
                .expect("Should create client");
        assert_eq!(
            client.base_url, "http://localhost:8181/",
            "Should add trailing slash"
        );
    }

    #[test]
    fn test_deserialize_history_item_with_numbers() {
        let json = r#"{
            "user": "alice",
            "rating_key": 12345,
            "grandparent_title": "Breaking Bad",
            "parent_media_index": 1,
            "media_index": 5,
            "percent_complete": 95,
            "stopped": 1234567890
        }"#;

        let item: HistoryItem = serde_json::from_str(json).expect("Should deserialize numbers");
        assert_eq!(item.user, "alice");
        assert_eq!(item.rating_key, "12345");
        assert_eq!(item.grandparent_title, "Breaking Bad");
        assert_eq!(item.parent_media_index, 1);
        assert_eq!(item.media_index, 5);
        assert_eq!(item.percent_complete, 95);
        assert_eq!(item.stopped, 1234567890);
    }

    #[test]
    fn test_deserialize_history_item_mixed_formats() {
        let json = r#"{
            "user": "bob",
            "rating_key": "67890",
            "grandparent_title": "The Wire",
            "parent_media_index": "2",
            "media_index": 10,
            "percent_complete": "100",
            "stopped": 9876543210
        }"#;

        let item: HistoryItem =
            serde_json::from_str(json).expect("Should deserialize mixed formats");
        assert_eq!(item.user, "bob");
        assert_eq!(item.grandparent_title, "The Wire");
        assert_eq!(item.parent_media_index, 2);
        assert_eq!(item.media_index, 10);
        assert_eq!(item.percent_complete, 100);
        assert_eq!(item.stopped, 9876543210);
    }

    #[test]
    fn test_deserialize_history_item_with_empty_strings() {
        // Movies and music don't have season/episode numbers
        let json = r#"{
            "user": "charlie",
            "rating_key": 99999,
            "grandparent_title": "",
            "parent_media_index": "",
            "media_index": "",
            "percent_complete": 75,
            "stopped": 1234567890
        }"#;

        let item: HistoryItem =
            serde_json::from_str(json).expect("Should deserialize empty strings");
        assert_eq!(item.user, "charlie");
        assert_eq!(item.rating_key, "99999");
        assert_eq!(item.grandparent_title, "");
        assert_eq!(item.parent_media_index, 0); // Empty string becomes 0
        assert_eq!(item.media_index, 0); // Empty string becomes 0
        assert_eq!(item.percent_complete, 75);
        assert_eq!(item.stopped, 1234567890);
    }
}
