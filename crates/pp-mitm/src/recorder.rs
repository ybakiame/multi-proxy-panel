//! MITM 流量记录。

use std::collections::VecDeque;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 一条被捕获的 HTTP 交换记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrafficRecord {
    pub id: Uuid,
    pub method: String,
    pub url: String,
    pub request_headers: Vec<(String, String)>,
    pub request_body: Option<String>,
    pub response_status: u16,
    pub response_headers: Vec<(String, String)>,
    pub response_body: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
}

/// 流量记录持久化抽象。
pub trait TrafficRecorder: Send + Sync {
    fn record(&self, rec: TrafficRecord);
    fn list(&self) -> Vec<TrafficRecord>;
}

/// 内存环形缓冲记录器，最多保留 `cap` 条记录。
#[derive(Debug)]
pub struct MemoryRecorder {
    cap: usize,
    inner: Mutex<VecDeque<TrafficRecord>>,
}

impl MemoryRecorder {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            inner: Mutex::new(VecDeque::new()),
        }
    }
}

impl TrafficRecorder for MemoryRecorder {
    fn record(&self, rec: TrafficRecord) {
        if self.cap == 0 {
            return;
        }
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if inner.len() >= self.cap {
            inner.pop_front();
        }
        inner.push_back(rec);
    }

    fn list(&self) -> Vec<TrafficRecord> {
        let inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(url: &str) -> TrafficRecord {
        TrafficRecord {
            id: Uuid::new_v4(),
            method: "GET".to_string(),
            url: url.to_string(),
            request_headers: Vec::new(),
            request_body: None,
            response_status: 200,
            response_headers: Vec::new(),
            response_body: None,
            timestamp: Utc::now(),
            duration_ms: 0,
        }
    }

    #[test]
    fn memory_recorder_evicts_oldest_when_full() {
        let recorder = MemoryRecorder::new(3);
        for i in 0..5 {
            recorder.record(record(&format!("http://example.com/{i}")));
        }
        let list = recorder.list();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].url, "http://example.com/2");
        assert_eq!(list[2].url, "http://example.com/4");
    }

    #[test]
    fn memory_recorder_cap_zero_keeps_nothing() {
        let recorder = MemoryRecorder::new(0);
        recorder.record(record("http://example.com/"));
        assert!(recorder.list().is_empty());
    }
}
