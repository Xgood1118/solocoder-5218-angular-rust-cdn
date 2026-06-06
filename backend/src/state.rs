use crate::models::*;
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;
use priority_queue::PriorityQueue;
use std::cmp::Reverse;
use chrono::{DateTime, Utc};

pub struct AppState {
    pub resources: RwLock<HashMap<Uuid, Resource>>,
    pub resource_stats: RwLock<HashMap<(Uuid, Uuid), ResourceStats>>,
    pub nodes: RwLock<HashMap<Uuid, Node>>,
    pub resource_publishes: RwLock<HashMap<(Uuid, Uuid), ResourcePublish>>,
    pub preheat_tasks: RwLock<HashMap<Uuid, PreheatTask>>,
    pub preheat_queue: RwLock<PriorityQueue<Uuid, Reverse<TaskPriority>>>,
    pub purge_tasks: RwLock<HashMap<Uuid, PurgeTask>>,
    pub delayed_purge_queue: RwLock<PriorityQueue<DelayedPurgeItem, Reverse<DateTime<Utc>>>>,
    pub audit_logs: RwLock<Vec<AuditLog>>,
    pub request_history: RwLock<Vec<(DateTime<Utc>, bool, Option<Uuid>)>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            resources: RwLock::new(HashMap::new()),
            resource_stats: RwLock::new(HashMap::new()),
            nodes: RwLock::new(HashMap::new()),
            resource_publishes: RwLock::new(HashMap::new()),
            preheat_tasks: RwLock::new(HashMap::new()),
            preheat_queue: RwLock::new(PriorityQueue::new()),
            purge_tasks: RwLock::new(HashMap::new()),
            delayed_purge_queue: RwLock::new(PriorityQueue::new()),
            audit_logs: RwLock::new(Vec::new()),
            request_history: RwLock::new(Vec::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for DelayedPurgeItem {
    fn eq(&self, other: &Self) -> bool {
        self.resource_id == other.resource_id
            && self.node_id == other.node_id
            && self.expected_delete_time == other.expected_delete_time
    }
}

impl Eq for DelayedPurgeItem {}

impl std::hash::Hash for DelayedPurgeItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.resource_id.hash(state);
        self.node_id.hash(state);
        self.expected_delete_time.hash(state);
    }
}

impl Ord for DelayedPurgeItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.expected_delete_time.cmp(&other.expected_delete_time)
    }
}

impl PartialOrd for DelayedPurgeItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
