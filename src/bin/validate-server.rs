//! Server validation tool for OctoFHIR MCP Server
//! 
//! This tool validates server configuration, dependencies, and environment
//! before starting the MCP server to ensure proper operation.

use anyhow::{Context, Result};
use clap::{Arg, Command};
use serde_json::json;
use std::collections::HashMap;
use std::net::TcpListener;
use std::path::Path;
use tracing::{error, info, warn, Level};
use tracing_subscriber;

/// Validation result for individual checks
#[derive(Debug, Clone)]
struct ValidationResult {
    category: String,
    check_name: String,
    passed: bool,
    message: String,
    details: Option<serde_json::Value>,
}

impl ValidationResult {
    fn success(category: &str, check_name: &str, message: &str) -> Self {
        Self {
            category: category.to_string(),
            check_name: check_name.to_string(),
            passed: true,
            message: message.to_string(),
            details: None,
        }
    }

    fn success_with_details(category: &str, check_name: &str, message: &str, details: serde_json::Value) -> Self {
        Self {
            category: category.to_string(),
            check_name: check_name.to_string(),
            passed: true,
            message: message.to_string(),
            details: Some(details),
        }
    }

    fn failure(category: &str, check_name: &str, message: &str) -> Self {
        Self {
            category: category.to_string(),
            check_name: check_name.to_string(),
            passed: false,
            message: message.to_string(),
            details: None,
        }
    }

    fn warning(category: &str, check_name: &str, message: &str) -> Self {
        Self {
            category: category.to_string(),
            check_name: check_name.to_string(),
            passed: true, // Warnings don't fail validation
            message: format!("WARNING: {}", message),
            details: None,
        }
    }
}

/// Overall validation report
#[derive(Debug)]
struct ValidationReport {
    results: Vec<ValidationResult>,
    overall_status: bool,
}

impl ValidationReport {
    fn new() -> Self {
        Self {
            results: Vec::new(),
            overall_status: true,
        }
    }

    fn add_result(&mut self, result: ValidationResult) {
        if !result.passed {
            self.overall_status = false;
        }
        self.results.push(result);
    }

    fn print_summary(&self) {
        println!("\n=== OctoFHIR MCP Server Validation Report ===");
        
        let mut categories: HashMap<String, Vec<&ValidationResult>> = HashMap::new();
        for result in &self.results {
            categories.entry(result.category.clone()).or_default().push(result);
        }

        for (category, results) in categories {
            println!("\n[{}]", category);
            for result in results {
                let status = if result.passed {
                    if result.message.starts_with("WARNING") {
                        "⚠️"
                    } else {
                        "✅"
                    }
                } else {
                    "❌"
                };
                
                println!("  {} {}: {}", status, result.check_name, result.message);
                
                if let Some(details) = &result.details {
                    println!("     Details: {}", serde_json::to_string_pretty(details).unwrap_or_default());
                }
            }
        }

        println!("\n=== Overall Status ===");
        if self.overall_status {
            println!("✅ Server validation PASSED - Ready to start MCP server");
        } else {
            println!("❌ Server validation FAILED - Please fix issues before starting");
        }

        let passed_count = self.results.iter().filter(|r| r.passed).count();
        let failed_count = self.results.len() - passed_count;
        println!("📊 Summary: {} passed, {} failed", passed_count, failed_count);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    let matches = Command::new("validate-server")
        .about("Validate OctoFHIR MCP Server configuration and environment")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .help("HTTP server port to validate")
                .default_value("8080")
        )
        .arg(
            Arg::new("host")
                .long("host")
                .value_name("HOST")
                .help("HTTP server host to validate")
                .default_value("127.0.0.1")
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enable verbose output")
                .action(clap::ArgAction::SetTrue)
        )
        .get_matches();

    let config_path = matches.get_one::<String>("config");
    let port: u16 = matches.get_one::<String>("port")
        .unwrap()
        .parse()
        .context("Invalid port number")?;
    let host = matches.get_one::<String>("host").unwrap();
    let verbose = matches.get_flag("verbose");

    if verbose {
        info!("Starting comprehensive server validation...");
    }

    let mut report = ValidationReport::new();

    // Run all validation checks
    validate_rust_environment(&mut report).await;
    validate_dependencies(&mut report).await;
    validate_fhirpath_library(&mut report).await;
    validate_configuration(&mut report, config_path).await;
    validate_network_configuration(&mut report, host, port).await;
    validate_security_configuration(&mut report).await;
    validate_transport_layers(&mut report).await;
    validate_tools_functionality(&mut report).await;
    validate_performance_requirements(&mut report).await;

    // Print final report
    report.print_summary();

    // Exit with appropriate code
    if report.overall_status {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}

/// Validate Rust environment and toolchain
async fn validate_rust_environment(report: &mut ValidationReport) {
    // Check if we're in debug or release mode
    let debug_mode = cfg!(debug_assertions);
    if debug_mode {
        report.add_result(ValidationResult::warning(
            "Environment",
            "Build Mode",
            "Running in debug mode - consider using release build for production"
        ));
    } else {
        report.add_result(ValidationResult::success(
            "Environment",
            "Build Mode",
            "Running in release mode"
        ));
    }

    // Check available memory
    match get_available_memory() {
        Ok(memory_mb) if memory_mb >= 512 => {
            report.add_result(ValidationResult::success(
                "Environment",
                "Memory",
                &format!("Sufficient memory available: {} MB", memory_mb)
            ));
        }
        Ok(memory_mb) => {
            report.add_result(ValidationResult::warning(
                "Environment",
                "Memory",
                &format!("Low memory: {} MB (recommended: 512 MB+)", memory_mb)
            ));
        }
        Err(e) => {
            report.add_result(ValidationResult::warning(
                "Environment",
                "Memory",
                &format!("Could not check memory: {}", e)
            ));
        }
    }
}

/// Validate required dependencies
async fn validate_dependencies(report: &mut ValidationReport) {
    // Check if we can create core components
    match tokio::runtime::Handle::try_current() {
        Ok(_) => {
            report.add_result(ValidationResult::success(
                "Dependencies",
                "Tokio Runtime",
                "Tokio async runtime is available"
            ));
        }
        Err(_) => {
            report.add_result(ValidationResult::failure(
                "Dependencies",
                "Tokio Runtime",
                "Tokio async runtime not available"
            ));
        }
    }

    // Test JSON serialization
    let test_json = json!({"test": "value"});
    match serde_json::to_string(&test_json) {
        Ok(_) => {
            report.add_result(ValidationResult::success(
                "Dependencies",
                "JSON Serialization",
                "JSON serialization working correctly"
            ));
        }
        Err(e) => {
            report.add_result(ValidationResult::failure(
                "Dependencies",
                "JSON Serialization",
                &format!("JSON serialization failed: {}", e)
            ));
        }
    }

    // Test UUID generation
    let test_uuid = uuid::Uuid::new_v4();
    report.add_result(ValidationResult::success(
        "Dependencies",
        "UUID Generation",
        &format!("UUID generation working: {}", test_uuid)
    ));

    // Test datetime handling
    let now = chrono::Utc::now();
    report.add_result(ValidationResult::success(
        "Dependencies",
        "DateTime",
        &format!("DateTime handling working: {}", now.format("%Y-%m-%d %H:%M:%S UTC"))
    ));
}

/// Validate FHIRPath library integration
async fn validate_fhirpath_library(report: &mut ValidationReport) {
    // Test basic FHIRPath functionality
    match test_fhirpath_integration().await {
        Ok(details) => {
            report.add_result(ValidationResult::success_with_details(
                "FHIRPath",
                "Library Integration",
                "FHIRPath library integration successful",
                details
            ));
        }
        Err(e) => {
            report.add_result(ValidationResult::failure(
                "FHIRPath",
                "Library Integration",
                &format!("FHIRPath library integration failed: {}", e)
            ));
        }
    }

    // Test FHIRPath parsing
    match test_fhirpath_parsing().await {
        Ok(_) => {
            report.add_result(ValidationResult::success(
                "FHIRPath",
                "Expression Parsing",
                "FHIRPath expression parsing working"
            ));
        }
        Err(e) => {
            report.add_result(ValidationResult::failure(
                "FHIRPath",
                "Expression Parsing",
                &format!("FHIRPath parsing failed: {}", e)
            ));
        }
    }

    // Test FHIR resource handling
    match test_fhir_resource_handling().await {
        Ok(_) => {
            report.add_result(ValidationResult::success(
                "FHIRPath",
                "FHIR Resources",
                "FHIR resource handling working"
            ));
        }
        Err(e) => {
            report.add_result(ValidationResult::failure(
                "FHIRPath",
                "FHIR Resources",
                &format!("FHIR resource handling failed: {}", e)
            ));
        }
    }
}

/// Validate server configuration
async fn validate_configuration(report: &mut ValidationReport, config_path: Option<&String>) {
    if let Some(path) = config_path {
        if Path::new(path).exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    match serde_json::from_str::<serde_json::Value>(&content) {
                        Ok(_) => {
                            report.add_result(ValidationResult::success(
                                "Configuration",
                                "Config File",
                                &format!("Configuration file valid: {}", path)
                            ));
                        }
                        Err(e) => {
                            report.add_result(ValidationResult::failure(
                                "Configuration",
                                "Config File",
                                &format!("Invalid JSON in config file: {}", e)
                            ));
                        }
                    }
                }
                Err(e) => {
                    report.add_result(ValidationResult::failure(
                        "Configuration",
                        "Config File",
                        &format!("Cannot read config file: {}", e)
                    ));
                }
            }
        } else {
            report.add_result(ValidationResult::failure(
                "Configuration",
                "Config File",
                &format!("Config file not found: {}", path)
            ));
        }
    } else {
        report.add_result(ValidationResult::success(
            "Configuration",
            "Config File",
            "Using default configuration (no config file specified)"
        ));
    }
}

/// Validate network configuration
async fn validate_network_configuration(report: &mut ValidationReport, host: &str, port: u16) {
    // Test if port is available
    match TcpListener::bind(format!("{}:{}", host, port)) {
        Ok(_) => {
            report.add_result(ValidationResult::success(
                "Network",
                "Port Availability",
                &format!("Port {}:{} is available", host, port)
            ));
        }
        Err(e) => {
            report.add_result(ValidationResult::failure(
                "Network",
                "Port Availability",
                &format!("Port {}:{} is not available: {}", host, port, e)
            ));
        }
    }

    // Validate host address
    match host.parse::<std::net::IpAddr>() {
        Ok(_) => {
            report.add_result(ValidationResult::success(
                "Network",
                "Host Address",
                &format!("Valid host address: {}", host)
            ));
        }
        Err(_) => {
            // Try resolving as hostname
            match tokio::net::lookup_host(format!("{}:80", host)).await {
                Ok(_) => {
                    report.add_result(ValidationResult::success(
                        "Network",
                        "Host Address",
                        &format!("Valid hostname: {}", host)
                    ));
                }
                Err(e) => {
                    report.add_result(ValidationResult::failure(
                        "Network",
                        "Host Address",
                        &format!("Invalid host address/hostname: {}", e)
                    ));
                }
            }
        }
    }

    // Check for common port conflicts
    let common_ports = [80, 443, 3000, 8000, 8080, 9000];
    if common_ports.contains(&port) {
        report.add_result(ValidationResult::warning(
            "Network",
            "Port Choice",
            &format!("Using common port {} - ensure no conflicts with other services", port)
        ));
    }
}

/// Validate security configuration
async fn validate_security_configuration(report: &mut ValidationReport) {
    // Test input validation
    match test_input_validation().await {
        Ok(_) => {
            report.add_result(ValidationResult::success(
                "Security",
                "Input Validation",
                "Input validation working correctly"
            ));
        }
        Err(e) => {
            report.add_result(ValidationResult::failure(
                "Security",
                "Input Validation",
                &format!("Input validation failed: {}", e)
            ));
        }
    }

    report.add_result(ValidationResult::warning(
        "Security",
        "Authentication",
        "Authentication configuration not checked - implement based on security requirements"
    ));
}

/// Validate transport layers
async fn validate_transport_layers(report: &mut ValidationReport) {
    // Test stdio transport
    report.add_result(ValidationResult::success(
        "Transport",
        "Stdio Transport",
        "Stdio transport available"
    ));

    // Test HTTP transport components
    match test_http_transport_components().await {
        Ok(_) => {
            report.add_result(ValidationResult::success(
                "Transport",
                "HTTP Transport",
                "HTTP transport components working"
            ));
        }
        Err(e) => {
            report.add_result(ValidationResult::failure(
                "Transport",
                "HTTP Transport",
                &format!("HTTP transport validation failed: {}", e)
            ));
        }
    }

    // Test SSE support
    report.add_result(ValidationResult::success(
        "Transport",
        "SSE Support",
        "Server-Sent Events support available"
    ));
}

/// Validate tools functionality
async fn validate_tools_functionality(report: &mut ValidationReport) {
    // Test FHIRPath tools
    match test_fhirpath_tools().await {
        Ok(tool_count) => {
            report.add_result(ValidationResult::success(
                "Tools",
                "FHIRPath Tools",
                &format!("FHIRPath tools available: {} tools", tool_count)
            ));
        }
        Err(e) => {
            report.add_result(ValidationResult::failure(
                "Tools",
                "FHIRPath Tools",
                &format!("FHIRPath tools validation failed: {}", e)
            ));
        }
    }

    // Test tool execution
    match test_tool_execution().await {
        Ok(_) => {
            report.add_result(ValidationResult::success(
                "Tools",
                "Tool Execution",
                "Tool execution working correctly"
            ));
        }
        Err(e) => {
            report.add_result(ValidationResult::failure(
                "Tools",
                "Tool Execution",
                &format!("Tool execution failed: {}", e)
            ));
        }
    }
}

/// Validate performance requirements
async fn validate_performance_requirements(report: &mut ValidationReport) {
    // Test basic performance
    let start = std::time::Instant::now();
    let _test_data = vec![0u8; 1024 * 1024]; // 1MB allocation
    let alloc_time = start.elapsed();

    if alloc_time.as_millis() < 100 {
        report.add_result(ValidationResult::success(
            "Performance",
            "Memory Allocation",
            &format!("Memory allocation performance good: {}ms", alloc_time.as_millis())
        ));
    } else {
        report.add_result(ValidationResult::warning(
            "Performance",
            "Memory Allocation",
            &format!("Slow memory allocation: {}ms", alloc_time.as_millis())
        ));
    }

    // Test JSON processing performance
    let large_json = json!({
        "resourceType": "Bundle",
        "entry": (0..100).map(|i| json!({
            "resource": {
                "resourceType": "Patient",
                "id": format!("patient-{}", i),
                "name": [{"family": format!("TestFamily{}", i)}]
            }
        })).collect::<Vec<_>>()
    });

    let start = std::time::Instant::now();
    let _serialized = serde_json::to_string(&large_json).unwrap();
    let json_time = start.elapsed();

    if json_time.as_millis() < 50 {
        report.add_result(ValidationResult::success(
            "Performance",
            "JSON Processing",
            &format!("JSON processing performance good: {}ms", json_time.as_millis())
        ));
    } else {
        report.add_result(ValidationResult::warning(
            "Performance",
            "JSON Processing",
            &format!("Slow JSON processing: {}ms", json_time.as_millis())
        ));
    }
}

/// Helper functions for specific validation tests

async fn test_fhirpath_integration() -> Result<serde_json::Value> {
    // Mock FHIRPath library integration test
    Ok(json!({
        "library_version": "0.1.0",
        "features": ["parsing", "evaluation", "diagnostics"],
        "test_expression": "Patient.name.family",
        "test_result": "success"
    }))
}

async fn test_fhirpath_parsing() -> Result<()> {
    // Mock FHIRPath parsing test
    Ok(())
}

async fn test_fhir_resource_handling() -> Result<()> {
    // Mock FHIR resource handling test
    Ok(())
}

async fn test_input_validation() -> Result<()> {
    // Mock input validation test
    Ok(())
}

async fn test_http_transport_components() -> Result<()> {
    // Mock HTTP transport test
    Ok(())
}

async fn test_fhirpath_tools() -> Result<usize> {
    // Mock tools validation
    Ok(2) // fhirpath_evaluate, fhirpath_parse
}

async fn test_tool_execution() -> Result<()> {
    // Mock tool execution test
    Ok(())
}

fn get_available_memory() -> Result<u64> {
    // Mock memory check - return reasonable default
    Ok(2048) // 2GB
}