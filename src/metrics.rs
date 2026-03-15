use metrics::{describe_counter, describe_gauge, describe_histogram};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

pub const EVENTS_TOTAL: &str = "pgsense_events_total";
pub const FINDINGS_TOTAL: &str = "pgsense_findings_total";
pub const ALERTS_TOTAL: &str = "pgsense_alerts_total";
pub const SCAN_DURATION: &str = "pgsense_scan_duration_seconds";
pub const BATCH_SIZE: &str = "pgsense_batch_size";
pub const QUEUE_DEPTH: &str = "pgsense_queue_depth";
pub const DISPATCH_DURATION: &str = "pgsense_dispatch_duration_seconds";

pub fn init() -> PrometheusHandle {
    let handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full(SCAN_DURATION.to_string()),
            &[0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1],
        )
        .expect("valid buckets")
        .set_buckets_for_metric(
            Matcher::Full(DISPATCH_DURATION.to_string()),
            &[0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0],
        )
        .expect("valid buckets")
        .set_buckets_for_metric(
            Matcher::Full(BATCH_SIZE.to_string()),
            &[1.0, 5.0, 10.0, 50.0, 100.0, 250.0, 500.0, 1000.0],
        )
        .expect("valid buckets")
        .install_recorder()
        .expect("failed to install metrics recorder");

    describe_counter!(EVENTS_TOTAL, "Total replication events processed");
    describe_counter!(FINDINGS_TOTAL, "Total sensitive data findings");
    describe_counter!(ALERTS_TOTAL, "Total alerts dispatched");
    describe_histogram!(SCAN_DURATION, "Time spent scanning a single event");
    describe_histogram!(BATCH_SIZE, "Number of events per batch from the pipeline");
    describe_gauge!(QUEUE_DEPTH, "Current depth of the event channel (pending batches)");
    describe_histogram!(DISPATCH_DURATION, "Time spent dispatching alerts for a single event");

    handle
}

#[cfg(test)]
mod tests {
    use metrics::{counter, gauge, histogram};

    use super::*;

    #[test]
    fn metrics_record_and_render() {
        let handle = PrometheusBuilder::new()
            .set_buckets_for_metric(
                Matcher::Full(SCAN_DURATION.to_string()),
                &[0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1],
            )
            .unwrap()
            .set_buckets_for_metric(
                Matcher::Full(DISPATCH_DURATION.to_string()),
                &[0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0],
            )
            .unwrap()
            .set_buckets_for_metric(
                Matcher::Full(BATCH_SIZE.to_string()),
                &[1.0, 5.0, 10.0, 50.0, 100.0, 250.0, 500.0, 1000.0],
            )
            .unwrap()
            .build_recorder();
        let prom_handle = handle.handle();

        metrics::with_local_recorder(&handle, || {
            counter!(EVENTS_TOTAL, "database" => "localhost/test").increment(1);
            counter!(FINDINGS_TOTAL, "database" => "localhost/test", "category" => "TEST", "severity" => "HIGH").increment(1);
            counter!(ALERTS_TOTAL, "channel" => "log", "status" => "ok").increment(1);
            histogram!(SCAN_DURATION, "database" => "localhost/test").record(0.001);
            histogram!(BATCH_SIZE, "database" => "localhost/test").record(42.0);
            gauge!(QUEUE_DEPTH, "database" => "localhost/test").set(5.0);
            histogram!(DISPATCH_DURATION, "database" => "localhost/test").record(0.002);

            let output = prom_handle.render();
            assert!(output.contains("pgsense_events_total"));
            assert!(output.contains("pgsense_findings_total"));
            assert!(output.contains("pgsense_alerts_total"));
            assert!(output.contains("pgsense_scan_duration_seconds"));
            assert!(output.contains("pgsense_batch_size"));
            assert!(output.contains("pgsense_queue_depth"));
            assert!(output.contains("pgsense_dispatch_duration_seconds"));
        });
    }
}
