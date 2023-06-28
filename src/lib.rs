pub mod connection;

use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use tracing::{info, info_span};

use opentelemetry::sdk::{
    propagation::TraceContextPropagator,
    trace::{RandomIdGenerator, Sampler},
};
use opentelemetry_otlp::WithExportConfig;
use tracing_subscriber::{
    fmt::format::FmtSpan, prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Layer,
};

pub async fn get_server() -> anyhow::Result<Router> {
    let router = Router::new()
        .route("/", get(health))
        .route("/echo/*rest", get(echo))
        .layer(tower_http::trace::TraceLayer::new_for_http());

    Ok(router)
}

pub async fn health() -> impl IntoResponse {
    "up"
}

pub async fn echo(Path(s): Path<String>) -> impl IntoResponse {
    info_span!("cockledoodledoo").in_scope(|| {
        info!("I'm a chicken! but with {}", s);
    });
    s
}

pub fn setup_tracing() -> anyhow::Result<()> {
    tracing::info!("setup_tracing");
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            // We actually export using gRPC, and also need to
            // override the default localhost endpoint to use
            // HTTP vs HTTPS.
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_timeout(std::time::Duration::from_secs(5))
                .with_endpoint("http://localhost:4317"),
        )
        .with_trace_config(
            opentelemetry::sdk::trace::config()
                .with_sampler(Sampler::AlwaysOn)
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(opentelemetry::sdk::Resource::new(vec![
                    opentelemetry::KeyValue::new(
                        opentelemetry_semantic_conventions::resource::SERVICE_NAME,
                        "turmoil_test",
                    ),
                ])),
        )
        // .install_simple()
        .install_batch(opentelemetry::runtime::Tokio)?;

    tracing::info!("initialized tracing");

    // Here, we create the tracing/OpenTelemetry bridge and then attach
    // it to the standard [`tracing_subscriber::Registry`], then set it
    // as the global subscriber to tracing events/spans.
    let tracing_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer)
        .with_filter(EnvFilter::from_default_env());

    // This layer takes tracing events and prints it out to console
    let span_event = FmtSpan::ACTIVE;
    let console_layer = tracing_subscriber::fmt::layer()
        .event_format(tracing_subscriber::fmt::format().with_ansi(false))
        .with_span_events(span_event)
        .with_filter(EnvFilter::from_default_env());

    // This puts the multiple subscriber layers together
    let subscriber = tracing_subscriber::registry()
        .with(tracing_layer)
        .with(console_layer);

    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to register global tracing subscriber");

    Ok(())
}
