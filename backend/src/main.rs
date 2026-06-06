mod models;
mod state;
mod validation;
mod audit;
mod handlers {
    pub mod resource;
    pub mod node;
    pub mod preheat;
    pub mod purge;
    pub mod stats;
}
mod middleware {
    pub mod auth;
}

use actix_web::{web, App, HttpServer, middleware::Logger};
use actix_cors::Cors;
use state::AppState;
use std::sync::Arc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let state = Arc::new(AppState::new());

    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        audit::cleanup_old_logs_worker(state_clone).await;
    });

    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        handlers::purge::delayed_cleanup_worker(state_clone).await;
    });

    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        handlers::preheat::preheat_worker(state_clone).await;
    });

    log::info!("Starting CDN Manager server on http://localhost:8080");

    HttpServer::new(move || {
        let cors = Cors::permissive();

        App::new()
            .app_data(web::Data::new(Arc::clone(&state)))
            .wrap(cors)
            .wrap(Logger::default())
            .service(
                web::scope("/api")
                    .service(
                        web::scope("/resources")
                            .route("", web::get().to(handlers::resource::list_resources))
                            .route("", web::post().to(handlers::resource::create_resource))
                            .route("/{id}", web::get().to(handlers::resource::get_resource))
                            .route("/{id}", web::put().to(handlers::resource::update_resource))
                            .route("/{id}", web::delete().to(handlers::resource::delete_resource))
                            .route("/{id}/versions", web::get().to(handlers::resource::list_versions))
                            .route("/{id}/publish", web::post().to(handlers::resource::publish_resource))
                            .route("/{id}/unpublish", web::post().to(handlers::resource::unpublish_resource))
                            .route("/tree", web::get().to(handlers::resource::get_directory_tree))
                            .route("/upload", web::post().to(handlers::resource::upload_resource))
                    )
                    .service(
                        web::scope("/nodes")
                            .route("", web::get().to(handlers::node::list_nodes))
                            .route("", web::post().to(handlers::node::create_node))
                            .route("/{id}", web::get().to(handlers::node::get_node))
                            .route("/{id}", web::put().to(handlers::node::update_node))
                            .route("/{id}", web::delete().to(handlers::node::delete_node))
                            .route("/{id}/status", web::put().to(handlers::node::update_node_status))
                            .route("/{id}/resources", web::get().to(handlers::node::list_node_resources))
                    )
                    .service(
                        web::scope("/preheat")
                            .route("", web::post().to(handlers::preheat::create_preheat_task))
                            .route("", web::get().to(handlers::preheat::list_preheat_tasks))
                            .route("/{task_id}", web::get().to(handlers::preheat::get_preheat_task))
                            .route("/{task_id}/cancel", web::post().to(handlers::preheat::cancel_preheat_task))
                            .route("/{task_id}/retry", web::post().to(handlers::preheat::retry_preheat_task))
                    )
                    .service(
                        web::scope("/purge")
                            .route("", web::post().to(handlers::purge::create_purge_task))
                            .route("", web::get().to(handlers::purge::list_purge_tasks))
                            .route("/{task_id}", web::get().to(handlers::purge::get_purge_task))
                            .route("/dry-run", web::post().to(handlers::purge::dry_run_purge))
                    )
                    .service(
                        web::scope("/stats")
                            .route("/overview", web::get().to(handlers::stats::get_overview))
                            .route("/hit-rate", web::get().to(handlers::stats::get_hit_rate))
                            .route("/trend", web::get().to(handlers::stats::get_trend))
                            .route("/export", web::get().to(handlers::stats::export_csv))
                    )
                    .service(
                        web::scope("/audit")
                            .route("/logs", web::get().to(audit::list_audit_logs))
                    )
            )
    })
    .bind(("0.0.0.0", 8118))?
    .run()
    .await
}
