use anyhow::Result;
use octofhir_mcp::transport::{McpMessage, ToolsCallParams, JsonRpcMessage};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Test utilities for integration testing
pub mod test_utils {
    use super::*;

    /// Load test fixture from file
    pub fn load_fixture(path: &str) -> Result<Value> {
        let fixture_path = Path::new("tests/fixtures").join(path);
        let content = std::fs::read_to_string(fixture_path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Create a test FHIR Patient resource
    pub fn create_test_patient() -> Value {
        json!({
            "resourceType": "Patient",
            "id": "test-patient-123",
            "name": [{
                "use": "official",
                "family": "Doe",
                "given": ["John", "Michael"]
            }],
            "gender": "male",
            "birthDate": "1980-01-01",
            "telecom": [{
                "system": "phone",
                "value": "+1-555-123-4567",
                "use": "home"
            }],
            "address": [{
                "use": "home",
                "line": ["123 Main St"],
                "city": "Anytown",
                "state": "CA",
                "postalCode": "12345",
                "country": "US"
            }]
        })
    }

    /// Create a test FHIR Observation resource
    pub fn create_test_observation() -> Value {
        json!({
            "resourceType": "Observation",
            "id": "test-observation-456",
            "status": "final",
            "category": [{
                "coding": [{
                    "system": "http://terminology.hl7.org/CodeSystem/observation-category",
                    "code": "vital-signs"
                }]
            }],
            "code": {
                "coding": [{
                    "system": "http://loinc.org",
                    "code": "8310-5",
                    "display": "Body temperature"
                }]
            },
            "subject": {
                "reference": "Patient/test-patient-123"
            },
            "valueQuantity": {
                "value": 36.5,
                "unit": "°C",
                "system": "http://unitsofmeasure.org",
                "code": "Cel"
            }
        })
    }

    /// Create a test FHIR Bundle resource
    pub fn create_test_bundle() -> Value {
        json!({
            "resourceType": "Bundle",
            "id": "test-bundle-789",
            "type": "collection",
            "entry": [
                {
                    "resource": create_test_patient()
                },
                {
                    "resource": create_test_observation()
                }
            ]
        })
    }

    /// Generate a unique client ID
    pub fn generate_client_id() -> String {
        format!("test-client-{}", Uuid::new_v4())
    }

    /// Create test MCP initialize message
    pub fn create_initialize_message() -> McpMessage {
        McpMessage::Initialize {
            id: 1,
            params: octofhir_mcp::transport::InitializeParams {
                protocol_version: "1.0.0".to_string(),
                capabilities: octofhir_mcp::transport::ClientCapabilities {
                    tools: Some(octofhir_mcp::transport::ToolsCapability {
                        list_changed: Some(true),
                    }),
                    resources: None,
                    prompts: None,
                    logging: None,
                },
                client_info: octofhir_mcp::transport::ClientInfo {
                    name: "test-client".to_string(),
                    version: "1.0.0".to_string(),
                },
            },
        }
    }

    /// Create test tool call message
    pub fn create_tool_call_message(tool_name: &str, arguments: Value) -> McpMessage {
        McpMessage::ToolsCall {
            id: 2,
            params: ToolsCallParams {
                name: tool_name.to_string(),
                arguments: Some(arguments),
            },
        }
    }

    /// Create test tools list message
    pub fn create_tools_list_message() -> McpMessage {
        McpMessage::ToolsList { id: 3 }
    }
}

/// Mock MCP client for testing
pub struct MockMcpClient {
    pub messages_sent: Vec<JsonRpcMessage>,
    pub messages_received: Vec<JsonRpcMessage>,
    sender: mpsc::UnboundedSender<JsonRpcMessage>,
    receiver: mpsc::UnboundedReceiver<JsonRpcMessage>,
}

impl MockMcpClient {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            messages_sent: Vec::new(),
            messages_received: Vec::new(),
            sender,
            receiver,
        }
    }

    /// Send a message to the server
    pub async fn send_message(&mut self, message: McpMessage) -> Result<()> {
        let jsonrpc_message = message.to_jsonrpc();
        self.messages_sent.push(jsonrpc_message.clone());
        self.sender.send(jsonrpc_message)?;
        Ok(())
    }

    /// Wait for a response message
    pub async fn wait_for_response(&mut self) -> Result<Option<JsonRpcMessage>> {
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.receiver.recv()
        ).await {
            Ok(Some(message)) => {
                self.messages_received.push(message.clone());
                Ok(Some(message))
            }
            Ok(None) => Ok(None),
            Err(_) => Err(anyhow::anyhow!("Timeout waiting for response")),
        }
    }

    /// Get the number of messages sent
    pub fn sent_count(&self) -> usize {
        self.messages_sent.len()
    }

    /// Get the number of messages received
    pub fn received_count(&self) -> usize {
        self.messages_received.len()
    }

    /// Get the last sent message
    pub fn last_sent(&self) -> Option<&JsonRpcMessage> {
        self.messages_sent.last()
    }

    /// Get the last received message
    pub fn last_received(&self) -> Option<&JsonRpcMessage> {
        self.messages_received.last()
    }
}

/// Test server configuration
pub struct TestServerConfig {
    pub port: u16,
    pub host: String,
    pub enable_auth: bool,
    pub api_keys: Vec<String>,
    pub enable_metrics: bool,
    pub enable_health_checks: bool,
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            port: 0, // Use ephemeral port for testing
            host: "127.0.0.1".to_string(),
            enable_auth: false, // Disable auth for easier testing
            api_keys: vec!["test-api-key".to_string()],
            enable_metrics: true,
            enable_health_checks: true,
        }
    }
}

impl TestServerConfig {
    pub fn with_auth(mut self, enabled: bool) -> Self {
        self.enable_auth = enabled;
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn with_api_keys(mut self, keys: Vec<String>) -> Self {
        self.api_keys = keys;
        self
    }
}

/// HTTP client helper for testing HTTP transport
#[derive(Clone)]
pub struct TestHttpClient {
    pub base_url: String,
    pub client: reqwest::Client,
    pub default_headers: HashMap<String, String>,
}

impl TestHttpClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
            default_headers: HashMap::new(),
        }
    }

    pub fn with_auth_header(mut self, token: String) -> Self {
        self.default_headers.insert("Authorization".to_string(), format!("Bearer {}", token));
        self
    }

    /// Call a tool via HTTP
    pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<Value> {
        let url = format!("{}/mcp/tools/{}", self.base_url, tool_name);
        let mut request = self.client.post(&url).json(&json!({
            "arguments": arguments
        }));

        // Add default headers
        for (key, value) in &self.default_headers {
            request = request.header(key, value);
        }

        let response = request.send().await?;
        let status = response.status();
        let body: Value = response.json().await?;

        if status.is_success() {
            Ok(body)
        } else {
            Err(anyhow::anyhow!("HTTP request failed with status {}: {}", status, body))
        }
    }

    /// Get tools list via HTTP
    pub async fn get_tools_list(&self) -> Result<Value> {
        let url = format!("{}/mcp/tools/list", self.base_url);
        let mut request = self.client.get(&url);

        // Add default headers
        for (key, value) in &self.default_headers {
            request = request.header(key, value);
        }

        let response = request.send().await?;
        let status = response.status();
        let body: Value = response.json().await?;

        if status.is_success() {
            Ok(body)
        } else {
            Err(anyhow::anyhow!("HTTP request failed with status {}: {}", status, body))
        }
    }

    /// Get health status
    pub async fn get_health_status(&self) -> Result<Value> {
        let url = format!("{}/health", self.base_url);
        let response = self.client.get(&url).send().await?;
        let status = response.status();
        let body: Value = response.json().await?;

        if status.is_success() {
            Ok(body)
        } else {
            Err(anyhow::anyhow!("Health check failed with status {}: {}", status, body))
        }
    }

    /// Get readiness status
    pub async fn get_readiness_status(&self) -> Result<Value> {
        let url = format!("{}/ready", self.base_url);
        let response = self.client.get(&url).send().await?;
        let status = response.status();
        let body: Value = response.json().await?;

        if status.is_success() {
            Ok(body)
        } else {
            Err(anyhow::anyhow!("Readiness check failed with status {}: {}", status, body))
        }
    }

    /// Get metrics in Prometheus format
    pub async fn get_metrics(&self) -> Result<String> {
        let url = format!("{}/metrics", self.base_url);
        let response = self.client.get(&url).send().await?;
        let status = response.status();

        if status.is_success() {
            Ok(response.text().await?)
        } else {
            Err(anyhow::anyhow!("Metrics request failed with status {}", status))
        }
    }

    /// Get SSE authentication info
    pub async fn get_sse_auth_info(&self) -> Result<Value> {
        let url = format!("{}/sse/auth-info", self.base_url);
        let response = self.client.get(&url).send().await?;
        let status = response.status();
        let body: Value = response.json().await?;

        if status.is_success() {
            Ok(body)
        } else {
            Err(anyhow::anyhow!("SSE auth info request failed with status {}: {}", status, body))
        }
    }
}

/// Test assertions and validation helpers
pub mod assertions {
    use super::*;

    /// Assert that a JsonRpcMessage is a successful response
    pub fn assert_success_response(message: &JsonRpcMessage) {
        match message {
            JsonRpcMessage::Response { error, .. } => {
                assert!(error.is_none(), "Expected success response, got error: {:?}", error);
            }
            _ => panic!("Expected response message, got: {:?}", message),
        }
    }

    /// Assert that a JsonRpcMessage is an error response
    pub fn assert_error_response(message: &JsonRpcMessage) {
        match message {
            JsonRpcMessage::Response { error, .. } => {
                assert!(error.is_some(), "Expected error response, got success");
            }
            _ => panic!("Expected response message, got: {:?}", message),
        }
    }

    /// Assert that a tool call result contains expected fields
    pub fn assert_tool_result_structure(result: &Value, expected_fields: &[&str]) {
        for field in expected_fields {
            assert!(
                result.get(field).is_some(),
                "Expected field '{}' not found in result: {}",
                field,
                result
            );
        }
    }

    /// Assert that FHIRPath evaluation result is valid
    pub fn assert_fhirpath_result(result: &Value) {
        assert_tool_result_structure(result, &["values", "types", "performance"]);
        
        let values = result.get("values").unwrap();
        let types = result.get("types").unwrap();
        
        assert!(values.is_array(), "Values should be an array");
        assert!(types.is_array(), "Types should be an array");
        
        let values_len = values.as_array().unwrap().len();
        let types_len = types.as_array().unwrap().len();
        assert_eq!(values_len, types_len, "Values and types arrays should have same length");
    }

    /// Assert that health status is valid
    pub fn assert_health_status_structure(health: &Value) {
        let required_fields = &["status", "timestamp", "uptime_seconds", "version"];
        for field in required_fields {
            assert!(
                health.get(field).is_some(),
                "Health status missing required field: {}",
                field
            );
        }
    }

    /// Assert that metrics are in valid Prometheus format
    pub fn assert_prometheus_metrics_format(metrics: &str) {
        assert!(!metrics.is_empty(), "Metrics should not be empty");
        
        // Check for basic Prometheus metric format
        let lines: Vec<&str> = metrics.lines().collect();
        let mut has_help = false;
        let mut has_type = false;
        let mut has_metric = false;
        
        for line in lines {
            if line.starts_with("# HELP") {
                has_help = true;
            } else if line.starts_with("# TYPE") {
                has_type = true;
            } else if !line.starts_with("#") && !line.trim().is_empty() {
                has_metric = true;
                // Basic metric line format check
                assert!(line.contains(" "), "Metric line should contain space: {}", line);
            }
        }
        
        assert!(has_help, "Metrics should contain HELP comments");
        assert!(has_type, "Metrics should contain TYPE comments");
        assert!(has_metric, "Metrics should contain actual metric values");
    }
}

/// Performance testing utilities
pub mod performance {
    use super::*;
    use std::time::{Duration, Instant};

    /// Measure execution time of an async function
    pub async fn measure_async<F, Fut, T>(f: F) -> (T, Duration)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let start = Instant::now();
        let result = f().await;
        let duration = start.elapsed();
        (result, duration)
    }

    /// Run a function multiple times and collect timing statistics
    pub async fn benchmark_async<F, Fut, T>(
        iterations: usize,
        f: F,
    ) -> BenchmarkResults
    where
        F: Fn() -> Fut + Copy,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut times = Vec::with_capacity(iterations);
        let mut errors = 0;

        for _ in 0..iterations {
            let (result, duration) = measure_async(f).await;
            if result.is_err() {
                errors += 1;
            }
            times.push(duration);
        }

        BenchmarkResults::new(times, errors)
    }

    #[derive(Debug)]
    pub struct BenchmarkResults {
        pub total_iterations: usize,
        pub errors: usize,
        pub min_time: Duration,
        pub max_time: Duration,
        pub average_time: Duration,
        pub median_time: Duration,
        pub p95_time: Duration,
        pub p99_time: Duration,
    }

    impl BenchmarkResults {
        pub fn new(mut times: Vec<Duration>, errors: usize) -> Self {
            times.sort();
            let total_iterations = times.len();
            
            let min_time = times.first().copied().unwrap_or_default();
            let max_time = times.last().copied().unwrap_or_default();
            
            let total_nanos: u128 = times.iter().map(|d| d.as_nanos()).sum();
            let average_time = Duration::from_nanos((total_nanos / total_iterations as u128) as u64);
            
            let median_time = times.get(total_iterations / 2).copied().unwrap_or_default();
            let p95_time = times.get((total_iterations as f64 * 0.95) as usize).copied().unwrap_or_default();
            let p99_time = times.get((total_iterations as f64 * 0.99) as usize).copied().unwrap_or_default();

            Self {
                total_iterations,
                errors,
                min_time,
                max_time,
                average_time,
                median_time,
                p95_time,
                p99_time,
            }
        }

        pub fn success_rate(&self) -> f64 {
            if self.total_iterations == 0 {
                return 0.0;
            }
            (self.total_iterations - self.errors) as f64 / self.total_iterations as f64
        }

        pub fn error_rate(&self) -> f64 {
            1.0 - self.success_rate()
        }
    }
}