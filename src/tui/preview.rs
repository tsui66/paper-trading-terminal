use super::app::App;
use anyhow::Result;
use std::path::Path;

/// Render a deterministic TUI frame to PNG (for docs / README).
pub async fn capture_screenshot(path: impl AsRef<Path>) -> Result<()> {
    App::capture_screenshot(path).await
}
