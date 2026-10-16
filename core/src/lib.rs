pub mod git;

use serde::{Deserialize, Serialize};

/// Request to greet a person by name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreetRequest {
    pub name: String,
}

/// Response containing a greeting message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreetResponse {
    pub message: String,
}

/// Generate a greeting for the given name.
pub fn greet(request: GreetRequest) -> GreetResponse {
    GreetResponse {
        message: format!("Hello, {}! You've been greeted from Rust!", request.name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet_returns_message() {
        let response = greet(GreetRequest {
            name: "World".to_string(),
        });
        assert_eq!(
            response.message,
            "Hello, World! You've been greeted from Rust!"
        );
    }
}
