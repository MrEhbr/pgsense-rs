use prometheus::{Encoder, Histogram, HistogramOpts, IntCounter, IntCounterVec, Opts, Registry, TextEncoder};

#[derive(Clone)]
pub struct Metrics {
    pub events_total: IntCounter,
    pub findings_total: IntCounterVec,
    pub alerts_total: IntCounterVec,
    pub scan_duration_seconds: Histogram,
    registry: Registry,
}

impl Metrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        let events_total = IntCounter::with_opts(Opts::new("pgsense_events_total", "Total replication events processed")).unwrap();

        let findings_total = IntCounterVec::new(
            Opts::new("pgsense_findings_total", "Total sensitive data findings"),
            &["category", "severity"],
        )
        .unwrap();

        let alerts_total = IntCounterVec::new(
            Opts::new("pgsense_alerts_total", "Total alerts dispatched"),
            &["channel", "status"],
        )
        .unwrap();

        let scan_duration_seconds = Histogram::with_opts(
            HistogramOpts::new("pgsense_scan_duration_seconds", "Time spent scanning a single event")
                .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]),
        )
        .unwrap();

        registry.register(Box::new(events_total.clone())).unwrap();
        registry.register(Box::new(findings_total.clone())).unwrap();
        registry.register(Box::new(alerts_total.clone())).unwrap();
        registry
            .register(Box::new(scan_duration_seconds.clone()))
            .unwrap();

        Self {
            events_total,
            findings_total,
            alerts_total,
            scan_duration_seconds,
            registry,
        }
    }

    pub fn encode(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_register_and_encode() {
        let m = Metrics::new();
        m.events_total.inc();
        m.findings_total.with_label_values(&["TEST", "HIGH"]).inc();
        m.alerts_total.with_label_values(&["log", "ok"]).inc();
        m.scan_duration_seconds.observe(0.001);

        let output = m.encode();
        assert!(output.contains("pgsense_events_total 1"));
        assert!(output.contains("pgsense_findings_total"));
        assert!(output.contains("pgsense_alerts_total"));
        assert!(output.contains("pgsense_scan_duration_seconds"));
    }
}
