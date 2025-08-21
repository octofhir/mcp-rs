//! Metrics and observability implementations

pub mod health;

use anyhow::Result;
use health::{
    HealthMonitor, HealthResponse, MonitoringConfig, PerformanceMetrics, ReadinessResponse,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::RwLock;

pub use health::{HealthCheck, HealthStatus};

#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub timestamp: std::time::SystemTime,
    pub performance: PerformanceMetrics,
    pub custom_metrics: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrometheusMetrics {
    pub content_type: String,
    pub data: String,
}

pub struct MetricsProvider {
    health_monitor: Arc<HealthMonitor>,
    custom_metrics: Arc<RwLock<HashMap<String, AtomicU64>>>,
    config: MonitoringConfig,
}

impl MetricsProvider {
    pub fn new(config: MonitoringConfig, version: String) -> Self {
        Self {
            health_monitor: Arc::new(HealthMonitor::new(config.clone(), version)),
            custom_metrics: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub async fn get_health_status(&self) -> HealthResponse {
        if self.config.enable_health_checks {
            // Run health checks before returning status
            if let Err(e) = self.health_monitor.run_system_health_checks().await {
                tracing::warn!("Health check error: {}", e);
            }
        }
        self.health_monitor.get_health_status().await
    }

    pub async fn get_readiness_status(&self) -> ReadinessResponse {
        self.health_monitor.get_readiness_status().await
    }

    pub fn get_performance_metrics(&self) -> PerformanceMetrics {
        self.health_monitor.get_performance_metrics()
    }

    pub async fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        let performance = self.get_performance_metrics();
        let custom_metrics = self.get_custom_metrics().await;

        MetricsSnapshot {
            timestamp: std::time::SystemTime::now(),
            performance,
            custom_metrics,
        }
    }

    pub async fn get_prometheus_metrics(&self) -> PrometheusMetrics {
        let performance = self.get_performance_metrics();
        let custom_metrics = self.get_custom_metrics().await;

        let mut prometheus_data = String::new();

        // Performance metrics
        prometheus_data.push_str(&format!(
            "# HELP octofhir_requests_total Total number of requests\n# TYPE octofhir_requests_total counter\noctofhir_requests_total {}\n",
            performance.total_requests
        ));

        prometheus_data.push_str(&format!(
            "# HELP octofhir_requests_per_minute Current requests per minute\n# TYPE octofhir_requests_per_minute gauge\noctofhir_requests_per_minute {}\n",
            performance.requests_per_minute
        ));

        prometheus_data.push_str(&format!(
            "# HELP octofhir_response_time_avg_ms Average response time in milliseconds\n# TYPE octofhir_response_time_avg_ms gauge\noctofhir_response_time_avg_ms {}\n",
            performance.average_response_time_ms
        ));

        prometheus_data.push_str(&format!(
            "# HELP octofhir_response_time_p95_ms 95th percentile response time in milliseconds\n# TYPE octofhir_response_time_p95_ms gauge\noctofhir_response_time_p95_ms {}\n",
            performance.p95_response_time_ms
        ));

        prometheus_data.push_str(&format!(
            "# HELP octofhir_error_rate_percent Error rate percentage\n# TYPE octofhir_error_rate_percent gauge\noctofhir_error_rate_percent {}\n",
            performance.error_rate_percent
        ));

        prometheus_data.push_str(&format!(
            "# HELP octofhir_active_connections Current active connections\n# TYPE octofhir_active_connections gauge\noctofhir_active_connections {}\n",
            performance.active_connections
        ));

        prometheus_data.push_str(&format!(
            "# HELP octofhir_memory_usage_mb Memory usage in megabytes\n# TYPE octofhir_memory_usage_mb gauge\noctofhir_memory_usage_mb {}\n",
            performance.memory_usage_mb
        ));

        // Custom metrics
        for (name, value) in custom_metrics {
            prometheus_data.push_str(&format!(
                "# HELP octofhir_{name} Custom metric {name}\n# TYPE octofhir_{name} gauge\noctofhir_{name} {value}\n"
            ));
        }

        PrometheusMetrics {
            content_type: "text/plain; version=0.0.4; charset=utf-8".to_string(),
            data: prometheus_data,
        }
    }

    pub fn record_request(&self, response_time: Duration, is_error: bool) {
        if self.config.enable_metrics {
            self.health_monitor
                .record_request(response_time.as_millis() as f64, is_error);
        }
    }

    pub fn increment_active_connections(&self) {
        if self.config.enable_metrics {
            self.health_monitor.increment_active_connections();
        }
    }

    pub fn decrement_active_connections(&self) {
        if self.config.enable_metrics {
            self.health_monitor.decrement_active_connections();
        }
    }

    pub async fn increment_custom_metric(&self, name: &str, value: u64) {
        if !self.config.enable_metrics {
            return;
        }

        let mut metrics = self.custom_metrics.write().await;
        metrics
            .entry(name.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(value, Ordering::Relaxed);
    }

    pub async fn set_custom_metric(&self, name: &str, value: u64) {
        if !self.config.enable_metrics {
            return;
        }

        let mut metrics = self.custom_metrics.write().await;
        metrics
            .entry(name.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .store(value, Ordering::Relaxed);
    }

    pub async fn get_custom_metrics(&self) -> HashMap<String, f64> {
        let metrics = self.custom_metrics.read().await;
        metrics
            .iter()
            .map(|(name, value)| (name.clone(), value.load(Ordering::Relaxed) as f64))
            .collect()
    }

    pub async fn update_health_check(&self, name: impl Into<String>, check: HealthCheck) {
        self.health_monitor.update_health_check(name, check).await;
    }

    pub async fn start_periodic_health_checks(&self) -> Result<()> {
        if !self.config.enable_health_checks {
            return Ok(());
        }

        let health_monitor = self.health_monitor.clone();
        let interval_seconds = self.config.health_check_interval_seconds;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));

            loop {
                interval.tick().await;

                if let Err(e) = health_monitor.run_system_health_checks().await {
                    tracing::error!("Periodic health check failed: {}", e);
                }
            }
        });

        tracing::info!("Started periodic health checks every {}s", interval_seconds);
        Ok(())
    }

    pub fn health_monitor(&self) -> &HealthMonitor {
        &self.health_monitor
    }
}

impl Default for MetricsProvider {
    fn default() -> Self {
        Self::new(
            MonitoringConfig::default(),
            env!("CARGO_PKG_VERSION").to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_provider_creation() {
        let config = MonitoringConfig::default();
        let provider = MetricsProvider::new(config, "test-0.1.0".to_string());

        let health = provider.get_health_status().await;
        assert_eq!(health.version, "test-0.1.0");
    }

    #[tokio::test]
    async fn test_custom_metrics() {
        let provider = MetricsProvider::default();

        provider.increment_custom_metric("test_counter", 5).await;
        provider.set_custom_metric("test_gauge", 42).await;

        let metrics = provider.get_custom_metrics().await;
        assert_eq!(metrics.get("test_counter"), Some(&5.0));
        assert_eq!(metrics.get("test_gauge"), Some(&42.0));
    }

    #[tokio::test]
    async fn test_prometheus_metrics() {
        let provider = MetricsProvider::default();
        provider.increment_custom_metric("test_metric", 10).await;

        let prometheus = provider.get_prometheus_metrics().await;
        assert!(prometheus.data.contains("octofhir_test_metric 10"));
        assert_eq!(
            prometheus.content_type,
            "text/plain; version=0.0.4; charset=utf-8"
        );
    }

    #[test]
    fn test_request_recording() {
        let provider = MetricsProvider::default();

        provider.record_request(Duration::from_millis(100), false);
        provider.record_request(Duration::from_millis(200), true);

        let metrics = provider.get_performance_metrics();
        assert!(metrics.total_requests >= 2);
    }
}
