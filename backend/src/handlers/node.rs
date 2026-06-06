use crate::state::AppState;
use crate::models::*;
use crate::audit;
use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Deserialize)]
pub struct NodeQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub region: Option<String>,
    pub status: Option<String>,
    pub carrier: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateNodeRequest {
    pub name: String,
    pub region: String,
    pub datacenter_address: String,
    pub carrier: String,
    pub capacity_gb: u64,
    pub operator: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNodeRequest {
    pub name: Option<String>,
    pub datacenter_address: Option<String>,
    pub capacity_gb: Option<u64>,
    pub region: Option<String>,
    pub carrier: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNodeStatusRequest {
    pub status: String,
    pub operator: String,
}

#[derive(Debug, Serialize)]
pub struct NodeWithStats {
    #[serde(flatten)]
    pub node: Node,
    pub resource_count: u64,
}

pub async fn list_nodes(
    state: web::Data<Arc<AppState>>,
    query: web::Query<NodeQuery>,
) -> impl Responder {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20).min(100);

    let nodes = state.nodes.read().await;
    let publishes = state.resource_publishes.read().await;

    let mut filtered: Vec<&Node> = nodes.values().collect();

    if let Some(region) = &query.region {
        if let Some(r) = Region::from_str(region) {
            filtered.retain(|n| n.region == r);
        }
    }
    if let Some(status) = &query.status {
        let status_enum = match status.as_str() {
            "online" => NodeStatus::Online,
            "offline" => NodeStatus::Offline,
            "maintenance" => NodeStatus::Maintenance,
            _ => {
                return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid status"));
            }
        };
        filtered.retain(|n| n.status == status_enum);
    }

    filtered.sort_by(|a, b| b.heartbeat_at.cmp(&a.heartbeat_at));

    let total = filtered.len() as u64;
    let start = ((page - 1) * page_size) as usize;

    let items: Vec<NodeWithStats> = filtered
        .into_iter()
        .skip(start)
        .take(page_size as usize)
        .map(|n| {
            let resource_count = publishes
                .iter()
                .filter(|((_, nid), _)| *nid == n.id)
                .count() as u64;

            NodeWithStats {
                node: n.clone(),
                resource_count,
            }
        })
        .collect();

    let result = serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "page_size": page_size,
    });

    HttpResponse::Ok().json(ApiResponse::ok(result))
}

pub async fn get_node(
    state: web::Data<Arc<AppState>>,
    id: web::Path<Uuid>,
) -> impl Responder {
    let nodes = state.nodes.read().await;
    let node = match nodes.get(&id) {
        Some(n) => n,
        None => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error("Node not found"));
        }
    };

    let publishes = state.resource_publishes.read().await;
    let resource_count = publishes
        .iter()
        .filter(|((_, nid), _)| *nid == node.id)
        .count() as u64;

    let result = NodeWithStats {
        node: node.clone(),
        resource_count,
    };

    HttpResponse::Ok().json(ApiResponse::ok(result))
}

pub async fn create_node(
    state: web::Data<Arc<AppState>>,
    req: web::Json<CreateNodeRequest>,
) -> impl Responder {
    let region = match Region::from_str(&req.region) {
        Some(r) => r,
        None => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid region"));
        }
    };

    let carrier = match req.carrier.as_str() {
        "电信" => Carrier::Telecom,
        "联通" => Carrier::Unicom,
        "移动" => Carrier::Mobile,
        "多线" => Carrier::MultiLine,
        _ => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid carrier"));
        }
    };

    let node = Node {
        id: Uuid::new_v4(),
        name: req.name.clone(),
        region,
        datacenter_address: req.datacenter_address.clone(),
        carrier,
        capacity_gb: req.capacity_gb,
        used_gb: 0,
        status: NodeStatus::Offline,
        heartbeat_at: Utc::now(),
    };

    let id = node.id;
    state.nodes.write().await.insert(id, node);

    audit::log_audit(
        &state,
        &req.operator,
        "create_node",
        &id.to_string(),
        &req.name,
        "success",
    )
    .await;

    HttpResponse::Created().json(ApiResponse::ok(id))
}

pub async fn update_node(
    state: web::Data<Arc<AppState>>,
    id: web::Path<Uuid>,
    req: web::Json<UpdateNodeRequest>,
) -> impl Responder {
    let mut nodes = state.nodes.write().await;
    let node = match nodes.get_mut(&id) {
        Some(n) => n,
        None => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error("Node not found"));
        }
    };

    if let Some(name) = &req.name {
        node.name = name.clone();
    }
    if let Some(addr) = &req.datacenter_address {
        node.datacenter_address = addr.clone();
    }
    if let Some(cap) = req.capacity_gb {
        node.capacity_gb = cap;
    }
    if let Some(region_str) = &req.region {
        if let Some(r) = Region::from_str(region_str) {
            node.region = r;
        }
    }
    if let Some(carrier_str) = &req.carrier {
        let carrier = match carrier_str.as_str() {
            "电信" => Carrier::Telecom,
            "联通" => Carrier::Unicom,
            "移动" => Carrier::Mobile,
            "多线" => Carrier::MultiLine,
            _ => {
                return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid carrier"));
            }
        };
        node.carrier = carrier;
    }

    let updated = node.clone();
    drop(nodes);

    audit::log_audit(
        &state,
        "system",
        "update_node",
        &id.to_string(),
        &updated.name,
        "success",
    )
    .await;

    HttpResponse::Ok().json(ApiResponse::ok(updated))
}

pub async fn delete_node(
    state: web::Data<Arc<AppState>>,
    id: web::Path<Uuid>,
) -> impl Responder {
    let id_val = id.into_inner();
    let mut nodes = state.nodes.write().await;
    let node = match nodes.remove(&id_val) {
        Some(n) => n,
        None => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error("Node not found"));
        }
    };

    let mut publishes = state.resource_publishes.write().await;
    publishes.retain(|(_, nid), _| *nid != id_val);

    let mut stats = state.resource_stats.write().await;
    stats.retain(|(_, nid), _| *nid != id_val);

    drop(nodes);
    drop(publishes);
    drop(stats);

    audit::log_audit(
        &state,
        "system",
        "delete_node",
        &id_val.to_string(),
        &node.name,
        "success",
    )
    .await;

    HttpResponse::Ok().json(ApiResponse::ok(true))
}

pub async fn update_node_status(
    state: web::Data<Arc<AppState>>,
    id: web::Path<Uuid>,
    req: web::Json<UpdateNodeStatusRequest>,
) -> impl Responder {
    let status = match req.status.as_str() {
        "online" => NodeStatus::Online,
        "offline" => NodeStatus::Offline,
        "maintenance" => NodeStatus::Maintenance,
        _ => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid status"));
        }
    };

    let mut nodes = state.nodes.write().await;
    let node = match nodes.get_mut(&id) {
        Some(n) => n,
        None => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error("Node not found"));
        }
    };

    node.status = status;
    node.heartbeat_at = Utc::now();

    drop(nodes);

    audit::log_audit(
        &state,
        &req.operator,
        "update_node_status",
        &id.to_string(),
        &req.status,
        "success",
    )
    .await;

    HttpResponse::Ok().json(ApiResponse::ok(true))
}

pub async fn list_node_resources(
    state: web::Data<Arc<AppState>>,
    id: web::Path<Uuid>,
) -> impl Responder {
    let id_val = id.into_inner();
    let nodes = state.nodes.read().await;
    if !nodes.contains_key(&id_val) {
        return HttpResponse::NotFound().json(ApiResponse::<()>::error("Node not found"));
    }

    let publishes = state.resource_publishes.read().await;
    let resources = state.resources.read().await;
    let stats = state.resource_stats.read().await;

    let node_resource_ids: Vec<Uuid> = publishes
        .iter()
        .filter(|((_, nid), _)| *nid == id_val)
        .map(|((rid, _), _)| *rid)
        .collect();

    let mut result = Vec::new();
    for rid in &node_resource_ids {
        if let Some(r) = resources.get(rid) {
            let node_stats = stats.get(&(*rid, id_val)).cloned().unwrap_or_default();
            let total = node_stats.hit_count + node_stats.miss_count;
            let hit_rate = if total > 0 {
                node_stats.hit_count as f64 / total as f64
            } else {
                0.0
            };

            result.push(serde_json::json!({
                "resource": r,
                "hit_count": node_stats.hit_count,
                "miss_count": node_stats.miss_count,
                "hit_rate": hit_rate,
                "last_accessed_at": node_stats.last_accessed_at,
            }));
        }
    }

    result.sort_by(|a, b| {
        b["hit_count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["hit_count"].as_u64().unwrap_or(0))
    });

    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "items": result,
        "total": result.len(),
    })))
}
