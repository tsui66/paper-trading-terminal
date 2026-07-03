use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub provider: ProviderConfig,
    pub account: AccountConfig,
    pub trading: TradingConfig,
    pub cache: CacheConfig,
    pub watchlist: WatchlistConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub default: String,
    #[serde(default)]
    pub fallback: Vec<String>,
    #[serde(default)]
    pub fcontext: FcontextConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FcontextConfig {
    #[serde(default = "default_fcontext_cli")]
    pub cli: String,
    #[serde(default = "default_fcontext_timeout")]
    pub timeout_secs: u64,
}

impl Default for FcontextConfig {
    fn default() -> Self {
        Self {
            cli: default_fcontext_cli(),
            timeout_secs: default_fcontext_timeout(),
        }
    }
}

fn default_fcontext_cli() -> String {
    "fcontext".to_string()
}

fn default_fcontext_timeout() -> u64 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountConfig {
    pub initial_cash: f64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfig {
    pub commission_per_trade: f64,
    pub slippage_bps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub enabled: bool,
    pub ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchlistConfig {
    pub symbols: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider: ProviderConfig {
                default: "yahoo".to_string(),
                fallback: vec!["fcontext".into()],
                fcontext: FcontextConfig::default(),
            },
            account: AccountConfig {
                initial_cash: 100_000.0,
                currency: "USD".to_string(),
            },
            trading: TradingConfig {
                commission_per_trade: 0.0,
                slippage_bps: 5.0,
            },
            cache: CacheConfig {
                enabled: true,
                ttl_secs: 60,
            },
            watchlist: WatchlistConfig {
                symbols: vec![
                    "AAPL".into(),
                    "MSFT".into(),
                    "NVDA".into(),
                    "GOOGL".into(),
                    "AMZN".into(),
                ],
            },
        }
    }
}

impl AppConfig {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let path = resolve_config_path(path)?;
        if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("read config {}", path.display()))?;
            toml::from_str(&raw).with_context(|| format!("parse config {}", path.display()))
        } else {
            tracing::warn!("config not found at {}, using defaults", path.display());
            Ok(Self::default())
        }
    }

    pub fn provider_kind(&self) -> crate::provider::ProviderKind {
        crate::provider::ProviderKind::parse(&self.provider.default)
    }

    pub fn provider_chain(&self) -> Vec<crate::provider::ProviderKind> {
        let primary = self.provider_kind();
        let mut chain = vec![primary];
        for fb in &self.provider.fallback {
            let kind = crate::provider::ProviderKind::parse(fb);
            // Mock is offline/dev-only — never auto-fallback to synthetic prices.
            if kind == crate::provider::ProviderKind::Mock && primary != crate::provider::ProviderKind::Mock
            {
                tracing::warn!(
                    "ignoring mock in fallback chain; use `paper config set-provider mock` for offline dev"
                );
                continue;
            }
            if !chain.contains(&kind) {
                chain.push(kind);
            }
        }
        chain
    }

    pub fn config_path(path: Option<&Path>) -> Result<PathBuf> {
        resolve_config_path(path)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(path, raw).with_context(|| format!("write config {}", path.display()))?;
        Ok(())
    }

    pub fn set_default_provider(&mut self, provider: &str) {
        self.provider.default = provider.trim().to_lowercase();
    }

    pub fn set_fallback(&mut self, providers: Vec<String>) {
        self.provider.fallback = providers
            .into_iter()
            .map(|p| p.trim().to_lowercase())
            .filter(|p| !p.is_empty())
            .collect();
    }
}

#[cfg(test)]
mod provider_chain_tests {
    use super::*;

    #[test]
    fn mock_is_excluded_from_fallback_unless_primary() {
        let cfg = AppConfig {
                provider: ProviderConfig {
                    default: "yahoo".into(),
                    fallback: vec!["fcontext".into(), "mock".into()],
                    fcontext: FcontextConfig::default(),
                },
                ..AppConfig::default()
            };
            let chain = cfg.provider_chain();
            assert_eq!(chain.len(), 2);
            assert_eq!(chain[0], crate::provider::ProviderKind::Yahoo);
            assert_eq!(chain[1], crate::provider::ProviderKind::Fcontext);
    }

    #[test]
    fn mock_allowed_when_primary() {
        let cfg = AppConfig {
            provider: ProviderConfig {
                default: "mock".into(),
                fallback: vec![],
                fcontext: FcontextConfig::default(),
            },
            ..AppConfig::default()
        };
        assert_eq!(cfg.provider_chain(), vec![crate::provider::ProviderKind::Mock]);
    }
}

fn resolve_config_path(path: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = path {
        return Ok(p.to_path_buf());
    }
    if let Ok(env) = std::env::var("PAPER_CONFIG").or_else(|_| std::env::var("PPT_CONFIG")) {
        return Ok(PathBuf::from(env));
    }
    Ok(PathBuf::from("config.toml"))
}