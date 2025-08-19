//! MCP tools implementation
//!
//! Tools provide the core functionality exposed through the MCP protocol:
//! - fhirpath_evaluate: Evaluate FHIRPath expressions against resources
//! - fhirpath_parse: Parse and validate FHIRPath expressions
//! - fhirpath_extract: Extract data from FHIR resources using FHIRPath
//! - fhirpath_explain: Explain FHIRPath expressions and their evaluation

pub mod fhirpath;
pub mod fhirpath_evaluate;
pub mod fhirpath_parse;

pub use fhirpath_evaluate::FhirPathEvaluateTool;
pub use fhirpath_parse::FhirPathParseTool;
