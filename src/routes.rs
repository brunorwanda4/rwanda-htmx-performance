use std::time::Duration;

use actix_web::{error::ErrorInternalServerError, http::header, web, HttpResponse, Result};
use askama::Template;
use serde::Serialize;

use crate::{
    data::LocationStore,
    metrics::{MetricsSnapshot, MetricsStore, PerformanceRecord},
    models::{LocationCounts, LocationDetails, LocationQuery, SearchPage},
};

#[derive(Clone)]
struct SelectOption {
    value: String,
    selected: bool,
}

#[derive(Clone)]
struct ResultRow {
    id: String,
    breadcrumb: String,
    village: String,
    details_url: String,
}

struct ResultsView {
    rows: Vec<ResultRow>,
    total: usize,
    page: usize,
    total_pages: usize,
    has_previous: bool,
    has_next: bool,
    previous_url: String,
    next_url: String,
    previous_page: usize,
    next_page: usize,
}

struct RecentMetricView {
    timestamp: String,
    method: String,
    path: String,
    status: u16,
    mode: String,
    duration_ms: String,
    response_size: String,
}

struct DashboardMetricsView {
    requests: u64,
    average_response_size: String,
    htmx_average_duration_ms: String,
    traditional_average_duration_ms: String,
    htmx_average_response_size: String,
    traditional_average_response_size: String,
    htmx_time_bar_width: String,
    traditional_time_bar_width: String,
    htmx_size_bar_width: String,
    traditional_size_bar_width: String,
    recent: Vec<RecentMetricView>,
}

struct DashboardView {
    title: String,
    active_mode: String,
    counts: LocationCounts,
    query: LocationQuery,
    provinces: Vec<SelectOption>,
    districts: Vec<SelectOption>,
    sectors: Vec<SelectOption>,
    cells: Vec<SelectOption>,
    villages: Vec<SelectOption>,
    results_html: String,
    detail_html: String,
    show_detail: bool,
    metrics: DashboardMetricsView,
}

#[derive(Template)]
#[template(path = "index.html")]
struct HtmxPageTemplate {
    view: DashboardView,
}

#[derive(Template)]
#[template(path = "traditional.html")]
struct TraditionalPageTemplate {
    view: DashboardView,
}

#[derive(Template)]
#[template(path = "fragments/location_results.html")]
struct LocationResultsTemplate {
    view: ResultsView,
    is_htmx: bool,
}

#[derive(Template)]
#[template(path = "fragments/location_details.html")]
struct LocationDetailsTemplate {
    detail: LocationDetails,
}

macro_rules! option_template {
    ($name:ident, $path:literal) => {
        #[derive(Template)]
        #[template(path = $path)]
        struct $name {
            options: Vec<SelectOption>,
        }
    };
}

option_template!(DistrictOptionsTemplate, "fragments/district_options.html");
option_template!(SectorOptionsTemplate, "fragments/sector_options.html");
option_template!(CellOptionsTemplate, "fragments/cell_options.html");
option_template!(VillageOptionsTemplate, "fragments/village_options.html");

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    dataset: &'static str,
    counts: LocationCounts,
}

pub fn configure(config: &mut web::ServiceConfig) {
    config
        .route("/", web::get().to(htmx_dashboard))
        .route("/traditional", web::get().to(traditional_dashboard))
        .route("/locations", web::get().to(traditional_dashboard))
        .route("/fragments/locations", web::get().to(location_results))
        .route("/fragments/districts", web::get().to(district_options))
        .route("/fragments/sectors", web::get().to(sector_options))
        .route("/fragments/cells", web::get().to(cell_options))
        .route("/fragments/villages", web::get().to(village_options))
        .route("/fragments/location/{id}", web::get().to(location_details))
        .route("/health", web::get().to(health))
        .route("/metrics", web::get().to(metrics));
}

async fn htmx_dashboard(
    store: web::Data<LocationStore>,
    metrics: web::Data<MetricsStore>,
    query: web::Query<LocationQuery>,
) -> Result<HttpResponse> {
    let query = query.into_inner().normalized();
    apply_delay(query.delay_ms).await;
    let view = dashboard_view(&store, &metrics, query, true)?;
    render_html(HtmxPageTemplate { view })
}

async fn traditional_dashboard(
    store: web::Data<LocationStore>,
    metrics: web::Data<MetricsStore>,
    query: web::Query<LocationQuery>,
) -> Result<HttpResponse> {
    let query = query.into_inner().normalized();
    apply_delay(query.delay_ms).await;
    let view = dashboard_view(&store, &metrics, query, false)?;
    render_html(TraditionalPageTemplate { view })
}

async fn location_results(
    store: web::Data<LocationStore>,
    query: web::Query<LocationQuery>,
) -> Result<HttpResponse> {
    let query = query.into_inner().normalized();
    apply_delay(query.delay_ms).await;
    let page = store.search(&query);
    let view = results_view(page, &query, true);
    render_html(LocationResultsTemplate {
        view,
        is_htmx: true,
    })
}

async fn district_options(
    store: web::Data<LocationStore>,
    query: web::Query<LocationQuery>,
) -> Result<HttpResponse> {
    let query = query.into_inner().normalized();
    apply_delay(query.delay_ms).await;
    let options = select_options(store.districts(&query.province), &query.district);
    render_options(DistrictOptionsTemplate { options })
}

async fn sector_options(
    store: web::Data<LocationStore>,
    query: web::Query<LocationQuery>,
) -> Result<HttpResponse> {
    let query = query.into_inner().normalized();
    apply_delay(query.delay_ms).await;
    let options = select_options(
        store.sectors(&query.province, &query.district),
        &query.sector,
    );
    render_options(SectorOptionsTemplate { options })
}

async fn cell_options(
    store: web::Data<LocationStore>,
    query: web::Query<LocationQuery>,
) -> Result<HttpResponse> {
    let query = query.into_inner().normalized();
    apply_delay(query.delay_ms).await;
    let options = select_options(
        store.cells(&query.province, &query.district, &query.sector),
        &query.cell,
    );
    render_options(CellOptionsTemplate { options })
}

async fn village_options(
    store: web::Data<LocationStore>,
    query: web::Query<LocationQuery>,
) -> Result<HttpResponse> {
    let query = query.into_inner().normalized();
    apply_delay(query.delay_ms).await;
    let options = select_options(
        store.villages(&query.province, &query.district, &query.sector, &query.cell),
        &query.village,
    );
    render_options(VillageOptionsTemplate { options })
}

async fn location_details(
    store: web::Data<LocationStore>,
    path: web::Path<String>,
    query: web::Query<LocationQuery>,
) -> Result<HttpResponse> {
    let query = query.into_inner().normalized();
    apply_delay(query.delay_ms).await;
    match store.details(&path.into_inner()) {
        Some(detail) => render_html(LocationDetailsTemplate { detail }),
        None => Ok(HttpResponse::NotFound()
            .content_type("text/html; charset=utf-8")
            .body("<div class=\"alert alert-error\">Location not found.</div>")),
    }
}

async fn health(store: web::Data<LocationStore>) -> HttpResponse {
    HttpResponse::Ok().json(HealthResponse {
        status: "ok",
        dataset: "locations.json (loaded once at startup)",
        counts: store.counts().clone(),
    })
}

async fn metrics(metrics: web::Data<MetricsStore>) -> HttpResponse {
    HttpResponse::Ok().json(metrics.snapshot())
}

fn dashboard_view(
    store: &LocationStore,
    metrics: &MetricsStore,
    query: LocationQuery,
    is_htmx: bool,
) -> Result<DashboardView> {
    let page = store.search(&query);
    let results_html = LocationResultsTemplate {
        view: results_view(page, &query, is_htmx),
        is_htmx,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    let detail = if query.detail.is_empty() {
        None
    } else {
        store.details(&query.detail)
    };
    let show_detail = detail.is_some();
    let detail_html = detail
        .map(|detail| LocationDetailsTemplate { detail }.render())
        .transpose()
        .map_err(ErrorInternalServerError)?
        .unwrap_or_default();

    Ok(DashboardView {
        title: if is_htmx {
            "HTMX Mode · Rwanda Locations Explorer"
        } else {
            "Traditional Mode · Rwanda Locations Explorer"
        }
        .to_owned(),
        active_mode: if is_htmx { "htmx" } else { "traditional" }.to_owned(),
        counts: store.counts().clone(),
        provinces: select_options(store.provinces(), &query.province),
        districts: select_options(store.districts(&query.province), &query.district),
        sectors: select_options(
            store.sectors(&query.province, &query.district),
            &query.sector,
        ),
        cells: select_options(
            store.cells(&query.province, &query.district, &query.sector),
            &query.cell,
        ),
        villages: select_options(
            store.villages(&query.province, &query.district, &query.sector, &query.cell),
            &query.village,
        ),
        query,
        results_html,
        detail_html,
        show_detail,
        metrics: metrics_view(metrics.snapshot()),
    })
}

fn results_view(page: SearchPage, query: &LocationQuery, is_htmx: bool) -> ResultsView {
    let rows = page
        .results
        .into_iter()
        .map(|location| {
            let details_url = if is_htmx {
                format!(
                    "/fragments/location/{}?delay_ms={}",
                    location.id, query.delay_ms
                )
            } else {
                traditional_url(query, page.page, &location.id)
            };
            ResultRow {
                id: location.id,
                breadcrumb: location.breadcrumb,
                village: location.village,
                details_url,
            }
        })
        .collect();

    ResultsView {
        rows,
        total: page.total,
        page: page.page,
        total_pages: page.total_pages,
        has_previous: page.has_previous,
        has_next: page.has_next,
        previous_url: traditional_url(query, page.previous_page, ""),
        next_url: traditional_url(query, page.next_page, ""),
        previous_page: page.previous_page,
        next_page: page.next_page,
    }
}

fn traditional_url(query: &LocationQuery, page: usize, detail: &str) -> String {
    format!(
        "/locations?query={}&province={}&district={}&sector={}&cell={}&village={}&page={}&per_page={}&delay_ms={}&detail={}",
        urlencoding::encode(&query.query),
        urlencoding::encode(&query.province),
        urlencoding::encode(&query.district),
        urlencoding::encode(&query.sector),
        urlencoding::encode(&query.cell),
        urlencoding::encode(&query.village),
        page,
        query.per_page,
        query.delay_ms,
        urlencoding::encode(detail),
    )
}

fn select_options(values: Vec<String>, selected: &str) -> Vec<SelectOption> {
    values
        .into_iter()
        .map(|value| SelectOption {
            selected: value == selected,
            value,
        })
        .collect()
}

fn metrics_view(snapshot: MetricsSnapshot) -> DashboardMetricsView {
    let aggregate = snapshot.aggregate;
    let max_time = aggregate
        .htmx_average_duration_ms
        .max(aggregate.traditional_average_duration_ms);
    let max_size = aggregate
        .htmx_average_response_size_bytes
        .max(aggregate.traditional_average_response_size_bytes);
    DashboardMetricsView {
        requests: aggregate.requests,
        average_response_size: format_bytes(aggregate.average_response_size_bytes),
        htmx_average_duration_ms: format!("{:.2} ms", aggregate.htmx_average_duration_ms),
        traditional_average_duration_ms: format!(
            "{:.2} ms",
            aggregate.traditional_average_duration_ms
        ),
        htmx_average_response_size: format_bytes(aggregate.htmx_average_response_size_bytes),
        traditional_average_response_size: format_bytes(
            aggregate.traditional_average_response_size_bytes,
        ),
        htmx_time_bar_width: comparison_width(aggregate.htmx_average_duration_ms, max_time),
        traditional_time_bar_width: comparison_width(
            aggregate.traditional_average_duration_ms,
            max_time,
        ),
        htmx_size_bar_width: comparison_width(aggregate.htmx_average_response_size_bytes, max_size),
        traditional_size_bar_width: comparison_width(
            aggregate.traditional_average_response_size_bytes,
            max_size,
        ),
        recent: snapshot
            .recent
            .into_iter()
            .map(recent_metric_view)
            .collect(),
    }
}

fn recent_metric_view(record: PerformanceRecord) -> RecentMetricView {
    let timestamp = record
        .timestamp
        .get(11..19)
        .unwrap_or(&record.timestamp)
        .to_owned();
    RecentMetricView {
        timestamp,
        method: record.method,
        path: record.path,
        status: record.status,
        mode: if record.is_htmx {
            "HTMX"
        } else {
            "Traditional"
        }
        .to_owned(),
        duration_ms: format!("{:.2} ms", record.duration_ms),
        response_size: record
            .response_size_bytes
            .map(|bytes| format_bytes(bytes as f64))
            .unwrap_or_else(|| "streamed".to_owned()),
    }
}

fn comparison_width(value: f64, maximum: f64) -> String {
    if maximum <= 0.0 {
        "0".to_owned()
    } else {
        format!("{:.1}", (value / maximum * 100.0).clamp(4.0, 100.0))
    }
}

fn format_bytes(bytes: f64) -> String {
    if bytes >= 1_048_576.0 {
        format!("{:.2} MiB", bytes / 1_048_576.0)
    } else if bytes >= 1_024.0 {
        format!("{:.1} KiB", bytes / 1_024.0)
    } else {
        format!("{bytes:.0} B")
    }
}

fn render_html(template: impl Template) -> Result<HttpResponse> {
    let html = template.render().map_err(ErrorInternalServerError)?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

fn render_options(template: impl Template) -> Result<HttpResponse> {
    let html = template.render().map_err(ErrorInternalServerError)?;
    Ok(HttpResponse::Ok()
        .insert_header((
            header::HeaderName::from_static("hx-trigger"),
            "filtersChanged",
        ))
        .content_type("text/html; charset=utf-8")
        .body(html))
}

async fn apply_delay(delay_ms: u64) {
    if delay_ms > 0 {
        actix_web::rt::time::sleep(Duration::from_millis(delay_ms)).await;
    }
}
