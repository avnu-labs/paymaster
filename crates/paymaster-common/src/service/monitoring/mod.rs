use std::collections::HashMap;

use serde::{Deserialize, Serialize};

mod tracer;
pub use tracer::Tracer;

mod metric;
pub use metric::Metric;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Configuration {
    pub endpoint: String,
    pub token: Option<String>,
}

impl Configuration {
    fn headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        if let Some(token) = &self.token {
            headers.insert("Authorization".to_string(), format!("Basic {}", token));
        }

        headers
    }
}
