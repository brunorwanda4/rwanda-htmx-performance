use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
};

use chrono::Utc;
use serde::Serialize;

const MAX_RECENT_RECORDS: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceRecord {
    pub timestamp: String,
    pub method: String,
    pub path: String,
    pub status: u16,
    pub is_htmx: bool,
    pub duration_ms: f64,
    pub response_size_bytes: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PerformanceAggregate {
    pub requests: u64,
    pub htmx_requests: u64,
    pub traditional_requests: u64,
    pub average_duration_ms: f64,
    pub average_response_size_bytes: f64,
    pub htmx_average_duration_ms: f64,
    pub traditional_average_duration_ms: f64,
    pub htmx_average_response_size_bytes: f64,
    pub traditional_average_response_size_bytes: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MetricsSnapshot {
    pub aggregate: PerformanceAggregate,
    pub recent: Vec<PerformanceRecord>,
}

#[derive(Default)]
struct MetricsState {
    recent: VecDeque<PerformanceRecord>,
    requests: u64,
    htmx_requests: u64,
    total_duration_ms: f64,
    htmx_duration_ms: f64,
    traditional_duration_ms: f64,
    sized_responses: u64,
    htmx_sized_responses: u64,
    traditional_sized_responses: u64,
    total_response_bytes: u64,
    htmx_response_bytes: u64,
    traditional_response_bytes: u64,
}

#[derive(Clone, Default)]
pub struct MetricsStore {
    inner: Arc<RwLock<MetricsState>>,
}

impl MetricsStore {
    pub fn record(
        &self,
        method: String,
        path: String,
        status: u16,
        is_htmx: bool,
        duration_ms: f64,
        response_size_bytes: Option<usize>,
    ) -> PerformanceRecord {
        let record = PerformanceRecord {
            timestamp: Utc::now().to_rfc3339(),
            method,
            path,
            status,
            is_htmx,
            duration_ms,
            response_size_bytes,
        };

        let mut state = self
            .inner
            .write()
            .unwrap_or_else(|error| error.into_inner());
        state.requests += 1;
        state.total_duration_ms += duration_ms;
        if is_htmx {
            state.htmx_requests += 1;
            state.htmx_duration_ms += duration_ms;
        } else {
            state.traditional_duration_ms += duration_ms;
        }

        if let Some(bytes) = response_size_bytes {
            state.sized_responses += 1;
            state.total_response_bytes += bytes as u64;
            if is_htmx {
                state.htmx_sized_responses += 1;
                state.htmx_response_bytes += bytes as u64;
            } else {
                state.traditional_sized_responses += 1;
                state.traditional_response_bytes += bytes as u64;
            }
        }

        state.recent.push_front(record.clone());
        state.recent.truncate(MAX_RECENT_RECORDS);
        record
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let state = self.inner.read().unwrap_or_else(|error| error.into_inner());
        let traditional_requests = state.requests.saturating_sub(state.htmx_requests);
        MetricsSnapshot {
            aggregate: PerformanceAggregate {
                requests: state.requests,
                htmx_requests: state.htmx_requests,
                traditional_requests,
                average_duration_ms: average(state.total_duration_ms, state.requests),
                average_response_size_bytes: average(
                    state.total_response_bytes as f64,
                    state.sized_responses,
                ),
                htmx_average_duration_ms: average(state.htmx_duration_ms, state.htmx_requests),
                traditional_average_duration_ms: average(
                    state.traditional_duration_ms,
                    traditional_requests,
                ),
                htmx_average_response_size_bytes: average(
                    state.htmx_response_bytes as f64,
                    state.htmx_sized_responses,
                ),
                traditional_average_response_size_bytes: average(
                    state.traditional_response_bytes as f64,
                    state.traditional_sized_responses,
                ),
            },
            recent: state.recent.iter().take(12).cloned().collect(),
        }
    }
}

fn average(total: f64, count: u64) -> f64 {
    if count == 0 {
        0.0
    } else {
        total / count as f64
    }
}
