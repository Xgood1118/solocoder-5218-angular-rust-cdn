use crate::state::AppState;
use crate::models::*;
use crate::validation;
use crate::audit;
use actix_web::{web, HttpResponse, Responder, Error};
use actix_multipart::Multipart;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct ResourceQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub business_line: Option<String>,
    pub tag: Option<String>,
    pub search: Option<String>,
    pub mime_type: Option<String>,
    pub directory: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateResourceRequest {
    pub original_filename: String,
    pub storage_path: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub sha256: Option<String>,
    pub uploaded_by: String,
    pub business_line: String,
    pub tags: Vec<String>,
    pub version: Option<String>,
    pub version_group: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateResourceRequest {
    pub tags: Option<Vec<String>>,
    pub business_line: Option<String>,
    pub original_filename: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PublishRequest {
    pub node_ids: Vec<Uuid>,
    pub operator: String,
}

#[derive(Debug, Serialize)]
pub struct ResourceWithStats {
    #[serde(flatten)]
    pub resource: Resource,
    pub hit_count: u64,
    pub miss_count: u64,
    pub hit_rate: f64,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub published_nodes: Vec<Uuid>,
}

pub async fn list_resources(
    state: web::Data<Arc<AppState>>,
    query: web::Query<ResourceQuery>,
) -> impl Responder {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20).min(100);

    let resources = state.resources.read().await;
    let resource_stats = state.resource_stats.read().await;
    let publishes = state.resource_publishes.read().await;

    let mut filtered: Vec<&Resource> = resources.values().collect();

    if let Some(bl) = &query.business_line {
        filtered.retain(|r| r.business_line == *bl);
    }
    if let Some(tag) = &query.tag {
        filtered.retain(|r| r.tags.contains(tag));
    }
    if let Some(search) = &query.search {
        filtered.retain(|r| {
            r.original_filename.contains(search)
                || r.storage_path.contains(search)
                || r.id.to_string().contains(search)
        });
    }
    if let Some(mime) = &query.mime_type {
        filtered.retain(|r| r.mime_type.starts_with(mime));
    }
    if let Some(dir) = &query.directory {
        let dir = if dir.ends_with('/') { dir.clone() } else { format!("{}/", dir) };
        filtered.retain(|r| r.storage_path.starts_with(&dir) || r.storage_path == dir.trim_end_matches('/'));
    }

    filtered.sort_by(|a, b| b.uploaded_at.cmp(&a.uploaded_at));

    let total = filtered.len() as u64;
    let start = ((page - 1) * page_size) as usize;

    let items: Vec<ResourceWithStats> = filtered
        .into_iter()
        .skip(start)
        .take(page_size as usize)
        .map(|r| {
            let mut total_hit = 0u64;
            let mut total_miss = 0u64;
            let mut last_access = None;
            let mut published_nodes = Vec::new();

            for ((rid, nid), publish) in publishes.iter() {
                if rid == &r.id {
                    published_nodes.push(*nid);
                }
            }

            for ((rid, _nid), stats) in resource_stats.iter() {
                if rid == &r.id {
                    total_hit += stats.hit_count;
                    total_miss += stats.miss_count;
                    if let Some(la) = stats.last_accessed_at {
                        if last_access.is_none() || la > last_access.unwrap() {
                            last_access = Some(la);
                        }
                    }
                }
            }

            let total = total_hit + total_miss;
            let hit_rate = if total > 0 {
                total_hit as f64 / total as f64
            } else {
                0.0
            };

            ResourceWithStats {
                resource: r.clone(),
                hit_count: total_hit,
                miss_count: total_miss,
                hit_rate,
                last_accessed_at: last_access,
                published_nodes,
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

pub async fn get_resource(
    state: web::Data<Arc<AppState>>,
    id: web::Path<Uuid>,
) -> impl Responder {
    let resources = state.resources.read().await;
    let resource_stats = state.resource_stats.read().await;
    let publishes = state.resource_publishes.read().await;

    let resource = match resources.get(&id) {
        Some(r) => r,
        None => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error("Resource not found"));
        }
    };

    let mut total_hit = 0u64;
    let mut total_miss = 0u64;
    let mut last_access = None;
    let mut published_nodes = Vec::new();

    for ((rid, nid), _publish) in publishes.iter() {
        if rid == &resource.id {
            published_nodes.push(*nid);
        }
    }

    for ((rid, _nid), stats) in resource_stats.iter() {
        if rid == &resource.id {
            total_hit += stats.hit_count;
            total_miss += stats.miss_count;
            if let Some(la) = stats.last_accessed_at {
                if last_access.is_none() || la > last_access.unwrap() {
                    last_access = Some(la);
                }
            }
        }
    }

    let total = total_hit + total_miss;
    let hit_rate = if total > 0 {
        total_hit as f64 / total as f64
    } else {
        0.0
    };

    let result = ResourceWithStats {
        resource: resource.clone(),
        hit_count: total_hit,
        miss_count: total_miss,
        hit_rate,
        last_accessed_at: last_access,
        published_nodes,
    };

    HttpResponse::Ok().json(ApiResponse::ok(result))
}

pub async fn create_resource(
    state: web::Data<Arc<AppState>>,
    req: web::Json<CreateResourceRequest>,
) -> impl Responder {
    if !validation::is_valid_filename(&req.original_filename) {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid filename"));
    }
    if !validation::is_valid_tags(&req.tags) {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(&format!("Tags must be <= {} items", validation::MAX_TAGS)));
    }
    if !validation::is_valid_mime(&req.mime_type) {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("MIME type not allowed"));
    }
    if !validation::is_valid_file_size(req.size_bytes) {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("File too large"));
    }

    let version_group = req.version_group.unwrap_or_else(Uuid::new_v4);
    let version = req.version.clone().unwrap_or_else(|| "1.0.0".to_string());
    let storage_path = validation::sanitize_path(&req.storage_path);

    let resource = Resource {
        id: Uuid::new_v4(),
        original_filename: req.original_filename.clone(),
        storage_path,
        mime_type: req.mime_type.clone(),
        size_bytes: req.size_bytes,
        md5: None,
        sha256: req.sha256.clone().unwrap_or_default(),
        uploaded_at: Utc::now(),
        uploaded_by: req.uploaded_by.clone(),
        business_line: req.business_line.clone(),
        tags: req.tags.clone(),
        version,
        version_group,
    };

    let id = resource.id;
    state.resources.write().await.insert(id, resource);

    audit::log_audit(
        &state,
        &req.uploaded_by,
        "create_resource",
        &id.to_string(),
        &req.business_line,
        "success",
    )
    .await;

    HttpResponse::Created().json(ApiResponse::ok(id))
}

pub async fn update_resource(
    state: web::Data<Arc<AppState>>,
    id: web::Path<Uuid>,
    req: web::Json<UpdateResourceRequest>,
) -> impl Responder {
    let mut resources = state.resources.write().await;
    let resource = match resources.get_mut(&id) {
        Some(r) => r,
        None => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error("Resource not found"));
        }
    };

    if let Some(tags) = &req.tags {
        if !validation::is_valid_tags(tags) {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid tags"));
        }
        resource.tags = tags.clone();
    }
    if let Some(bl) = &req.business_line {
        resource.business_line = bl.clone();
    }
    if let Some(name) = &req.original_filename {
        if !validation::is_valid_filename(name) {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid filename"));
        }
        resource.original_filename = name.clone();
    }

    let updated = resource.clone();
    drop(resources);

    audit::log_audit(
        &state,
        "system",
        "update_resource",
        &id.to_string(),
        &updated.business_line,
        "success",
    )
    .await;

    HttpResponse::Ok().json(ApiResponse::ok(updated))
}

pub async fn delete_resource(
    state: web::Data<Arc<AppState>>,
    id: web::Path<Uuid>,
) -> impl Responder {
    let mut resources = state.resources.write().await;
    let resource = match resources.remove(&id) {
        Some(r) => r,
        None => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error("Resource not found"));
        }
    };

    let id_val = id.into_inner();

    let mut stats = state.resource_stats.write().await;
    stats.retain(|(rid, _), _| rid != &id_val);

    let mut publishes = state.resource_publishes.write().await;
    publishes.retain(|(rid, _), _| rid != &id_val);

    drop(resources);
    drop(stats);
    drop(publishes);

    audit::log_audit(
        &state,
        "system",
        "delete_resource",
        &id_val.to_string(),
        &resource.business_line,
        "success",
    )
    .await;

    HttpResponse::Ok().json(ApiResponse::ok(true))
}

pub async fn list_versions(
    state: web::Data<Arc<AppState>>,
    id: web::Path<Uuid>,
) -> impl Responder {
    let resources = state.resources.read().await;

    let resource = match resources.get(&id) {
        Some(r) => r,
        None => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error("Resource not found"));
        }
    };

    let version_group = resource.version_group;

    let mut versions: Vec<Resource> = resources
        .values()
        .filter(|r| r.version_group == version_group)
        .cloned()
        .collect();

    versions.sort_by(|a, b| b.uploaded_at.cmp(&a.uploaded_at));

    HttpResponse::Ok().json(ApiResponse::ok(versions))
}

pub async fn publish_resource(
    state: web::Data<Arc<AppState>>,
    id: web::Path<Uuid>,
    req: web::Json<PublishRequest>,
) -> impl Responder {
    let id_val = id.into_inner();
    let resources = state.resources.read().await;
    if !resources.contains_key(&id_val) {
        return HttpResponse::NotFound().json(ApiResponse::<()>::error("Resource not found"));
    }

    let nodes = state.nodes.read().await;
    for node_id in &req.node_ids {
        if !nodes.contains_key(node_id) {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(&format!(
                "Node {} not found",
                node_id
            )));
        }
    }

    let mut publishes = state.resource_publishes.write().await;
    for node_id in &req.node_ids {
        publishes.insert(
            (id_val, *node_id),
            ResourcePublish {
                resource_id: id_val,
                node_id: *node_id,
                published_at: Utc::now(),
            },
        );
    }

    drop(resources);
    drop(nodes);
    drop(publishes);

    audit::log_audit(
        &state,
        &req.operator,
        "publish_resource",
        &id_val.to_string(),
        &format!("nodes: {:?}", req.node_ids),
        "success",
    )
    .await;

    HttpResponse::Ok().json(ApiResponse::ok(true))
}

pub async fn unpublish_resource(
    state: web::Data<Arc<AppState>>,
    id: web::Path<Uuid>,
    req: web::Json<PublishRequest>,
) -> impl Responder {
    let id_val = id.into_inner();
    let mut publishes = state.resource_publishes.write().await;
    for node_id in &req.node_ids {
        publishes.remove(&(id_val, *node_id));
    }

    drop(publishes);

    audit::log_audit(
        &state,
        &req.operator,
        "unpublish_resource",
        &id_val.to_string(),
        &format!("nodes: {:?}", req.node_ids),
        "success",
    )
    .await;

    HttpResponse::Ok().json(ApiResponse::ok(true))
}

pub async fn get_directory_tree(
    state: web::Data<Arc<AppState>>,
) -> impl Responder {
    let resources = state.resources.read().await;

    let mut resource_list: Vec<&Resource> = resources.values().collect();
    resource_list.sort_by(|a, b| a.storage_path.cmp(&b.storage_path));

    let mut root = DirectoryNode {
        name: "root".to_string(),
        path: "/".to_string(),
        is_dir: true,
        children: Vec::new(),
        resource_id: None,
    };

    for resource in &resource_list {
        let path = &resource.storage_path;
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

        let mut current = &mut root;
        let mut current_path = String::from("/");

        for (i, part) in parts.iter().enumerate() {
            let is_last = i == parts.len() - 1;
            let next_path = if current_path == "/" {
                format!("/{}", part)
            } else {
                format!("{}/{}", current_path, part)
            };

            if is_last {
                if !current.children.iter().any(|c| !c.is_dir && c.name == *part) {
                    current.children.push(DirectoryNode {
                        name: part.to_string(),
                        path: next_path,
                        is_dir: false,
                        children: Vec::new(),
                        resource_id: Some(resource.id),
                    });
                }
            } else {
                let idx = current
                    .children
                    .iter()
                    .position(|c| c.is_dir && c.name == *part);
                match idx {
                    Some(i) => {
                        current = &mut current.children[i];
                    }
                    None => {
                        current.children.push(DirectoryNode {
                            name: part.to_string(),
                            path: next_path.clone(),
                            is_dir: true,
                            children: Vec::new(),
                            resource_id: None,
                        });
                        let last_idx = current.children.len() - 1;
                        current = &mut current.children[last_idx];
                    }
                }
                current_path = next_path;
            }
        }
    }

    fn sort_tree(node: &mut DirectoryNode) {
        node.children.sort_by(|a, b| {
            if a.is_dir && !b.is_dir {
                std::cmp::Ordering::Less
            } else if !a.is_dir && b.is_dir {
                std::cmp::Ordering::Greater
            } else {
                a.name.cmp(&b.name)
            }
        });
        for child in &mut node.children {
            if child.is_dir {
                sort_tree(child);
            }
        }
    }

    sort_tree(&mut root);

    HttpResponse::Ok().json(ApiResponse::ok(root))
}

pub async fn upload_resource(
    state: web::Data<Arc<AppState>>,
    mut payload: Multipart,
) -> Result<HttpResponse, Error> {
    let mut files = Vec::new();

    while let Some(item) = payload.next().await {
        let mut field = item?;
        let content_disposition = field.content_disposition();

        let filename = content_disposition
            .get_filename()
            .unwrap_or("unnamed")
            .to_string();

        if !validation::is_valid_filename(&filename) {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid filename")));
        }

        let mime_type = mime_guess::from_path(&filename)
            .first_or_octet_stream()
            .to_string();

        if !validation::is_valid_mime(&mime_type) {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(&format!(
                "MIME type not allowed: {}",
                mime_type
            ))));
        }

        let mut body = Vec::new();
        let mut hasher = Sha256::new();

        while let Some(chunk) = field.next().await {
            let data = chunk?;
            hasher.update(&data);
            body.extend_from_slice(&data);

            if body.len() as u64 > validation::MAX_FILE_SIZE {
                return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                    "File exceeds maximum size of 100MB",
                )));
            }
        }

        let size_bytes = body.len() as u64;
        let sha256_hash = format!("{:x}", hasher.finalize());

        files.push((filename, mime_type, size_bytes, sha256_hash, body));
    }

    let mut uploaded_ids = Vec::new();
    let mut resources_write = state.resources.write().await;

    for (filename, mime_type, size_bytes, sha256_hash, _body) in files {
        let id = Uuid::new_v4();
        let storage_path = format!("/{}", filename);

        let resource = Resource {
            id,
            original_filename: filename.clone(),
            storage_path,
            mime_type,
            size_bytes,
            md5: None,
            sha256: sha256_hash,
            uploaded_at: Utc::now(),
            uploaded_by: "uploader".to_string(),
            business_line: "default".to_string(),
            tags: Vec::new(),
            version: "1.0.0".to_string(),
            version_group: Uuid::new_v4(),
        };

        uploaded_ids.push(id);
        resources_write.insert(id, resource);
    }

    drop(resources_write);

    for id in &uploaded_ids {
        audit::log_audit(
            &state,
            "uploader",
            "upload_resource",
            &id.to_string(),
            "default",
            "success",
        )
        .await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::ok(uploaded_ids)))
}
