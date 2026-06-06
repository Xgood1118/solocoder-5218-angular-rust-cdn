use crate::state::AppState;
use crate::models::*;
use crate::audit;
use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};
use std::cmp::Reverse;

#[derive(Debug, Deserialize)]
pub struct CreatePreheatRequest {
    pub resource_ids: Vec<Uuid>,
    pub node_ids: Vec<Uuid>,
    pub priority: Option<String>,
    pub created_by: String,
}

#[derive(Debug, Deserialize)]
pub struct PreheatTaskQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub status: Option<String>,
}

pub async fn create_preheat_task(
    state: web::Data<Arc<AppState>>,
    req: web::Json<CreatePreheatRequest>,
) -> impl Responder {
    if req.resource_ids.is_empty() {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("No resources specified"));
    }
    if req.node_ids.is_empty() {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("No nodes specified"));
    }

    let resources = state.resources.read().await;
    for rid in &req.resource_ids {
        if !resources.contains_key(rid) {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(&format!(
                "Resource {} not found",
                rid
            )));
        }
    }
    drop(resources);

    let nodes = state.nodes.read().await;
    for nid in &req.node_ids {
        if !nodes.contains_key(nid) {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(&format!(
                "Node {} not found",
                nid
            )));
        }
    }
    drop(nodes);

    let priority = match req.priority.as_deref() {
        Some("high") => TaskPriority::High,
        Some("medium") => TaskPriority::Medium,
        Some("low") => TaskPriority::Low,
        _ => TaskPriority::Medium,
    };

    let total = (req.resource_ids.len() * req.node_ids.len()) as u64;

    let resources = state.resources.read().await;
    let total_size: u64 = req
        .resource_ids
        .iter()
        .filter_map(|rid| resources.get(rid).map(|r| r.size_bytes))
        .sum();
    drop(resources);

    let estimated_duration_secs = (total_size / (10 * 1024 * 1024)).max(10);

    let task = PreheatTask {
        task_id: Uuid::new_v4(),
        resource_ids: req.resource_ids.clone(),
        node_ids: req.node_ids.clone(),
        status: TaskStatus::Pending,
        priority: priority.clone(),
        total,
        done: 0,
        failed: 0,
        failed_resources: Vec::new(),
        started_at: None,
        finished_at: None,
        created_by: req.created_by.clone(),
        estimated_duration_secs,
        created_at: Utc::now(),
    };

    let task_id = task.task_id;
    state.preheat_tasks.write().await.insert(task_id, task);
    state
        .preheat_queue
        .write()
        .await
        .push(task_id, Reverse(priority));

    audit::log_audit(
        &state,
        &req.created_by,
        "create_preheat",
        &task_id.to_string(),
        &format!("{} resources, {} nodes", req.resource_ids.len(), req.node_ids.len()),
        "success",
    )
    .await;

    HttpResponse::Created().json(ApiResponse::ok(task_id))
}

pub async fn list_preheat_tasks(
    state: web::Data<Arc<AppState>>,
    query: web::Query<PreheatTaskQuery>,
) -> impl Responder {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20).min(100);

    let tasks = state.preheat_tasks.read().await;

    let mut filtered: Vec<&PreheatTask> = tasks.values().collect();

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

    filtered.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let total = filtered.len() as u64;
    let start = ((page - 1) * page_size) as usize;

    let items: Vec<PreheatTask> = filtered
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

pub async fn get_preheat_task(
    state: web::Data<Arc<AppState>>,
    task_id: web::Path<Uuid>,
) -> impl Responder {
    let tasks = state.preheat_tasks.read().await;
    let task = match tasks.get(&task_id) {
        Some(t) => t,
        None => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error("Task not found"));
        }
    };

    HttpResponse::Ok().json(ApiResponse::ok(task.clone()))
}

pub async fn cancel_preheat_task(
    state: web::Data<Arc<AppState>>,
    task_id: web::Path<Uuid>,
) -> impl Responder {
    let mut tasks = state.preheat_tasks.write().await;
    let task = match tasks.get_mut(&task_id) {
        Some(t) => t,
        None => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error("Task not found"));
        }
    };

    if task.status == TaskStatus::Done
        || task.status == TaskStatus::Failed
        || task.status == TaskStatus::Cancelled
    {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Task already finished"));
    }

    task.status = TaskStatus::Cancelled;
    task.finished_at = Some(Utc::now());

    drop(tasks);

    audit::log_audit(
        &state,
        "system",
        "cancel_preheat",
        &task_id.to_string(),
        "",
        "success",
    )
    .await;

    HttpResponse::Ok().json(ApiResponse::ok(true))
}

pub async fn retry_preheat_task(
    state: web::Data<Arc<AppState>>,
    task_id: web::Path<Uuid>,
) -> impl Responder {
    let mut tasks = state.preheat_tasks.write().await;
    let task = match tasks.get(&task_id) {
        Some(t) => t.clone(),
        None => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error("Task not found"));
        }
    };

    if task.failed_resources.is_empty() {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("No failed resources to retry"));
    }

    let new_task = PreheatTask {
        task_id: Uuid::new_v4(),
        resource_ids: task.failed_resources.clone(),
        node_ids: task.node_ids.clone(),
        status: TaskStatus::Pending,
        priority: task.priority.clone(),
        total: (task.failed_resources.len() * task.node_ids.len()) as u64,
        done: 0,
        failed: 0,
        failed_resources: Vec::new(),
        started_at: None,
        finished_at: None,
        created_by: task.created_by.clone(),
        estimated_duration_secs: task.estimated_duration_secs,
        created_at: Utc::now(),
    };

    let new_task_id = new_task.task_id;
    tasks.insert(new_task_id, new_task);
    state
        .preheat_queue
        .write()
        .await
        .push(new_task_id, Reverse(TaskPriority::High));

    drop(tasks);

    audit::log_audit(
        &state,
        &task.created_by,
        "retry_preheat",
        &new_task_id.to_string(),
        &format!("retry of {}", task_id),
        "success",
    )
    .await;

    HttpResponse::Ok().json(ApiResponse::ok(new_task_id))
}

pub async fn preheat_worker(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        let task_id_opt = {
            let mut queue = state.preheat_queue.write().await;
            queue.pop().map(|(id, _)| id)
        };

        let task_id = match task_id_opt {
            Some(id) => id,
            None => continue,
        };

        let task = {
            let mut tasks = state.preheat_tasks.write().await;
            if let Some(task) = tasks.get_mut(&task_id) {
                if task.status == TaskStatus::Cancelled {
                    continue;
                }
                task.status = TaskStatus::Running;
                task.started_at = Some(Utc::now());
                task.clone()
            } else {
                continue;
            }
        };

        log::info!("Starting preheat task {}", task_id);

        let state_clone = Arc::clone(&state);
        tokio::spawn(async move {
            execute_preheat_task(state_clone, task_id).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

async fn execute_preheat_task(state: Arc<AppState>, task_id: Uuid) {
    let task = {
        let tasks = state.preheat_tasks.read().await;
        match tasks.get(&task_id) {
            Some(t) => t.clone(),
            None => return,
        }
    };

    let mut done = 0u64;
    let mut failed = 0u64;
    let mut failed_resources = Vec::new();

    for node_id in &task.node_ids {
        for resource_id in &task.resource_ids {
            let is_cancelled = {
                let tasks = state.preheat_tasks.read().await;
                tasks.get(&task_id).map(|t| t.status == TaskStatus::Cancelled).unwrap_or(true)
            };
            if is_cancelled {
                log::info!("Preheat task {} cancelled", task_id);
                return;
            }

            let success = simulate_preheat_one(&state, *resource_id, *node_id).await;

            if success {
                done += 1;
            } else {
                failed += 1;
                if !failed_resources.contains(resource_id) {
                    failed_resources.push(*resource_id);
                }
            }

            {
                let mut tasks = state.preheat_tasks.write().await;
                if let Some(t) = tasks.get_mut(&task_id) {
                    t.done = done;
                    t.failed = failed;
                    t.failed_resources = failed_resources.clone();
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    }

    let mut tasks = state.preheat_tasks.write().await;
    if let Some(t) = tasks.get_mut(&task_id) {
        t.status = if failed == 0 {
            TaskStatus::Done
        } else if done > 0 {
            TaskStatus::Partial
        } else {
            TaskStatus::Failed
        };
        t.finished_at = Some(Utc::now());
    }

    log::info!(
        "Preheat task {} finished: {}/{} done, {} failed",
        task_id,
        done,
        task.total,
        failed
    );
}

async fn simulate_preheat_one(state: &Arc<AppState>, resource_id: Uuid, node_id: Uuid) -> bool {
    let nodes = state.nodes.read().await;
    let node = match nodes.get(&node_id) {
        Some(n) => n,
        None => return false,
    };
    if node.status != NodeStatus::Online {
        return false;
    }
    drop(nodes);

    let resources = state.resources.read().await;
    let resource = match resources.get(&resource_id) {
        Some(r) => r,
        None => return false,
    };
    drop(resources);

    let mut publishes = state.resource_publishes.write().await;
    publishes.insert(
        (resource_id, node_id),
        ResourcePublish {
            resource_id,
            node_id,
            published_at: Utc::now(),
        },
    );
    drop(publishes);

    let mut stats = state.resource_stats.write().await;
    let entry = stats.entry((resource_id, node_id)).or_default();
    entry.hit_count += 1;
    entry.last_accessed_at = Some(Utc::now());
    drop(stats);

    let mut history = state.request_history.write().await;
    history.push((Utc::now(), true, Some(node_id)));

    let success_rate = 0.95;
    rand::random::<f64>() < success_rate
}
