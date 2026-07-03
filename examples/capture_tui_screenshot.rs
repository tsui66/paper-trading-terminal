use anyhow::Result;
use paper_trading_terminal::tui::preview::capture_screenshot;

#[tokio::main]
async fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/tui-screenshot.png".into());
    capture_screenshot(&path).await?;
    println!("wrote {path}");
    Ok(())
}
