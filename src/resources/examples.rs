//! FHIR example resources

/// FHIR example provider (placeholder)
pub struct ExampleProvider;

impl Default for ExampleProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ExampleProvider {
    pub fn new() -> Self {
        Self
    }
}
