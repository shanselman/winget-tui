use anyhow::Result;
use async_trait::async_trait;

use crate::models::{Package, PackageDetail, Source};

#[async_trait]
pub trait WingetBackend: Send + Sync {
    /// Search for packages matching query, optionally filtered by source
    async fn search(&self, query: &str, source: Option<&str>) -> Result<Vec<Package>>;

    /// List all installed packages, optionally filtered by source
    async fn list_installed(&self, source: Option<&str>) -> Result<Vec<Package>>;

    /// List packages with available upgrades
    async fn list_upgrades(&self) -> Result<Vec<Package>>;

    /// Show detailed info for a specific package
    async fn show(&self, id: &str) -> Result<PackageDetail>;

    /// Install a package by id, optionally a specific version
    async fn install(&self, id: &str, version: Option<&str>) -> Result<String>;

    /// Uninstall a package by id
    async fn uninstall(&self, id: &str) -> Result<String>;

    /// Upgrade a package by id
    async fn upgrade(&self, id: &str) -> Result<String>;

    /// List configured package sources
    #[allow(dead_code)]
    async fn list_sources(&self) -> Result<Vec<Source>>;
}
