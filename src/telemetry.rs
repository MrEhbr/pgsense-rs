use anyhow::{Context, Result};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    trace::{Sampler, SdkTracerProvider, Tracer},
};

use crate::config::{OtlpProtocol, TelemetryConfig};

/// Build an OTel resource with service metadata.
fn build_resource(config: &TelemetryConfig) -> Resource {
    Resource::builder()
        .with_service_name(config.service_name.clone())
        .build()
}

/// Initialize the OTel tracer provider and return both the provider (for
/// shutdown) and a tracer (for the tracing-opentelemetry layer).
pub fn init_tracer(config: &TelemetryConfig) -> Result<(SdkTracerProvider, Tracer)> {
    let exporter = match config.protocol {
        OtlpProtocol::Grpc => SpanExporter::builder()
            .with_tonic()
            .with_endpoint(&config.endpoint)
            .build()
            .context("failed to build OTLP gRPC span exporter")?,
        OtlpProtocol::Http => SpanExporter::builder()
            .with_http()
            .with_endpoint(&config.endpoint)
            .build()
            .context("failed to build OTLP HTTP span exporter")?,
    };

    let sampler = if (config.sample_rate - 1.0).abs() < f64::EPSILON {
        Sampler::AlwaysOn
    } else if config.sample_rate <= 0.0 {
        Sampler::AlwaysOff
    } else {
        Sampler::TraceIdRatioBased(config.sample_rate)
    };

    let provider = SdkTracerProvider::builder()
        .with_resource(build_resource(config))
        .with_sampler(sampler)
        .with_batch_exporter(exporter)
        .build();

    let tracer = provider.tracer("pgsense");

    Ok((provider, tracer))
}
