use super::ProviderError;
use serde_json::Value;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Clone)]
pub struct CliRunner {
    program: String,
    timeout: Duration,
}

impl CliRunner {
    pub fn new(program: impl Into<String>, timeout_secs: u64) -> Self {
        Self {
            program: program.into(),
            timeout: Duration::from_secs(timeout_secs.max(1)),
        }
    }

    pub fn program(&self) -> &str {
        &self.program
    }

    pub async fn run_json(&self, args: &[&str]) -> Result<Value, ProviderError> {
        let output = timeout(self.timeout, self.spawn(args))
            .await
            .map_err(|_| {
                ProviderError::Network(format!(
                    "{} timed out after {}s",
                    self.program,
                    self.timeout.as_secs()
                ))
            })??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let message = if stderr.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                stderr.trim().to_string()
            };
            return Err(map_cli_error(&message));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(stdout.trim()).map_err(|e| {
            ProviderError::Other(anyhow::anyhow!("invalid JSON from {}: {e}", self.program))
        })
    }

    async fn spawn(&self, args: &[&str]) -> Result<std::process::Output, ProviderError> {
        Command::new(&self.program)
            .args(args)
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ProviderError::Unavailable(format!(
                        "CLI not found: {} (install or set provider.fcontext.cli)",
                        self.program
                    ))
                } else {
                    ProviderError::Other(e.into())
                }
            })
    }
}

fn map_cli_error(message: &str) -> ProviderError {
    let lower = message.to_lowercase();
    if lower.contains("command not found") || lower.contains("no such file") {
        ProviderError::Unavailable(message.to_string())
    } else if lower.contains("401") || lower.contains("unauthorized") {
        ProviderError::Unavailable(format!("fcontext auth required: {message}"))
    } else if lower.contains("402") || lower.contains("subscription") {
        ProviderError::Unavailable(format!("fcontext subscription required: {message}"))
    } else if lower.contains("403") || lower.contains("permission") {
        ProviderError::Unavailable(format!("fcontext permission denied: {message}"))
    } else if lower.contains("404") || lower.contains("not found") {
        ProviderError::NotFound(message.to_string())
    } else {
        ProviderError::Network(message.to_string())
    }
}