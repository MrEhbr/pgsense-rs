use metrics::{describe_counter, describe_histogram};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

pub const EVENTS_TOTAL: &str = "pgsense_events_total";
pub const FINDINGS_TOTAL: &str = "pgsense_findings_total";
pub const ALERTS_TOTAL: &str = "pgsense_alerts_total";
pub const SCAN_DURATION: &str = "pgsense_scan_duration_seconds";

pub fn init() -> PrometheusHandle {
    let handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full(SCAN_DURATION.to_string()),
            &[0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1],
        )
        .expect("valid buckets")
        .install_recorder()
        .expect("failed to install metrics recorder");

    describe_counter!(EVENTS_TOTAL, "Total replication events processed");
    describe_counter!(FINDINGS_TOTAL, "Total sensitive data findings");
    describe_counter!(ALERTS_TOTAL, "Total alerts dispatched");
    describe_histogram!(SCAN_DURATION, "Time spent scanning a single event");

    handle
}

#[cfg(test)]
mod tests {
    use metrics::{counter, histogram};

    use super::*;

    #[test]
    fn metrics_record_and_render() {
        let handle = PrometheusBuilder::new()
            .set_buckets_for_metric(
                Matcher::Full(SCAN_DURATION.to_string()),
                &[0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1],
            )
            .unwrap()
            .build_recorder();
        let prom_handle = handle.handle();

        // Must install as the global recorder for macros to work
        metrics::with_local_recorder(&handle, || {
            counter!(EVENTS_TOTAL, "database" => "localhost/test").increment(1);
            counter!(FINDINGS_TOTAL, "database" => "localhost/test", "category" => "TEST", "severity" => "HIGH").increment(1);
            counter!(ALERTS_TOTAL, "channel" => "log", "status" => "ok").increment(1);
            histogram!(SCAN_DURATION, "database" => "localhost/test").record(0.001);

            let output = prom_handle.render();
            assert!(output.contains("pgsense_events_total"));
            assert!(output.contains("pgsense_findings_total"));
            assert!(output.contains("pgsense_alerts_total"));
            assert!(output.contains("pgsense_scan_duration_seconds"));
        });
    }
}
