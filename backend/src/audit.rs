use crate::state::AppState;
use crate::models::AuditLog;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;

pub async fn log_audit(
    state: &Arc<AppState>,
    actor: &str,
    action: &str,
    target: &str,
    scope: &str,
    result: &str,
) {
    let log = AuditLog {
        id: Uuid::new_v4(),
        actor: actor.to_string(),
        action: action.to_string(),
        target: target.to_string(),
        scope: scope.to_string(),
        timestamp: Utc::now(),
        result: result.to_string(),
    };
    state.audit_logs.write().await.push(log);
}

pub async fn cleanup_old_logs_worker(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
        let cutoff = Utc::now() - Duration::days(30);
        let mut logs = state.audit_logs.write().await;
        logs.retain(|log| log.timestamp > cutoff);
        log::info!("Cleaned up old audit logs, remaining: {}", logs.len());
    }
}

#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub action: Option<String>,
    pub actor: Option<String>,
}

pub async fn list_audit_logs(
    state: web::Data<Arc<AppState>>,
    query: web::Query<AuditLogQuery>,
) -> impl Responder {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(50).min(200);
    let logs = state.audit_logs.read().await;

    let mut filtered: Vec<&AuditLog> = logs.iter().collect();

    if let Some(action) = &query.action {
        filtered.retain(|l| l.action.contains(action));
    }
    if let Some(actor) = &query.actor {
        filtered.retain(|l| l.actor.contains(actor));
    }

    filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let total = filtered.len() as u64;
    let start = ((page - 1) * page_size) as usize;
    let items: Vec<AuditLog> = filtered
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

    HttpResponse::Ok().json(crate::models::ApiResponse::ok(result))
}
