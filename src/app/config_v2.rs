//! Enhanced configuration with optimal defaults for visual fidelity.
//!
//! This module provides auto-configuration based on available dependencies
//! and optimal settings for maximum visual fidelity.

use serde::{Deserialize, Serialize};

/// Enhanced configuration with optimal defaults for visual fidelity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedConfig {
    /// Visual diff threshold (adaptive, default 0.005)
    pub visual_diff_threshold: f64,

    /// SSIM floor for structural similarity (default 0.85)
    pub ssim_floor: f64,

    /// Maximum visual retry attempts (default 3)
    pub max_visual_retries: u32,

    /// Enable adaptive mask widening (default true)
    pub adaptive_mask_widening: bool,

    /// PDF rendering DPI for verification (default 300)
    pub verification_dpi: f32,

    /// Edit region DPI for high-fidelity checks (default 600)
    pub edit_region_dpi: f32,

    /// Tile size for localized diff (default 12px)
    pub tile_size: u32,

    /// Whether to use cloud APIs when available
    pub use_cloud_apis: bool,

    /// Timeout for cloud API calls in seconds
    pub cloud_api_timeout_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyType {
    PdfRest,    // Cloud rendering
    Applitools, // Visual AI
    Mindee,     // OCR
    DocAI,      // Google Document AI
    LlamaParse, // LLM parsing
    PyMuPDF,    // Local Python
    Pdfium,     // Local native
}

impl Default for EnhancedConfig {
    fn default() -> Self {
        Self {
            visual_diff_threshold: 0.005,
            ssim_floor: 0.85,
            max_visual_retries: 3,
            adaptive_mask_widening: true,
            verification_dpi: 300.0,
            edit_region_dpi: 600.0,
            tile_size: 12,
            use_cloud_apis: true,
            cloud_api_timeout_secs: 30,
        }
    }
}

impl EnhancedConfig {
    /// Create the optimal configuration for maximum visual fidelity.
    pub fn maximum_fidelity() -> Self {
        Self {
            visual_diff_threshold: 0.001, // Extremely tight
            ssim_floor: 0.95,             // Near-perfect SSIM
            max_visual_retries: 5,
            adaptive_mask_widening: true,
            verification_dpi: 600.0, // High DPI for verification
            edit_region_dpi: 1200.0, // Ultra-high for edit regions
            tile_size: 8,            // Very small tiles
            ..Default::default()
        }
    }

    /// Auto-configure based on available dependencies.
    pub fn auto_configure() -> Self {
        let mut config = Self::default();

        // Check which dependencies are available
        let has_pdfrest = std::env::var("PDFREST_API_KEY").is_ok();
        let has_applitools = std::env::var("APPLITOOLS_API_KEY").is_ok();
        let has_docai = std::env::var("DOCUMENT_AI_PROJECT_ID").is_ok();
        let _has_llamaparse = std::env::var("LLAMAPARSE_API_KEY").is_ok();

        // Adjust configuration based on available dependencies
        config.use_cloud_apis = has_pdfrest || has_applitools || has_docai;

        // If no cloud APIs, tighten local thresholds
        if !config.use_cloud_apis {
            config.visual_diff_threshold = 0.01; // Slightly relaxed for local-only
            config.ssim_floor = 0.75; // Relaxed SSIM for local
        }

        config
    }
}

/// Dependency health monitor that tracks API availability and response times.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealthStatus {
    pub is_available: bool,
    pub last_check: String, // ISO 8601 timestamp
    pub avg_response_time_ms: u64,
    pub consecutive_failures: u32,
    pub last_error: Option<String>,
}

impl HealthStatus {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = EnhancedConfig::default();
        assert_eq!(config.visual_diff_threshold, 0.005);
        assert_eq!(config.ssim_floor, 0.85);
        assert_eq!(config.tile_size, 12);
    }

    #[test]
    fn test_maximum_fidelity_config() {
        let config = EnhancedConfig::maximum_fidelity();
        assert_eq!(config.visual_diff_threshold, 0.001);
        assert_eq!(config.ssim_floor, 0.95);
        assert_eq!(config.tile_size, 8);
    }

    #[test]
    fn test_auto_configure() {
        let config = EnhancedConfig::auto_configure();
        // Should have reasonable defaults
        assert!(config.visual_diff_threshold > 0.0);
        assert!(config.ssim_floor > 0.0);
    }
}
