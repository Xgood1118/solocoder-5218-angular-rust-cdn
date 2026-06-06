use crate::state::AppState;
use crate::models::*;
use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct HitRateQuery {
    pub group_by: String,
    pub region: Option<String>,
    pub time_period: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TrendQuery {
    pub days: Option<u32>,
    pub region: Option<String>,
    pub node_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    pub days: Option<u32>,
}

pub async fn get_overview(
    state: web::Data<Arc<AppState>>,
) -> impl Responder {
    let resources = state.resources.read().await;
    let nodes = state.nodes.read().await;
    let stats = state.resource_stats.read().await;
    let history = state.request_history.read().await;
    let preheat_tasks = state.preheat_tasks.read().await;
    let purge_tasks = state.purge_tasks.read().await;
    let publishes = state.resource_publishes.read().await;

    let total_resources = resources.len() as u64;
    let total_nodes = nodes.len() as u64;

    let mut total_hits = 0u64;
    let mut total_misses = 0u64;
    let mut resource_hits: HashMap<Uuid, u64> = HashMap::new();

    for ((rid, _nid), s) in stats.iter() {
        total_hits += s.hit_count;
        total_misses += s.miss_count;
        *resource_hits.entry(*rid).or_insert(0) += s.hit_count;
    }

    let total_requests = total_hits + total_misses;
    let overall_hit_rate = if total_requests > 0 {
        total_hits as f64 / total_requests as f64
    } else {
        0.0
    };

    let today = Utc::now().date_naive();
    let today_start = today.and_hms_opt(0, 0, 0).unwrap().and_utc();

    let mut total_requests_today = 0u64;
    let mut total_hits_today = 0u64;
    for (ts, is_hit, _) in history.iter() {
        if ts >= &today_start {
            total_requests_today += 1;
            if *is_hit {
                total_hits_today += 1;
            }
        }
    }

    let mut sorted_resources: Vec<(Uuid, u64)> = resource_hits.into_iter().collect();
    sorted_resources.sort_by(|a, b| b.1.cmp(&a.1));

    let top_hot: Vec<(String, u64)> = sorted_resources
        .iter()
        .take(10)
        .filter_map(|(rid, count)| {
            resources.get(rid).map(|r| (r.original_filename.clone(), *count))
        })
        .collect();

    let top_cold: Vec<(String, u64)> = sorted_resources
        .iter()
        .rev()
        .take(10)
        .filter_map(|(rid, count)| {
            resources.get(rid).map(|r| (r.original_filename.clone(), *count))
        })
        .collect();

    let node_capacity: Vec<(String, f64)> = nodes
        .values()
        .map(|n| {
            let used_bytes: u64 = publishes
                .iter()
                .filter(|((_, nid), _)| *nid == n.id)
                .filter_map(|((rid, _), _)| resources.get(rid).map(|r| r.size_bytes))
                .sum();
            let used_gb = used_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            let usage = if n.capacity_gb > 0 {
                (used_gb / n.capacity_gb as f64) * 100.0
            } else {
                0.0
            };
            (n.name.clone(), usage)
        })
        .collect();

    let active_preheat = preheat_tasks
        .values()
        .filter(|t| t.status == TaskStatus::Running || t.status == TaskStatus::Pending)
        .count() as u64;

    let active_purge = purge_tasks
        .values()
        .filter(|t| t.status == TaskStatus::Running || t.status == TaskStatus::Pending)
        .count() as u64;

    let overview = OverviewStats {
        total_resources,
        total_nodes,
        overall_hit_rate,
        total_requests_today,
        total_hits_today,
        top_hot_resources: top_hot,
        top_cold_resources: top_cold,
        node_capacity_usage: node_capacity,
        active_preheat_tasks: active_preheat,
        active_purge_tasks: active_purge,
    };

    HttpResponse::Ok().json(ApiResponse::ok(overview))
}

pub async fn get_hit_rate(
    state: web::Data<Arc<AppState>>,
    query: web::Query<HitRateQuery>,
) -> impl Responder {
    let stats = state.resource_stats.read().await;
    let resources = state.resources.read().await;
    let nodes = state.nodes.read().await;

    let region_filter = query.region.as_deref().and_then(Region::from_str);

    let filtered_stats: Vec<((Uuid, Uuid), &ResourceStats)> = stats
        .iter()
        .filter(|((_rid, nid), _)| {
            if let Some(region) = &region_filter {
                if let Some(node) = nodes.get(nid) {
                    node.region == *region
                } else {
                    false
                }
            } else {
                true
            }
        })
        .map(|(k, v)| (k.clone(), v))
        .collect();

    let result: Vec<HitRateEntry> = match query.group_by.as_str() {
        "node" => {
            let mut node_stats: HashMap<Uuid, (u64, u64)> = HashMap::new();
            for ((_rid, nid), s) in &filtered_stats {
                let entry = node_stats.entry(*nid).or_insert((0, 0));
                entry.0 += s.hit_count;
                entry.1 += s.miss_count;
            }
            node_stats
                .into_iter()
                .map(|(nid, (hits, misses))| {
                    let total = hits + misses;
                    let rate = if total > 0 {
                        hits as f64 / total as f64
                    } else {
                        0.0
                    };
                    let name = nodes
                        .get(&nid)
                        .map(|n| n.name.clone())
                        .unwrap_or_else(|| nid.to_string());
                    HitRateEntry {
                        key: name,
                        hit_count: hits,
                        miss_count: misses,
                        hit_rate: rate,
                    }
                })
                .collect()
        }
        "resource" => {
            let mut res_stats: HashMap<Uuid, (u64, u64)> = HashMap::new();
            for ((rid, _nid), s) in &filtered_stats {
                let entry = res_stats.entry(*rid).or_insert((0, 0));
                entry.0 += s.hit_count;
                entry.1 += s.miss_count;
            }
            res_stats
                .into_iter()
                .map(|(rid, (hits, misses))| {
                    let total = hits + misses;
                    let rate = if total > 0 {
                        hits as f64 / total as f64
                    } else {
                        0.0
                    };
                    let name = resources
                        .get(&rid)
                        .map(|r| r.original_filename.clone())
                        .unwrap_or_else(|| rid.to_string());
                    HitRateEntry {
                        key: name,
                        hit_count: hits,
                        miss_count: misses,
                        hit_rate: rate,
                    }
                })
                .collect()
        }
        "time_period" => {
            let periods = vec![
                ("近1小时", Duration::hours(1)),
                ("近24小时", Duration::hours(24)),
                ("近7天", Duration::days(7)),
            ];
            let history = state.request_history.read().await;
            let now = Utc::now();

            periods
                .into_iter()
                .map(|(name, duration)| {
                    let cutoff = now - duration;
                    let mut hits = 0u64;
                    let mut misses = 0u64;
                    for (ts, is_hit, node_id) in history.iter() {
                        if ts < &cutoff {
                            continue;
                        }
                        if let Some(region) = &region_filter {
                            if let Some(nid) = node_id {
                                if let Some(node) = nodes.get(nid) {
                                    if node.region != *region {
                                        continue;
                                    }
                                }
                            }
                        }
                        if *is_hit {
                            hits += 1;
                        } else {
                            misses += 1;
                        }
                    }
                    let total = hits + misses;
                    let rate = if total > 0 {
                        hits as f64 / total as f64
                    } else {
                        0.0
                    };
                    HitRateEntry {
                        key: name.to_string(),
                        hit_count: hits,
                        miss_count: misses,
                        hit_rate: rate,
                    }
                })
                .collect()
        }
        _ => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid group_by"));
        }
    };

    HttpResponse::Ok().json(ApiResponse::ok(result))
}

pub async fn get_trend(
    state: web::Data<Arc<AppState>>,
    query: web::Query<TrendQuery>,
) -> impl Responder {
    let days = query.days.unwrap_or(7);
    let history = state.request_history.read().await;
    let nodes = state.nodes.read().await;

    let region_filter = query.region.as_deref().and_then(Region::from_str);
    let node_filter = query.node_id;

    let now = Utc::now();
    let start = now - Duration::days(days as i64);

    let mut points: Vec<TrendPoint> = Vec::new();

    if days <= 2 {
        let hours = days * 24;
        for h in 0..hours {
            let hour_start = start + Duration::hours(h as i64);
            let hour_end = hour_start + Duration::hours(1);
            let mut reqs = 0u64;
            let mut hits = 0u64;

            for (ts, is_hit, node_id) in history.iter() {
                if ts < &hour_start || ts >= &hour_end {
                    continue;
                }
                if let Some(nid) = node_filter {
                    if Some(nid) != *node_id {
                        continue;
                    }
                }
                if let Some(region) = &region_filter {
                    if let Some(nid) = node_id {
                        if let Some(node) = nodes.get(nid) {
                            if node.region != *region {
                                continue;
                            }
                        }
                    }
                }
                reqs += 1;
                if *is_hit {
                    hits += 1;
                }
            }

            let rate = if reqs > 0 { hits as f64 / reqs as f64 } else { 0.0 };
            points.push(TrendPoint {
                timestamp: hour_start,
                requests: reqs,
                hits,
                hit_rate: rate,
            });
        }
    } else {
        for d in 0..days {
            let day_start = start + Duration::days(d as i64);
            let day_end = day_start + Duration::days(1);
            let mut reqs = 0u64;
            let mut hits = 0u64;

            for (ts, is_hit, node_id) in history.iter() {
                if ts < &day_start || ts >= &day_end {
                    continue;
                }
                if let Some(nid) = node_filter {
                    if Some(nid) != *node_id {
                        continue;
                    }
                }
                if let Some(region) = &region_filter {
                    if let Some(nid) = node_id {
                        if let Some(node) = nodes.get(nid) {
                            if node.region != *region {
                                continue;
                            }
                        }
                    }
                }
                reqs += 1;
                if *is_hit {
                    hits += 1;
                }
            }

            let rate = if reqs > 0 { hits as f64 / reqs as f64 } else { 0.0 };
            points.push(TrendPoint {
                timestamp: day_start,
                requests: reqs,
                hits,
                hit_rate: rate,
            });
        }
    }

    HttpResponse::Ok().json(ApiResponse::ok(points))
}

pub async fn export_csv(
    state: web::Data<Arc<AppState>>,
    query: web::Query<ExportQuery>,
) -> impl Responder {
    let days = query.days.unwrap_or(7);
    let history = state.request_history.read().await;
    let stats = state.resource_stats.read().await;
    let resources = state.resources.read().await;
    let nodes = state.nodes.read().await;

    let mut wtr = csv::Writer::from_writer(Vec::new());

    wtr.write_record(&["Date", "Requests", "Hits", "Misses", "Hit Rate (%)"]).unwrap();

    let now = Utc::now();
    let start = now - Duration::days(days as i64);

    for d in 0..days {
        let day_start = start + Duration::days(d as i64);
        let day_end = day_start + Duration::days(1);
        let mut reqs = 0u64;
        let mut hits = 0u64;

        for (ts, is_hit, _) in history.iter() {
            if ts >= &day_start && ts < &day_end {
                reqs += 1;
                if *is_hit {
                    hits += 1;
                }
            }
        }

        let misses = reqs - hits;
        let rate = if reqs > 0 {
            format!("{:.2}", (hits as f64 / reqs as f64) * 100.0)
        } else {
            "0.00".to_string()
        };

        wtr.write_record(&[
            day_start.format("%Y-%m-%d").to_string(),
            reqs.to_string(),
            hits.to_string(),
            misses.to_string(),
            rate,
        ])
        .unwrap();
    }

    wtr.write_record(&[""]).unwrap();
    wtr.write_record(&["Per-Node Hit Rate"]).unwrap();
    wtr.write_record(&["Node", "Region", "Requests", "Hits", "Misses", "Hit Rate (%)"]).unwrap();

    let mut node_stats: HashMap<Uuid, (u64, u64)> = HashMap::new();
    for ((_rid, nid), s) in stats.iter() {
        let entry = node_stats.entry(*nid).or_insert((0, 0));
        entry.0 += s.hit_count;
        entry.1 += s.miss_count;
    }

    for (nid, (hits, misses)) in &node_stats {
        let node = nodes.get(nid);
        let name = node.map(|n| n.name.clone()).unwrap_or_else(|| nid.to_string());
        let region = node.map(|n| n.region.to_string().to_owned()).unwrap_or_default();
        let total = hits + misses;
        let rate = if total > 0 {
            format!("{:.2}", (*hits as f64 / total as f64) * 100.0)
        } else {
            "0.00".to_string()
        };

        wtr.write_record(&[
            name,
            region,
            total.to_string(),
            hits.to_string(),
            misses.to_string(),
            rate,
        ])
        .unwrap();
    }

    wtr.write_record(&[""]).unwrap();
    wtr.write_record(&["Top 20 Resources by Requests"]).unwrap();
    wtr.write_record(&["Resource", "Business Line", "MIME Type", "Requests", "Hits", "Hit Rate (%)"]).unwrap();

    let mut res_stats: HashMap<Uuid, (u64, u64)> = HashMap::new();
    for ((rid, _nid), s) in stats.iter() {
        let entry = res_stats.entry(*rid).or_insert((0, 0));
        entry.0 += s.hit_count;
        entry.1 += s.miss_count;
    }

    let mut sorted: Vec<(Uuid, u64, u64)> = res_stats
        .into_iter()
        .map(|(rid, (h, m))| (rid, h, m))
        .collect();
    sorted.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)));

    for (rid, hits, misses) in sorted.iter().take(20) {
        let resource = resources.get(rid);
        let name = resource.map(|r| r.original_filename.clone()).unwrap_or_else(|| rid.to_string());
        let bl = resource.map(|r| r.business_line.clone()).unwrap_or_default();
        let mime = resource.map(|r| r.mime_type.clone()).unwrap_or_default();
        let total = hits + misses;
        let rate = if total > 0 {
            format!("{:.2}", (*hits as f64 / total as f64) * 100.0)
        } else {
            "0.00".to_string()
        };

        wtr.write_record(&[
            name,
            bl,
            mime,
            total.to_string(),
            hits.to_string(),
            rate,
        ])
        .unwrap();
    }

    let csv_data = String::from_utf8(wtr.into_inner().unwrap()).unwrap();

    HttpResponse::Ok()
        .content_type("text/csv; charset=utf-8")
        .append_header((
            "Content-Disposition",
            "attachment; filename=\"cdn_stats_report.csv\"",
        ))
        .body(csv_data)
}
