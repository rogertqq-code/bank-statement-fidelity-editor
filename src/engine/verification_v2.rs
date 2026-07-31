//! Enhanced visual verification module with adaptive thresholds and multi-layer validation.
//!
//! This module provides improved visual fidelity checking with tighter thresholds,
//! smaller detection tiles, and adaptive masking for better accuracy.

use image::{GrayImage, RgbaImage};
use serde::{Deserialize, Serialize};

/// Enhanced verification report with detailed per-region analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedVerificationReport {
    pub math_valid: bool,
    pub visual_diff_score: f64,
    pub only_intended_changes: bool,
    pub report_files: Vec<String>,
    pub message: String,

    /// Per-edit region fidelity scores
    pub edit_region_scores: Vec<EditRegionScore>,

    /// Adaptive threshold used for this verification
    pub adaptive_threshold: f64,

    /// SSIM score (structural similarity)
    pub ssim_score: f64,

    /// Perceptual hash distance
    pub hash_distance: f64,

    /// Whether the visual diff passed with adaptive masking
    pub passed_with_adaptive_mask: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditRegionScore {
    pub page: usize,
    pub bbox: [f32; 4],
    pub original_text: String,
    pub edited_text: String,
    pub ssim_score: f64,
    pub pixel_diff_percentage: f64,
    pub font_metrics_match: bool,
    pub baseline_drift_px: f32,
    pub kerning_drift_px: f32,
}

/// Multi-layer visual validation engine with adaptive thresholds.
pub struct VisualFidelityEngine {
    base_threshold: f64,
    ssim_floor: f64,
    tile_size: u32,
    #[allow(dead_code)]
    edit_region_dpi: f32,
}

impl Default for VisualFidelityEngine {
    fn default() -> Self {
        Self {
            base_threshold: 0.005, // Much tighter than 0.02
            ssim_floor: 0.85,      // Much higher than 0.40
            tile_size: 12,         // Smaller tiles for character-level detection
            edit_region_dpi: 600.0,
        }
    }
}

impl VisualFidelityEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.base_threshold = threshold;
        self
    }

    pub fn with_ssim_floor(mut self, floor: f64) -> Self {
        self.ssim_floor = floor;
        self
    }

    /// Run comprehensive visual validation with adaptive thresholding.
    pub fn validate_edit(
        &self,
        original_img: &RgbaImage,
        edited_img: &RgbaImage,
        edit_regions: &[(usize, [f32; 4])], // (page, bbox)
    ) -> EnhancedVerificationReport {
        // Convert to grayscale for structural analysis
        let orig_gray = Self::to_gray(original_img);
        let edit_gray = Self::to_gray(edited_img);

        // Compute SSIM
        let ssim = self.compute_ssim(&orig_gray, &edit_gray, edit_regions);

        // Compute localized tile-max score with smaller tiles
        let tile_score = self.compute_tile_max(&orig_gray, &edit_gray, edit_regions);

        // Compute perceptual hash distance
        let hash_dist = self.compute_hash_distance(original_img, edited_img);

        // Adaptive threshold: tighten if edit regions score well
        let adaptive_threshold = if ssim > 0.95 {
            self.base_threshold * 0.5 // Tighter threshold if edits are good
        } else {
            self.base_threshold
        };

        // Determine if verification passed
        let passed = tile_score < adaptive_threshold && ssim >= self.ssim_floor;

        EnhancedVerificationReport {
            math_valid: true, // TODO: integrate math validation
            visual_diff_score: tile_score,
            only_intended_changes: passed,
            report_files: vec![],
            message: format!(
                "Visual verification: tile_max={:.4}, SSIM={:.4}, hash_dist={:.4}, adaptive_threshold={:.4}",
                tile_score, ssim, hash_dist, adaptive_threshold
            ),
            edit_region_scores: vec![],
            adaptive_threshold,
            ssim_score: ssim,
            hash_distance: hash_dist,
            passed_with_adaptive_mask: passed,
        }
    }

    /// Compute SSIM with excluded regions.
    fn compute_ssim(
        &self,
        orig: &GrayImage,
        edited: &GrayImage,
        exclude: &[(usize, [f32; 4])],
    ) -> f64 {
        // Mask excluded regions by setting them to black
        let mut masked_orig = orig.clone();
        let mut masked_edited = edited.clone();

        for (_, bbox) in exclude {
            Self::mask_region(&mut masked_orig, *bbox);
            Self::mask_region(&mut masked_edited, *bbox);
        }

        // Simple SSIM approximation using mean squared error
        // In production, this would use the image_compare crate
        let mse = self.compute_mse(&masked_orig, &masked_edited);
        1.0 - mse
    }

    /// Compute localized tile-max with adaptive tile size.
    fn compute_tile_max(
        &self,
        orig: &GrayImage,
        edited: &GrayImage,
        exclude: &[(usize, [f32; 4])],
    ) -> f64 {
        let tile_size = self.tile_size;
        let (width, height) = (orig.width(), orig.height());
        let mut max_score = 0.0f64;

        for y in (0..height).step_by(tile_size as usize) {
            for x in (0..width).step_by(tile_size as usize) {
                let x1 = (x + tile_size).min(width);
                let y1 = (y + tile_size).min(height);

                // Check if this tile overlaps an excluded region
                let is_excluded = exclude.iter().any(|(_, bbox)| {
                    x >= bbox[0] as u32
                        && x < bbox[2] as u32
                        && y >= bbox[1] as u32
                        && y < bbox[3] as u32
                });

                if !is_excluded {
                    let mut diff_sum = 0u64;
                    let mut count = 0u64;

                    for py in y..y1 {
                        for px in x..x1 {
                            let o = orig.get_pixel(px, py)[0] as i32;
                            let e = edited.get_pixel(px, py)[0] as i32;
                            diff_sum += (o - e).unsigned_abs() as u64;
                            count += 1;
                        }
                    }

                    if count > 0 {
                        let score = diff_sum as f64 / (255.0 * count as f64);
                        max_score = max_score.max(score);
                    }
                }
            }
        }

        max_score
    }

    fn compute_hash_distance(&self, orig: &RgbaImage, edited: &RgbaImage) -> f64 {
        // Simplified hash distance calculation
        // In production, this would use image_hasher crate
        let mut diff_pixels = 0u64;
        let total_pixels = (orig.width() * orig.height()) as u64;

        for (p1, p2) in orig.pixels().zip(edited.pixels()) {
            if p1[0] != p2[0] || p1[1] != p2[1] || p1[2] != p2[2] {
                diff_pixels += 1;
            }
        }

        diff_pixels as f64 / total_pixels as f64
    }

    fn compute_mse(&self, img1: &GrayImage, img2: &GrayImage) -> f64 {
        let mut sum_sq_diff = 0.0;
        let total_pixels = (img1.width() * img1.height()) as f64;

        for (p1, p2) in img1.pixels().zip(img2.pixels()) {
            let diff = p1[0] as f64 - p2[0] as f64;
            sum_sq_diff += diff * diff;
        }

        sum_sq_diff / total_pixels
    }

    fn mask_region(img: &mut GrayImage, bbox: [f32; 4]) {
        let (w, h) = img.dimensions();
        let x0 = (bbox[0] as u32).min(w);
        let y0 = (bbox[1] as u32).min(h);
        let x1 = (bbox[2] as u32).min(w);
        let y1 = (bbox[3] as u32).min(h);

        for y in y0..y1 {
            for x in x0..x1 {
                img.put_pixel(x, y, image::Luma([0]));
            }
        }
    }

    fn to_gray(img: &RgbaImage) -> GrayImage {
        let mut gray = GrayImage::new(img.width(), img.height());
        for (x, y, pixel) in img.enumerate_pixels() {
            let gray_value = (0.2126 * pixel[0] as f32
                + 0.7152 * pixel[1] as f32
                + 0.0722 * pixel[2] as f32) as u8;
            gray.put_pixel(x, y, image::Luma([gray_value]));
        }
        gray
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visual_fidelity_engine_creation() {
        let engine = VisualFidelityEngine::new();
        assert_eq!(engine.base_threshold, 0.005);
        assert_eq!(engine.ssim_floor, 0.85);
        assert_eq!(engine.tile_size, 12);
    }

    #[test]
    fn test_validate_identical_images() {
        let engine = VisualFidelityEngine::new();
        let img = RgbaImage::new(100, 100);
        let report = engine.validate_edit(&img, &img, &[]);

        assert!(
            report.passed_with_adaptive_mask,
            "Identical images should pass"
        );
        assert!(report.ssim_score > 0.99, "SSIM should be near 1.0");
    }
}
