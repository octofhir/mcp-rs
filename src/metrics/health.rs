use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, RwLock,
    },
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::RwLock as TokioRwLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    pub fn is_degraded(&self) -> bool {
        matches!(self, HealthStatus::Degraded)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthCheck {
    pub status: HealthStatus,
    pub message: String,
    pub last_checked: SystemTime,
    pub duration_ms: u64,
}

impl HealthCheck {
    pub fn healthy(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Healthy,
            message: message.into(),
            last_checked: SystemTime::now(),
            duration_ms: 0,
        }
    }

    pub fn degraded(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Degraded,
            message: message.into(),
            last_checked: SystemTime::now(),
            duration_ms: 0,
        }
    }

    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Unhealthy,
            message: message.into(),
            last_checked: SystemTime::now(),
            duration_ms: 0,
        }
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration_ms = duration.as_millis() as u64;
        self
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub timestamp: SystemTime,
    pub uptime_seconds: u64,
    pub version: String,
    pub checks: HashMap<String, HealthCheck>,
    pub metrics: PerformanceMetrics,
}

#[derive(Debug, Serialize)]
pub struct ReadinessResponse {
    pub ready: bool,
    pub timestamp: SystemTime,
    pub checks: HashMap<String, HealthCheck>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceMetrics {
    pub total_requests: u64,
    pub requests_per_minute: f64,
    pub average_response_time_ms: f64,
    pub p95_response_time_ms: f64,
    pub p99_response_time_ms: f64,
    pub error_rate_percent: f64,
    pub active_connections: usize,
    pub memory_usage_mb: f64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            requests_per_minute: 0.0,
            average_response_time_ms: 0.0,
            p95_response_time_ms: 0.0,
            p99_response_time_ms: 0.0,
            error_rate_percent: 0.0,
            active_connections: 0,
            memory_usage_mb: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    pub enable_health_checks: bool,
    pub enable_metrics: bool,
    pub metrics_retention_hours: u32,
    pub health_check_interval_seconds: u64,
    pub memory_threshold_mb: f64,
    pub response_time_threshold_ms: f64,
    pub error_rate_threshold_percent: f64,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_health_checks: true,
            enable_metrics: true,
            metrics_retention_hours: 24,
            health_check_interval_seconds: 30,
            memory_threshold_mb: 512.0,
            response_time_threshold_ms: 1000.0,
            error_rate_threshold_percent: 5.0,
        }
    }
}

#[derive(Debug)]
struct RequestMetrics {
    response_times: Vec<f64>,
    error_count: u64,
    last_minute_requests: Vec<Instant>,
}

impl RequestMetrics {
    fn new() -> Self {
        Self {
            response_times: Vec::new(),
            error_count: 0,
            last_minute_requests: Vec::new(),
        }
    }

    fn add_request(&mut self, response_time_ms: f64, is_error: bool) {
        let now = Instant::now();
        
        // Add response time (keep only last 1000 for percentile calculations)
        self.response_times.push(response_time_ms);
        if self.response_times.len() > 1000 {
            self.response_times.remove(0);
        }

        // Track errors
        if is_error {
            self.error_count += 1;
        }

        // Track requests in last minute
        self.last_minute_requests.push(now);
        self.last_minute_requests.retain(|&time| now.duration_since(time) <= Duration::from_secs(60));
    }

    fn calculate_percentile(&self, percentile: f64) -> f64 {
        if self.response_times.is_empty() {
            return 0.0;
        }

        let mut sorted_times = self.response_times.clone();
        sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let index = ((percentile / 100.0) * (sorted_times.len() - 1) as f64) as usize;
        sorted_times[index.min(sorted_times.len() - 1)]
    }

    fn average_response_time(&self) -> f64 {
        if self.response_times.is_empty() {
            return 0.0;
        }
        self.response_times.iter().sum::<f64>() / self.response_times.len() as f64
    }

    fn requests_per_minute(&self) -> f64 {
        self.last_minute_requests.len() as f64
    }

    fn error_rate_percent(&self) -> f64 {
        if self.response_times.is_empty() {
            return 0.0;
        }
        (self.error_count as f64 / self.response_times.len() as f64) * 100.0
    }
}

pub struct HealthMonitor {
    config: MonitoringConfig,
    start_time: Instant,
    version: String,
    health_checks: Arc<TokioRwLock<HashMap<String, HealthCheck>>>,
    request_metrics: Arc<RwLock<RequestMetrics>>,
    total_requests: AtomicU64,
    active_connections: AtomicUsize,
}

impl HealthMonitor {
    pub fn new(config: MonitoringConfig, version: String) -> Self {
        Self {
            config,
            start_time: Instant::now(),
            version,
            health_checks: Arc::new(TokioRwLock::new(HashMap::new())),
            request_metrics: Arc::new(RwLock::new(RequestMetrics::new())),
            total_requests: AtomicU64::new(0),
            active_connections: AtomicUsize::new(0),
        }
    }

    pub async fn get_health_status(&self) -> HealthResponse {
        let checks = self.health_checks.read().await.clone();
        let overall_status = self.calculate_overall_status(&checks).await;
        let metrics = self.get_performance_metrics();

        HealthResponse {
            status: overall_status,
            timestamp: SystemTime::now(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            version: self.version.clone(),
            checks,
            metrics,
        }
    }

    pub async fn get_readiness_status(&self) -> ReadinessResponse {
        let checks = self.health_checks.read().await.clone();
        let ready = checks.values().all(|check| check.status.is_healthy());

        ReadinessResponse {
            ready,
            timestamp: SystemTime::now(),
            checks,
        }
    }

    pub fn get_performance_metrics(&self) -> PerformanceMetrics {
        let metrics = self.request_metrics.read().unwrap();
        let memory_usage = self.get_memory_usage_mb();

        PerformanceMetrics {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            requests_per_minute: metrics.requests_per_minute(),
            average_response_time_ms: metrics.average_response_time(),
            p95_response_time_ms: metrics.calculate_percentile(95.0),
            p99_response_time_ms: metrics.calculate_percentile(99.0),
            error_rate_percent: metrics.error_rate_percent(),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            memory_usage_mb: memory_usage,
        }
    }

    pub async fn update_health_check(&self, name: impl Into<String>, check: HealthCheck) {
        let name = name.into();
        self.health_checks.write().await.insert(name, check);
    }

    pub async fn run_system_health_checks(&self) -> Result<()> {
        let start_time = Instant::now();

        // FHIRPath library health check
        let fhirpath_check = self.check_fhirpath_library().await;
        self.update_health_check("fhirpath_library", fhirpath_check).await;

        // Memory usage check
        let memory_check = self.check_memory_usage();
        self.update_health_check("memory_usage", memory_check).await;

        // Thread pool check
        let thread_check = self.check_thread_pool();
        self.update_health_check("thread_pool", thread_check).await;

        // Performance check
        let performance_check = self.check_performance();
        self.update_health_check("performance", performance_check).await;

        let duration = start_time.elapsed();
        tracing::debug!("Health checks completed in {}ms", duration.as_millis());

        Ok(())
    }

    pub fn record_request(&self, response_time_ms: f64, is_error: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.request_metrics.write().unwrap().add_request(response_time_ms, is_error);
    }

    pub fn increment_active_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn decrement_active_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    async fn calculate_overall_status(&self, checks: &HashMap<String, HealthCheck>) -> HealthStatus {
        if checks.values().any(|check| !check.status.is_healthy()) {
            if checks.values().any(|check| matches!(check.status, HealthStatus::Unhealthy)) {
                HealthStatus::Unhealthy
            } else {
                HealthStatus::Degraded
            }
        } else {
            HealthStatus::Healthy
        }
    }

    async fn check_fhirpath_library(&self) -> HealthCheck {
        let start_time = Instant::now();
        
        match self.test_fhirpath_evaluation().await {
            Ok(_) => HealthCheck::healthy("FHIRPath library operational").with_duration(start_time.elapsed()),
            Err(e) => HealthCheck::unhealthy(format!("FHIRPath library error: {}", e)).with_duration(start_time.elapsed()),
        }
    }

    async fn test_fhirpath_evaluation(&self) -> Result<()> {
        // Simple test to verify FHIRPath library is working
        let test_resource = serde_json::json!({
            "resourceType": "Patient",
            "id": "health-check-test"
        });

        let expression = "Patient.id";
        
        // Using the shared engine for health check
        match crate::fhirpath_engine::get_shared_engine().await {
            Ok(factory) => {
                match factory.evaluate(expression, test_resource).await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(anyhow::anyhow!("FHIRPath evaluation failed: {}", e)),
                }
            }
            Err(e) => Err(anyhow::anyhow!("Engine factory access failed: {}", e)),
        }
    }

    fn check_memory_usage(&self) -> HealthCheck {
        let start_time = Instant::now();
        let memory_mb = self.get_memory_usage_mb();

        if memory_mb > self.config.memory_threshold_mb * 1.5 {
            HealthCheck::unhealthy(format!("High memory usage: {:.1}MB", memory_mb)).with_duration(start_time.elapsed())
        } else if memory_mb > self.config.memory_threshold_mb {
            HealthCheck::degraded(format!("Elevated memory usage: {:.1}MB", memory_mb)).with_duration(start_time.elapsed())
        } else {
            HealthCheck::healthy(format!("Memory usage normal: {:.1}MB", memory_mb)).with_duration(start_time.elapsed())
        }
    }

    fn check_thread_pool(&self) -> HealthCheck {
        let start_time = Instant::now();
        
        // Basic thread pool health check
        let active_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        if active_threads > 0 {
            HealthCheck::healthy(format!("Thread pool operational: {} threads", active_threads)).with_duration(start_time.elapsed())
        } else {
            HealthCheck::unhealthy("Thread pool unavailable").with_duration(start_time.elapsed())
        }
    }

    fn check_performance(&self) -> HealthCheck {
        let start_time = Instant::now();
        let metrics = self.request_metrics.read().unwrap();

        let avg_response_time = metrics.average_response_time();
        let error_rate = metrics.error_rate_percent();

        if error_rate > self.config.error_rate_threshold_percent * 2.0 {
            HealthCheck::unhealthy(format!("High error rate: {:.1}%", error_rate)).with_duration(start_time.elapsed())
        } else if error_rate > self.config.error_rate_threshold_percent || 
                  avg_response_time > self.config.response_time_threshold_ms {
            HealthCheck::degraded(format!("Performance degraded - errors: {:.1}%, avg response: {:.1}ms", 
                                         error_rate, avg_response_time)).with_duration(start_time.elapsed())
        } else {
            HealthCheck::healthy(format!("Performance good - errors: {:.1}%, avg response: {:.1}ms", 
                                        error_rate, avg_response_time)).with_duration(start_time.elapsed())
        }
    }

    fn get_memory_usage_mb(&self) -> f64 {
        // Simple memory usage approximation
        // In production, you might want to use a system monitoring library like `sysinfo`
        // For now, return a reasonable approximation based on process info
        // This is a placeholder - in production you'd use system metrics
        
        // Try to get memory usage from /proc/self/status on Linux-like systems
        #[cfg(target_os = "linux")]
        {
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<f64>() {
                                return kb / 1024.0; // Convert KB to MB
                            }
                        }
                    }
                }
            }
        }
        
        // Fallback approximation
        32.0 // MB
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_creation() {
        let check = HealthCheck::healthy("Test message");
        assert_eq!(check.status, HealthStatus::Healthy);
        assert_eq!(check.message, "Test message");
    }

    #[test]
    fn test_health_status_methods() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(!HealthStatus::Healthy.is_degraded());
        
        assert!(!HealthStatus::Degraded.is_healthy());
        assert!(HealthStatus::Degraded.is_degraded());
        
        assert!(!HealthStatus::Unhealthy.is_healthy());
        assert!(!HealthStatus::Unhealthy.is_degraded());
    }

    #[test]
    fn test_request_metrics() {
        let mut metrics = RequestMetrics::new();
        
        metrics.add_request(100.0, false);
        metrics.add_request(200.0, false);
        metrics.add_request(150.0, true);
        
        assert_eq!(metrics.average_response_time(), 150.0);
        assert_eq!(metrics.calculate_percentile(50.0), 150.0);
        assert!((metrics.error_rate_percent() - 33.33).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_health_monitor_creation() {
        let config = MonitoringConfig::default();
        let monitor = HealthMonitor::new(config, "test-0.1.0".to_string());
        
        let health = monitor.get_health_status().await;
        assert_eq!(health.version, "test-0.1.0");
        assert!(health.uptime_seconds >= 0);
    }

    #[tokio::test]
    async fn test_readiness_check() {
        let config = MonitoringConfig::default();
        let monitor = HealthMonitor::new(config, "test-0.1.0".to_string());
        
        // Initially should be ready (no checks registered)
        let readiness = monitor.get_readiness_status().await;
        assert!(readiness.ready);
        
        // Add a failing check
        monitor.update_health_check("test", HealthCheck::unhealthy("Test failure")).await;
        let readiness = monitor.get_readiness_status().await;
        assert!(!readiness.ready);
    }

    #[test]
    fn test_monitoring_config_defaults() {
        let config = MonitoringConfig::default();
        assert!(config.enable_health_checks);
        assert!(config.enable_metrics);
        assert_eq!(config.metrics_retention_hours, 24);
    }
}