use crate::level::LogLevel;
use crate::message::LogMessage;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::VecDeque;

const DEFAULT_CAPACITY: usize = 1000;

#[derive(Clone, Serialize)]
pub struct RecentLogEntry {
    pub id: u64,
    pub logger_name: String,
    pub level: LogLevel,
    pub message: String,
    pub file: String,
    pub line: u32,
    pub thread_id: u64,
    pub timestamp: u64,
    pub formatted: String,
}

pub struct RecentLogStore {
    capacity: usize,
    next_id: u64,
    entries: VecDeque<RecentLogEntry>,
}

impl RecentLogStore {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            next_id: 1,
            entries: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, msg: &LogMessage, formatted: String) {
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }

        let entry = RecentLogEntry {
            id: self.next_id,
            logger_name: msg.name.clone(),
            level: msg.level,
            message: msg.payload.clone(),
            file: msg.file.clone(),
            line: msg.line,
            thread_id: msg.thread_id,
            timestamp: msg.timestamp,
            formatted,
        };

        self.next_id += 1;
        self.entries.push_back(entry);
    }

    pub fn list(
        &self,
        limit: usize,
        level: Option<LogLevel>,
        keyword: Option<&str>,
    ) -> Vec<RecentLogEntry> {
        let keyword = keyword.map(str::to_lowercase);

        self.entries
            .iter()
            .rev()
            .filter(|entry| level.map_or(true, |expected| entry.level == expected))
            .filter(|entry| {
                keyword.as_ref().map_or(true, |keyword| {
                    entry.message.to_lowercase().contains(keyword)
                        || entry.logger_name.to_lowercase().contains(keyword)
                        || entry.formatted.to_lowercase().contains(keyword)
                })
            })
            .take(limit)
            .cloned()
            .collect()
    }
}

pub static RECENT_LOGS: Lazy<Mutex<RecentLogStore>> =
    Lazy::new(|| Mutex::new(RecentLogStore::new(DEFAULT_CAPACITY)));

pub fn record(msg: &LogMessage, formatted: String) {
    RECENT_LOGS.lock().push(msg, formatted);
}

pub fn recent(limit: usize, level: Option<LogLevel>, keyword: Option<&str>) -> Vec<RecentLogEntry> {
    RECENT_LOGS.lock().list(limit, level, keyword)
}
