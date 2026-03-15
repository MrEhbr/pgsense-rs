use lazy_static::lazy_static;
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, IntGaugeVec, TextEncoder, register_histogram_vec, register_int_counter_vec,
    register_int_gauge, register_int_gauge_vec,
};

lazy_static! {
    pub static ref EVENTS_TOTAL: IntCounterVec =
        register_int_counter_vec!("pgsense_events_total", "Total replication events processed", &["database"]).unwrap();
    pub static ref FINDINGS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "pgsense_findings_total",
        "Total sensitive data findings",
        &["database", "category", "severity"]
    )
    .unwrap();
    pub static ref ALERTS_TOTAL: IntCounterVec = register_int_counter_vec!("pgsense_alerts_total", "Total alerts dispatched", &["channel", "status"]).unwrap();
    pub static ref PIPELINE_RECONNECTS: IntCounterVec = register_int_counter_vec!(
        "pgsense_pipeline_reconnects_total",
        "Total pipeline reconnection attempts",
        &["database"]
    )
    .unwrap();
    pub static ref EVENTS_SKIPPED: IntCounterVec = register_int_counter_vec!(
        "pgsense_events_skipped_total",
        "Total events skipped by scan filters",
        &["database", "reason"]
    )
    .unwrap();
    pub static ref DEDUP_TOTAL: IntCounterVec =
        register_int_counter_vec!("pgsense_dedup_total", "Total deduplication decisions", &["database", "outcome"]).unwrap();
    pub static ref CONFIG_RELOADS: IntCounterVec = register_int_counter_vec!(
        "pgsense_config_reloads_total",
        "Total configuration reload attempts",
        &["status"]
    )
    .unwrap();
    pub static ref SCRIPT_ERRORS: IntCounterVec = register_int_counter_vec!(
        "pgsense_script_errors_total",
        "Total Rhai script execution errors",
        &["rule_id"]
    )
    .unwrap();
    pub static ref RULES_LOADED: IntGauge = register_int_gauge!("pgsense_rules_loaded", "Number of detection rules currently loaded").unwrap();
    pub static ref PIPELINE_CONNECTED: IntGaugeVec = register_int_gauge_vec!(
        "pgsense_pipeline_connected",
        "Whether a database pipeline is connected (1) or disconnected (0)",
        &["database"]
    )
    .unwrap();
    pub static ref QUEUE_DEPTH: IntGaugeVec = register_int_gauge_vec!(
        "pgsense_queue_depth",
        "Current depth of the event channel (pending batches)",
        &["database"]
    )
    .unwrap();
    pub static ref SCAN_DURATION: HistogramVec = register_histogram_vec!(
        HistogramOpts::new("pgsense_scan_duration_seconds", "Time spent scanning a single event").buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]),
        &["database"]
    )
    .unwrap();
    pub static ref BATCH_SIZE: HistogramVec = register_histogram_vec!(
        HistogramOpts::new("pgsense_batch_size", "Number of events per batch from the pipeline")
            .buckets(vec![1.0, 5.0, 10.0, 50.0, 100.0, 250.0, 500.0, 1000.0]),
        &["database"]
    )
    .unwrap();
    pub static ref DISPATCH_DURATION: HistogramVec = register_histogram_vec!(
        HistogramOpts::new(
            "pgsense_dispatch_duration_seconds",
            "Time spent dispatching alerts for a single event"
        )
        .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]),
        &["database"]
    )
    .unwrap();
}

pub fn init() {
    lazy_static::initialize(&EVENTS_TOTAL);
    lazy_static::initialize(&FINDINGS_TOTAL);
    lazy_static::initialize(&ALERTS_TOTAL);
    lazy_static::initialize(&PIPELINE_RECONNECTS);
    lazy_static::initialize(&EVENTS_SKIPPED);
    lazy_static::initialize(&DEDUP_TOTAL);
    lazy_static::initialize(&CONFIG_RELOADS);
    lazy_static::initialize(&SCRIPT_ERRORS);
    lazy_static::initialize(&RULES_LOADED);
    lazy_static::initialize(&PIPELINE_CONNECTED);
    lazy_static::initialize(&QUEUE_DEPTH);
    lazy_static::initialize(&SCAN_DURATION);
    lazy_static::initialize(&BATCH_SIZE);
    lazy_static::initialize(&DISPATCH_DURATION);

    #[cfg(target_os = "linux")]
    {
        let collector = prometheus::process_collector::ProcessCollector::for_self();
        let _ = prometheus::register(Box::new(collector));
    }
}

pub fn render() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_record_and_render() {
        init();

        EVENTS_TOTAL.with_label_values(&["localhost/test"]).inc();
        FINDINGS_TOTAL
            .with_label_values(&["localhost/test", "TEST", "HIGH"])
            .inc();
        ALERTS_TOTAL.with_label_values(&["log", "ok"]).inc();
        SCAN_DURATION
            .with_label_values(&["localhost/test"])
            .observe(0.001);
        BATCH_SIZE
            .with_label_values(&["localhost/test"])
            .observe(42.0);
        QUEUE_DEPTH.with_label_values(&["localhost/test"]).set(5);
        DISPATCH_DURATION
            .with_label_values(&["localhost/test"])
            .observe(0.002);
        PIPELINE_CONNECTED
            .with_label_values(&["localhost/test"])
            .set(1);
        PIPELINE_RECONNECTS
            .with_label_values(&["localhost/test"])
            .inc();
        EVENTS_SKIPPED
            .with_label_values(&["localhost/test", "schema_excluded"])
            .inc();
        DEDUP_TOTAL
            .with_label_values(&["localhost/test", "suppressed"])
            .inc();
        RULES_LOADED.set(20);
        CONFIG_RELOADS.with_label_values(&["ok"]).inc();
        SCRIPT_ERRORS.with_label_values(&["test-rule"]).inc();

        let output = render();
        assert!(output.contains("pgsense_events_total"));
        assert!(output.contains("pgsense_findings_total"));
        assert!(output.contains("pgsense_alerts_total"));
        assert!(output.contains("pgsense_scan_duration_seconds"));
        assert!(output.contains("pgsense_batch_size"));
        assert!(output.contains("pgsense_queue_depth"));
        assert!(output.contains("pgsense_dispatch_duration_seconds"));
        assert!(output.contains("pgsense_pipeline_connected"));
        assert!(output.contains("pgsense_pipeline_reconnects_total"));
        assert!(output.contains("pgsense_events_skipped_total"));
        assert!(output.contains("pgsense_dedup_total"));
        assert!(output.contains("pgsense_rules_loaded"));
        assert!(output.contains("pgsense_config_reloads_total"));
        assert!(output.contains("pgsense_script_errors_total"));
    }
}
