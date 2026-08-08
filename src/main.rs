mod data;
mod metrics;
mod middleware;
mod models;
mod routes;

use std::{env, io};

use actix_files::Files;
use actix_web::{web, App, HttpServer};
use data::LocationStore;
use metrics::MetricsStore;
use middleware::PerformanceMiddleware;
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> io::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("rwanda_htmx_performance=info,actix_web=info")),
        )
        .init();

    let store = web::Data::new(LocationStore::load("locations.json")?);
    let metrics_store = MetricsStore::default();
    let metrics = web::Data::new(metrics_store.clone());
    let port = env::var("APP_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(4562);
    let address = ("127.0.0.1", port);
    let public_address = format!("http://127.0.0.1:{port}");

    tracing::info!(
        event = "server_start",
        address = %public_address,
        provinces = store.counts().provinces,
        districts = store.counts().districts,
        sectors = store.counts().sectors,
        cells = store.counts().cells,
        villages = store.counts().villages,
    );

    HttpServer::new(move || {
        App::new()
            .app_data(store.clone())
            .app_data(metrics.clone())
            .wrap(PerformanceMiddleware::new(metrics_store.clone()))
            .configure(routes::configure)
            .service(Files::new("/static", "static").prefer_utf8(true))
    })
    .bind(address)?
    .run()
    .await
}
