use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    SuperAdmin,
    NodeAdmin,
    BusinessLineAdmin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub role: UserRole,
    pub managed_nodes: Vec<Uuid>,
    pub managed_business_lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: Uuid,
    pub original_filename: String,
    pub storage_path: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub md5: Option<String>,
    pub sha256: String,
    pub uploaded_at: DateTime<Utc>,
    pub uploaded_by: String,
    pub business_line: String,
    pub tags: Vec<String>,
    pub version: String,
    pub version_group: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStats {
    pub hit_count: u64,
    pub miss_count: u64,
    pub last_accessed_at: Option<DateTime<Utc>>,
}

impl Default for ResourceStats {
    fn default() -> Self {
        Self {
            hit_count: 0,
            miss_count: 0,
            last_accessed_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Online,
    Offline,
    Maintenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    NorthChina,
    EastChina,
    SouthChina,
    CentralChina,
    Southwest,
    Northwest,
    Northeast,
}

impl Region {
    pub fn to_string(&self) -> &'static str {
        match self {
            Region::NorthChina => "华北",
            Region::EastChina => "华东",
            Region::SouthChina => "华南",
            Region::CentralChina => "华中",
            Region::Southwest => "西南",
            Region::Northwest => "西北",
            Region::Northeast => "东北",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "华北" => Some(Region::NorthChina),
            "华东" => Some(Region::EastChina),
            "华南" => Some(Region::SouthChina),
            "华中" => Some(Region::CentralChina),
            "西南" => Some(Region::Southwest),
            "西北" => Some(Region::Northwest),
            "东北" => Some(Region::Northeast),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Carrier {
    Telecom,
    Unicom,
    Mobile,
    MultiLine,
}

impl Carrier {
    pub fn to_string(&self) -> &'static str {
        match self {
            Carrier::Telecom => "电信",
            Carrier::Unicom => "联通",
            Carrier::Mobile => "移动",
            Carrier::MultiLine => "多线",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid,
    pub name: String,
    pub region: Region,
    pub datacenter_address: String,
    pub carrier: Carrier,
    pub capacity_gb: u64,
    pub used_gb: u64,
    pub status: NodeStatus,
    pub heartbeat_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ResourcePublishKey {
    pub resource_id: Uuid,
    pub node_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePublish {
    pub resource_id: Uuid,
    pub node_id: Uuid,
    pub published_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Done,
    Partial,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreheatTask {
    pub task_id: Uuid,
    pub resource_ids: Vec<Uuid>,
    pub node_ids: Vec<Uuid>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub total: u64,
    pub done: u64,
    pub failed: u64,
    pub failed_resources: Vec<Uuid>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub estimated_duration_secs: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PurgeType {
    ByNode,
    ByResource,
    ByTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgeTask {
    pub task_id: Uuid,
    pub purge_type: PurgeType,
    pub node_ids: Vec<Uuid>,
    pub resource_ids: Vec<Uuid>,
    pub days_not_accessed: Option<u32>,
    pub mime_types: Vec<String>,
    pub status: TaskStatus,
    pub total: u64,
    pub done: u64,
    pub failed: u64,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub dry_run: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayedPurgeItem {
    pub resource_id: Uuid,
    pub node_id: Uuid,
    pub expected_delete_time: DateTime<Utc>,
    pub task_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: Uuid,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub scope: String,
    pub timestamp: DateTime<Utc>,
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<DirectoryNode>,
    pub resource_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitRateEntry {
    pub key: String,
    pub hit_count: u64,
    pub miss_count: u64,
    pub hit_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendPoint {
    pub timestamp: DateTime<Utc>,
    pub requests: u64,
    pub hits: u64,
    pub hit_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverviewStats {
    pub total_resources: u64,
    pub total_nodes: u64,
    pub overall_hit_rate: f64,
    pub total_requests_today: u64,
    pub total_hits_today: u64,
    pub top_hot_resources: Vec<(String, u64)>,
    pub top_cold_resources: Vec<(String, u64)>,
    pub node_capacity_usage: Vec<(String, f64)>,
    pub active_preheat_tasks: u64,
    pub active_purge_tasks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
        }
    }

    pub fn error(message: &str) -> Self {
        Self {
            success: false,
            data: None,
            message: Some(message.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}
