use crate::state::AppState;
use crate::models::*;
use crate::audit;
use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};
use std::cmp::Reverse;
use priority_queue::PriorityQueue;

#[derive(Debug, Deserialize)]
pub struct CreatePurgeRequest {
    pub purge_type: String,
    pub node_ids: Option<Vec<Uuid>>,
    pub resource_ids: Option<Vec<Uuid>>,
    pub days_not_accessed: Option<u32>,
    pub mime_types: Option<Vec<String>>,
    pub created_by: String,
    pub dry_run: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PurgeTaskQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub status: Option<String>,
    pub purge_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DryRunResult {
    pub total_resources: u64,
    pub resources: Vec<PurgeResourceInfo>,
    pub estimated_savings_gb: f64,
}

#[derive(Debug, Serialize)]
pub struct PurgeResourceInfo {
    pub resource_id: Uuid,
    pub resource_name: String,
    pub node_id: Uuid,
    pub node_name: String,
    pub size_bytes: u64,
    pub last_accessed: Option<DateTime<Utc>>,
}

pub async fn create_purge_task(
    state: web::Data<Arc<AppState>>,
    req: web::Json<CreatePurgeRequest>,
) -> impl Responder {
    let purge_type = match req.purge_type.as_str() {
        "by_node" => PurgeType::ByNode,
        "by_resource" => PurgeType::ByResource,
        "by_time" => PurgeType::ByTime,
        _ => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid purge type"));
        }
    };

    let node_ids = req.node_ids.clone().unwrap_or_default();
    let resource_ids = req.resource_ids.clone().unwrap_or_default();

    match purge_type {
        PurgeType::ByNode => {
            if node_ids.is_empty() {
                return HttpResponse::BadRequest().json(ApiResponse::<()>::error("No nodes specified"));
            }
        }
        PurgeType::ByResource => {
            if resource_ids.is_empty() {
                return HttpResponse::BadRequest().json(ApiResponse::<()>::error("No resources specified"));
            }
        }
        PurgeType::ByTime => {
            if node_ids.is_empty() {
                return HttpResponse::BadRequest().json(ApiResponse::<()>::error("No nodes specified for time-based purge"));
            }
            if req.days_not_accessed.is_none() {
                return HttpResponse::BadRequest().json(ApiResponse::<()>::error("days_not_accessed is required for time-based purge"));
            }
        }
    }

    let nodes = state.nodes.read().await;
    for nid in &node_ids {
        if !nodes.contains_key(nid) {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(&format!(
                "Node {} not found",
                nid
            )));
        }
    }
    drop(nodes);

    let resources = state.resources.read().await;
    for rid in &resource_ids {
        if !resources.contains_key(rid) {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(&format!(
                "Resource {} not found",
                rid
            )));
        }
    }
    drop(resources);

    let dry_run = req.dry_run.unwrap_or(false);
    let mime_types = req.mime_types.clone().unwrap_or_default();

    let target_list = match purge_type {
        PurgeType::ByNode => {
            let publishes = state.resource_publishes.read().await;
            let mut count = 0u64;
            for nid in &node_ids {
                count += publishes.values().filter(|p| p.node_id == *nid).count() as u64;
            }
            count
        }
        PurgeType::ByResource => {
            let publishes = state.resource_publishes.read().await;
            let mut count = 0u64;
            for rid in &resource_ids {
                count += publishes.values().filter(|p| p.resource_id == *rid).count() as u64;
            }
            count
        }
        PurgeType::ByTime => {
            let stats = state.resource_stats.read().await;
            let resources = state.resources.read().await;
            let mut count = 0u64;
            let cutoff = Utc::now() - Duration::days(req.days_not_accessed.unwrap() as i64);

            for ((rid, nid), s) in stats.iter() {
                if !node_ids.contains(nid) {
                    continue;
                }
                if !mime_types.is_empty() {
                    if let Some(r) = resources.get(rid) {
                        if !mime_types.iter().any(|m| r.mime_type.starts_with(m)) {
                            continue;
                        }
                    }
                }
                if let Some(last) = s.last_accessed_at {
                    if last < cutoff {
                        count += 1;
                    }
                }
            }
            count
        }
    };

    let task = PurgeTask {
        task_id: Uuid::new_v4(),
        purge_type: purge_type.clone(),
        node_ids: node_ids.clone(),
        resource_ids: resource_ids.clone(),
        days_not_accessed: req.days_not_accessed,
        mime_types,
        status: TaskStatus::Pending,
        total: target_list,
        done: 0,
        failed: 0,
        started_at: None,
        finished_at: None,
        created_by: req.created_by.clone(),
        dry_run,
        created_at: Utc::now(),
    };

    let task_id = task.task_id;
    state.purge_tasks.write().await.insert(task_id, task);

    if !dry_run {
        let state_clone = state.get_ref().clone();
        tokio::spawn(async move {
            execute_purge_task(state_clone, task_id).await;
        });
    }

    audit::log_audit(
        &state,
        &req.created_by,
        "create_purge",
        &task_id.to_string(),
        &format!("{:?}", purge_type),
        if dry_run { "dry_run" } else { "success" },
    )
    .await;

    HttpResponse::Created().json(ApiResponse::ok(task_id))
}

pub async fn list_purge_tasks(
    state: web::Data<Arc<AppState>>,
    query: web::Query<PurgeTaskQuery>,
) -> impl Responder {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20).min(100);

    let tasks = state.purge_tasks.read().await;

    let mut filtered: Vec<&PurgeTask> = tasks.values().collect();

    if let Some(status) = &query.status {
        let status_enum = match status.as_str() {
            "pending" => TaskStatus::Pending,
            "running" => TaskStatus::Running,
            "done" => TaskStatus::Done,
            "partial" => TaskStatus::Partial,
            "failed" => TaskStatus::Failed,
            "cancelled" => TaskStatus::Cancelled,
            _ => {
                return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid status"));
            }
        };
        filtered.retain(|t| t.status == status_enum);
    }

    if let Some(ptype) = &query.purge_type {
        let ptype_enum = match ptype.as_str() {
            "by_node" => PurgeType::ByNode,
            "by_resource" => PurgeType::ByResource,
            "by_time" => PurgeType::ByTime,
            _ => {
                return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid purge type"));
            }
        };
        filtered.retain(|t| t.purge_type == ptype_enum);
    }

    filtered.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let total = filtered.len() as u64;
    let start = ((page - 1) * page_size) as usize;

    let items: Vec<PurgeTask> = filtered
        .into_iter()
        .skip(start)
        .take(page_size as usize)
        .cloned()
        .collect();

    let result = serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "page_size": page_size,
    });

    HttpResponse::Ok().json(ApiResponse::ok(result))
}

pub async fn get_purge_task(
    state: web::Data<Arc<AppState>>,
    task_id: web::Path<Uuid>,
) -> impl Responder {
    let tasks = state.purge_tasks.read().await;
    let task = match tasks.get(&task_id) {
        Some(t) => t,
        None => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error("Task not found"));
        }
    };

    HttpResponse::Ok().json(ApiResponse::ok(task.clone()))
}

pub async fn dry_run_purge(
    state: web::Data<Arc<AppState>>,
    req: web::Json<CreatePurgeRequest>,
) -> impl Responder {
    let purge_type = match req.purge_type.as_str() {
        "by_node" => PurgeType::ByNode,
        "by_resource" => PurgeType::ByResource,
        "by_time" => PurgeType::ByTime,
        _ => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid purge type"));
        }
    };

    let node_ids = req.node_ids.clone().unwrap_or_default();
    let resource_ids = req.resource_ids.clone().unwrap_or_default();
    let mime_types = req.mime_types.clone().unwrap_or_default();

    let resources_map = state.resources.read().await;
    let nodes_map = state.nodes.read().await;
    let publishes = state.resource_publishes.read().await;
    let stats = state.resource_stats.read().await;

    let mut result_resources = Vec::new();
    let mut total_size: u64 = 0;

    match purge_type {
        PurgeType::ByNode => {
            for nid in &node_ids {
                for publish in publishes.values() {
                    if &publish.node_id != nid {
                        continue;
                    }
                    if let Some(r) = resources_map.get(&publish.resource_id) {
                        if !mime_types.is_empty() {
                            if !mime_types.iter().any(|m| r.mime_type.starts_with(m)) {
                                continue;
                            }
                        }
                        let s = stats.get(&(publish.resource_id, publish.node_id));
                        total_size += r.size_bytes;
                        result_resources.push(PurgeResourceInfo {
                            resource_id: r.id,
                            resource_name: r.original_filename.clone(),
                            node_id: *nid,
                            node_name: nodes_map.get(nid).map(|n| n.name.clone()).unwrap_or_default(),
                            size_bytes: r.size_bytes,
                            last_accessed: s.and_then(|s| s.last_accessed_at),
                        });
                    }
                }
            }
        }
        PurgeType::ByResource => {
            for rid in &resource_ids {
                if let Some(r) = resources_map.get(rid) {
                    if !mime_types.is_empty() {
                        if !mime_types.iter().any(|m| r.mime_type.starts_with(m)) {
                            continue;
                        }
                    }
                    for publish in publishes.values() {
                        if &publish.resource_id != rid {
                            continue;
                        }
                        let s = stats.get(&(*rid, publish.node_id));
                        total_size += r.size_bytes;
                        result_resources.push(PurgeResourceInfo {
                            resource_id: r.id,
                            resource_name: r.original_filename.clone(),
                            node_id: publish.node_id,
                            node_name: nodes_map.get(&publish.node_id).map(|n| n.name.clone()).unwrap_or_default(),
                            size_bytes: r.size_bytes,
                            last_accessed: s.and_then(|s| s.last_accessed_at),
                        });
                    }
                }
            }
        }
        PurgeType::ByTime => {
            let days = req.days_not_accessed.unwrap_or(7);
            let cutoff = Utc::now() - Duration::days(days as i64);

            for ((rid, nid), s) in stats.iter() {
                if !node_ids.contains(nid) {
                    continue;
                }
                if let Some(last) = s.last_accessed_at {
                    if last >= cutoff {
                        continue;
                    }
                }

                if let Some(r) = resources_map.get(rid) {
                    if !mime_types.is_empty() {
                        if !mime_types.iter().any(|m| r.mime_type.starts_with(m)) {
                            continue;
                        }
                    }
                    total_size += r.size_bytes;
                    result_resources.push(PurgeResourceInfo {
                        resource_id: *rid,
                        resource_name: r.original_filename.clone(),
                        node_id: *nid,
                        node_name: nodes_map.get(nid).map(|n| n.name.clone()).unwrap_or_default(),
                        size_bytes: r.size_bytes,
                        last_accessed: s.last_accessed_at,
                    });
                }
            }
        }
    }

    result_resources.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    let result = DryRunResult {
        total_resources: result_resources.len() as u64,
        resources: result_resources,
        estimated_savings_gb: total_size as f64 / (1024.0 * 1024.0 * 1024.0),
    };

    HttpResponse::Ok().json(ApiResponse::ok(result))
}

async fn execute_purge_task(state: Arc<AppState>, task_id: Uuid) {
    let task = {
        let mut tasks = state.purge_tasks.write().await;
        if let Some(t) = tasks.get_mut(&task_id) {
            t.status = TaskStatus::Running;
            t.started_at = Some(Utc::now());
            t.clone()
        } else {
            return;
        }
    };

    log::info!("Starting purge task {}", task_id);

    let delete_time = Utc::now() + Duration::minutes(5);

    let mut items_to_purge: Vec<(Uuid, Uuid)> = Vec::new();

    let resources_map = state.resources.read().await;
    let publishes = state.resource_publishes.read().await;
    let stats = state.resource_stats.read().await;

    match task.purge_type {
        PurgeType::ByNode => {
            for nid in &task.node_ids {
                for publish in publishes.values() {
                    if &publish.node_id == nid {
                        if !task.mime_types.is_empty() {
                            if let Some(r) = resources_map.get(&publish.resource_id) {
                                if !task.mime_types.iter().any(|m| r.mime_type.starts_with(m)) {
                                    continue;
                                }
                            }
                        }
                        items_to_purge.push((publish.resource_id, publish.node_id));
                    }
                }
            }
        }
        PurgeType::ByResource => {
            for rid in &task.resource_ids {
                for publish in publishes.values() {
                    if &publish.resource_id == rid {
                        if !task.mime_types.is_empty() {
                            if let Some(r) = resources_map.get(rid) {
                                if !task.mime_types.iter().any(|m| r.mime_type.starts_with(m)) {
                                    continue;
                                }
                            }
                        }
                        items_to_purge.push((publish.resource_id, publish.node_id));
                    }
                }
            }
        }
        PurgeType::ByTime => {
            let days = task.days_not_accessed.unwrap_or(7);
            let cutoff = Utc::now() - Duration::days(days as i64);

            for ((rid, nid), s) in stats.iter() {
                if !task.node_ids.contains(nid) {
                    continue;
                }
                if let Some(last) = s.last_accessed_at {
                    if last >= cutoff {
                        continue;
                    }
                }
                if !task.mime_types.is_empty() {
                    if let Some(r) = resources_map.get(rid) {
                        if !task.mime_types.iter().any(|m| r.mime_type.starts_with(m)) {
                            continue;
                        }
                    }
                }
                items_to_purge.push((*rid, *nid));
            }
        }
    }

    drop(resources_map);
    drop(publishes);
    drop(stats);

    let total = items_to_purge.len() as u64;

    {
        let mut tasks = state.purge_tasks.write().await;
        if let Some(t) = tasks.get_mut(&task_id) {
            t.total = total;
        }
    }

    let mut queue = state.delayed_purge_queue.write().await;
    for (rid, nid) in &items_to_purge {
        queue.push(
            DelayedPurgeItem {
                resource_id: *rid,
                node_id: *nid,
                expected_delete_time: delete_time,
                task_id,
            },
            Reverse(delete_time),
        );
    }
    drop(queue);

    {
        let mut tasks = state.purge_tasks.write().await;
        if let Some(t) = tasks.get_mut(&task_id) {
            t.done = total;
            t.status = TaskStatus::Done;
            t.finished_at = Some(Utc::now());
        }
    }

    log::info!(
        "Purge task {} scheduled {} items for delayed deletion",
        task_id,
        total
    );
}

pub async fn delayed_cleanup_worker(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

        let now = Utc::now();
        let mut items_to_delete = Vec::new();

        {
            let mut queue = state.delayed_purge_queue.write().await;
            while let Some((item, _)) = queue.peek() {
                if item.expected_delete_time <= now {
                    if let Some((item, _)) = queue.pop() {
                        items_to_delete.push(item);
                    }
                } else {
                    break;
                }
            }
        }

        if !items_to_delete.is_empty() {
            let mut publishes = state.resource_publishes.write().await;
            let mut stats = state.resource_stats.write().await;
            let nodes = state.nodes.read().await;
            let resources = state.resources.read().await;

            for item in &items_to_delete {
                publishes.remove(&(item.resource_id, item.node_id));
                stats.remove(&(item.resource_id, item.node_id));

                if let Some(node) = nodes.get(&item.node_id) {
                    if let Some(resource) = resources.get(&item.resource_id) {
                        // Note: In a real system, we'd update used_gb here
                    }
                }
            }

            drop(publishes);
            drop(stats);
            drop(nodes);
            drop(resources);

            log::info!("Delayed cleanup: removed {} items", items_to_delete.len());
        }
    }
}
