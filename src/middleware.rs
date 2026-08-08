use std::{
    future::{ready, Ready},
    rc::Rc,
    time::Instant,
};

use actix_web::{
    body::{BodySize, BoxBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderName, HeaderValue, CONTENT_LENGTH},
    Error,
};
use futures_util::future::LocalBoxFuture;

use crate::metrics::MetricsStore;

pub struct PerformanceMiddleware {
    metrics: MetricsStore,
}

impl PerformanceMiddleware {
    pub fn new(metrics: MetricsStore) -> Self {
        Self { metrics }
    }
}

impl<S, B> Transform<S, ServiceRequest> for PerformanceMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type InitError = ();
    type Transform = PerformanceMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(PerformanceMiddlewareService {
            service: Rc::new(service),
            metrics: self.metrics.clone(),
        }))
    }
}

pub struct PerformanceMiddlewareService<S> {
    service: Rc<S>,
    metrics: MetricsStore,
}

impl<S, B> Service<ServiceRequest> for PerformanceMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, request: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        let metrics = self.metrics.clone();
        let method = request.method().to_string();
        let path = request.path().to_owned();
        let is_htmx = request
            .headers()
            .get("HX-Request")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.eq_ignore_ascii_case("true"));
        let started = Instant::now();

        Box::pin(async move {
            let mut response = service.call(request).await?;
            let duration_ms = started.elapsed().as_secs_f64() * 1_000.0;
            let status = response.status().as_u16();
            let response_size = match response.response().body().size() {
                BodySize::Sized(bytes) => Some(bytes as usize),
                _ => response
                    .headers()
                    .get(CONTENT_LENGTH)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse().ok()),
            };

            if let Ok(value) = HeaderValue::from_str(&format!("app;dur={duration_ms:.3}")) {
                response
                    .headers_mut()
                    .insert(HeaderName::from_static("server-timing"), value);
            }

            let record = metrics.record(method, path, status, is_htmx, duration_ms, response_size);

            tracing::info!(
                event = "request_performance",
                timestamp = %record.timestamp,
                method = %record.method,
                path = %record.path,
                status = record.status,
                htmx = record.is_htmx,
                server_ms = record.duration_ms,
                response_bytes = ?record.response_size_bytes,
            );

            Ok(response.map_into_boxed_body())
        })
    }
}
