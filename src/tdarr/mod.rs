use crate::config::{PathMappingConfig, TdarrBlockerConfig};
use crate::error::{AppError, Result};
use crate::move_blocker::{BlockDecision, BlockReason, MoveBlocker, MoveBlockerSnapshot};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

const TDARR_MOVE_BLOCKER_NAME: &str = "tdarr";

pub struct TdarrMoveBlocker {
    config: TdarrBlockerConfig,
    base_url: String,
    client: Client,
}

impl TdarrMoveBlocker {
    pub fn new(config: TdarrBlockerConfig) -> Result<Self> {
        let base_url = config.url.trim().trim_end_matches('/').to_string();
        if base_url.is_empty() {
            return Err(AppError::Config(
                "Tdarr blocker URL must not be empty".to_string(),
            ));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::Io(std::io::Error::other(e)))?;

        Ok(Self {
            config,
            base_url,
            client,
        })
    }

    fn collect_active_workers(
        &self,
        blocked_paths: &mut HashMap<PathBuf, BlockReason>,
    ) -> Result<()> {
        let response = self.get_json("api/v2/get-nodes")?;

        for (app_path, reason) in collect_worker_paths(&response) {
            self.insert_blocked_app_path(blocked_paths, &app_path, &reason);
        }

        Ok(())
    }

    fn collect_staged(&self, blocked_paths: &mut HashMap<PathBuf, BlockReason>) -> Result<()> {
        self.collect_client_rows(
            "api/v2/client/staged",
            &[],
            "staged or processing in Tdarr",
            blocked_paths,
        )
    }

    fn collect_status_table(
        &self,
        filter_id: &str,
        filter_value: &str,
        reason: &str,
        blocked_paths: &mut HashMap<PathBuf, BlockReason>,
    ) -> Result<()> {
        let filters = [ClientFilter {
            id: filter_id.to_string(),
            value: filter_value.to_string(),
        }];

        self.collect_client_rows(
            "api/v2/client/status-tables",
            &filters,
            reason,
            blocked_paths,
        )
    }

    fn collect_client_rows(
        &self,
        endpoint: &str,
        filters: &[ClientFilter],
        reason: &str,
        blocked_paths: &mut HashMap<PathBuf, BlockReason>,
    ) -> Result<()> {
        let page_size = self.config.page_size.max(1);
        let mut start = 0;

        loop {
            let request = ClientRequest {
                data: ClientRequestData {
                    start,
                    page_size,
                    filters: filters.to_vec(),
                    sorts: Vec::new(),
                    opts: serde_json::json!({}),
                },
            };
            let response: ClientResponse = self.post_json(endpoint, &request)?;
            let rows_read = response.array.len();

            for row in response.array {
                for app_path in collect_row_paths(&row) {
                    self.insert_blocked_app_path(blocked_paths, &app_path, reason);
                }
            }

            if rows_read == 0 || start + rows_read >= response.total_count {
                break;
            }

            start += rows_read;
        }

        Ok(())
    }

    fn get_json(&self, endpoint: &str) -> Result<Value> {
        let url = self.endpoint_url(endpoint);
        let response = self.client.get(&url).send().map_err(|e| {
            AppError::External(format!("Failed to query Tdarr endpoint {url}: {e}"))
        })?;

        if !response.status().is_success() {
            return Err(AppError::External(format!(
                "Tdarr endpoint {url} returned HTTP {}",
                response.status()
            )));
        }

        response
            .json()
            .map_err(|e| AppError::External(format!("Failed to parse Tdarr response: {e}")))
    }

    fn post_json<T: Serialize>(&self, endpoint: &str, body: &T) -> Result<ClientResponse> {
        let url = self.endpoint_url(endpoint);
        let response = self.client.post(&url).json(body).send().map_err(|e| {
            AppError::External(format!("Failed to query Tdarr endpoint {url}: {e}"))
        })?;

        if !response.status().is_success() {
            return Err(AppError::External(format!(
                "Tdarr endpoint {url} returned HTTP {}",
                response.status()
            )));
        }

        response
            .json()
            .map_err(|e| AppError::External(format!("Failed to parse Tdarr response: {e}")))
    }

    fn endpoint_url(&self, endpoint: &str) -> String {
        format!("{}/{}", self.base_url, endpoint.trim_start_matches('/'))
    }

    fn insert_blocked_app_path(
        &self,
        blocked_paths: &mut HashMap<PathBuf, BlockReason>,
        app_path: &str,
        reason: &str,
    ) {
        let block_reason = BlockReason {
            provider: self.name().to_string(),
            reason: reason.to_string(),
        };

        for host_path in map_app_path(app_path, &self.config.path_mappings) {
            blocked_paths
                .entry(host_path)
                .or_insert_with(|| block_reason.clone());
        }
    }
}

impl MoveBlocker for TdarrMoveBlocker {
    fn name(&self) -> &str {
        TDARR_MOVE_BLOCKER_NAME
    }

    fn snapshot(&self, candidates: &[PathBuf]) -> Result<Box<dyn MoveBlockerSnapshot>> {
        tracing::debug!(
            "Building Tdarr move-blocker snapshot for {} candidate moves",
            candidates.len()
        );

        let status = self.get_json("api/v2/status")?;
        if let Some(version) = status.get("version").and_then(Value::as_str) {
            tracing::debug!("Connected to Tdarr {version}");
        }

        let mut blocked_paths = HashMap::new();

        if self.config.block_active_workers {
            self.collect_active_workers(&mut blocked_paths)?;
        }
        if self.config.block_staged {
            self.collect_staged(&mut blocked_paths)?;
        }
        if self.config.block_queued_transcode {
            self.collect_status_table(
                "TranscodeDecisionMaker",
                "Queued",
                "queued for Tdarr transcode",
                &mut blocked_paths,
            )?;
        }
        if self.config.block_queued_healthcheck {
            self.collect_status_table(
                "HealthCheck",
                "Queued",
                "queued for Tdarr health check",
                &mut blocked_paths,
            )?;
        }

        tracing::info!(
            "Tdarr move-blocker snapshot loaded: {} host paths blocked",
            blocked_paths.len()
        );

        Ok(Box::new(TdarrMoveBlockerSnapshot { blocked_paths }))
    }
}

struct TdarrMoveBlockerSnapshot {
    blocked_paths: HashMap<PathBuf, BlockReason>,
}

impl MoveBlockerSnapshot for TdarrMoveBlockerSnapshot {
    fn check(&self, path: &Path) -> BlockDecision {
        self.blocked_paths
            .get(path)
            .map_or(BlockDecision::Allowed, |reason| {
                BlockDecision::Blocked(reason.clone())
            })
    }
}

#[derive(Debug, Clone, Serialize)]
struct ClientRequest {
    data: ClientRequestData,
}

#[derive(Debug, Clone, Serialize)]
struct ClientRequestData {
    start: usize,
    #[serde(rename = "pageSize")]
    page_size: usize,
    filters: Vec<ClientFilter>,
    sorts: Vec<Value>,
    opts: Value,
}

#[derive(Debug, Clone, Serialize)]
struct ClientFilter {
    id: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct ClientResponse {
    #[serde(default)]
    array: Vec<Value>,

    #[serde(default, rename = "totalCount")]
    total_count: usize,
}

fn map_app_path(app_path: &str, mappings: &[PathMappingConfig]) -> Vec<PathBuf> {
    let app_path = app_path.trim();
    let mut mapped_paths = Vec::new();

    for mapping in mappings {
        if let Some(relative_path) = strip_app_prefix(app_path, &mapping.app_prefix) {
            let mut host_path = mapping.host_prefix.clone();
            if !relative_path.is_empty() {
                host_path.push(relative_path);
            }
            mapped_paths.push(host_path);
        }
    }

    if mapped_paths.is_empty() {
        mapped_paths.push(PathBuf::from(app_path));
    }

    mapped_paths.sort();
    mapped_paths.dedup();
    mapped_paths
}

fn strip_app_prefix<'a>(app_path: &'a str, app_prefix: &str) -> Option<&'a str> {
    let app_prefix = app_prefix.trim().trim_end_matches('/');

    if app_path == app_prefix {
        return Some("");
    }

    let prefix_with_separator = format!("{app_prefix}/");
    app_path.strip_prefix(&prefix_with_separator)
}

fn collect_worker_paths(value: &Value) -> Vec<(String, String)> {
    let mut paths = Vec::new();
    collect_worker_paths_inner(value, &mut paths);
    paths
}

fn collect_worker_paths_inner(value: &Value, paths: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            if let Some(file_path) = map.get("file").and_then(Value::as_str)
                && looks_like_path(file_path)
            {
                let worker_type = map
                    .get("workerType")
                    .and_then(Value::as_str)
                    .unwrap_or("worker");
                let status = map
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("active");
                paths.push((
                    file_path.to_string(),
                    format!("active Tdarr {worker_type} worker ({status})"),
                ));
            }

            for child in map.values() {
                collect_worker_paths_inner(child, paths);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_worker_paths_inner(child, paths);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn collect_row_paths(row: &Value) -> Vec<String> {
    let mut paths = Vec::new();

    push_path_if_present(row.get("file").and_then(Value::as_str), &mut paths);
    push_path_if_present(row.get("_id").and_then(Value::as_str), &mut paths);
    push_path_if_present(
        row.pointer("/originalLibraryFile/file")
            .and_then(Value::as_str),
        &mut paths,
    );

    paths.sort();
    paths.dedup();
    paths
}

fn push_path_if_present(value: Option<&str>, paths: &mut Vec<String>) {
    if let Some(path) = value
        && looks_like_path(path)
    {
        paths.push(path.to_string());
    }
}

fn looks_like_path(value: &str) -> bool {
    value.contains('/') || value.contains('\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(host_prefix: &str, app_prefix: &str) -> PathMappingConfig {
        PathMappingConfig {
            host_prefix: PathBuf::from(host_prefix),
            app_prefix: app_prefix.to_string(),
        }
    }

    #[test]
    fn test_map_app_path_to_multiple_host_prefixes() {
        let mapped = map_app_path(
            "/media/movies/A/Movie.mkv",
            &[
                mapping("/mnt/tier1/media/movies", "/media/movies"),
                mapping("/mnt/tier2/media/movies", "/media/movies/"),
            ],
        );

        assert_eq!(
            mapped,
            vec![
                PathBuf::from("/mnt/tier1/media/movies/A/Movie.mkv"),
                PathBuf::from("/mnt/tier2/media/movies/A/Movie.mkv"),
            ]
        );
    }

    #[test]
    fn test_map_app_path_does_not_match_partial_prefix() {
        let mapped = map_app_path(
            "/media/movies-extra/Movie.mkv",
            &[mapping("/mnt/tier1/media/movies", "/media/movies")],
        );

        assert_eq!(mapped, vec![PathBuf::from("/media/movies-extra/Movie.mkv")]);
    }

    #[test]
    fn test_collect_row_paths_prefers_file_like_values() {
        let row = serde_json::json!({
            "_id": "not-a-path-id",
            "file": "/media/tv/Show/S01E01.mkv",
            "originalLibraryFile": {
                "file": "/media/tv/Show/S01E01-original.mkv"
            }
        });

        let paths = collect_row_paths(&row);

        assert_eq!(
            paths,
            vec![
                "/media/tv/Show/S01E01-original.mkv".to_string(),
                "/media/tv/Show/S01E01.mkv".to_string(),
            ]
        );
    }

    #[test]
    fn test_collect_worker_paths() {
        let nodes = serde_json::json!({
            "node-a": {
                "workers": {
                    "worker-a": {
                        "file": "/media/tv/Show/S01E01.mkv",
                        "workerType": "transcode",
                        "status": "running"
                    }
                }
            }
        });

        let paths = collect_worker_paths(&nodes);

        assert_eq!(
            paths,
            vec![(
                "/media/tv/Show/S01E01.mkv".to_string(),
                "active Tdarr transcode worker (running)".to_string(),
            )]
        );
    }
}
