//! Native PDF Engine - oxidize-pdf AST traversal + pdf-writer serialization.
//!
//! Phase 2 of the architecture rewrite. This replaces all FFI-based PDF
//! engines (MuPDF, PyMuPDF, pdfium-render) with pure Rust implementations.
//!
//! ## Design
//!
//! - **Read path:** Uses `oxidize_pdf::Document` for non-destructive PDF
//!   parsing. Content streams are walked operator-by-operator to extract
//!   text blocks with their positions, fonts, and sizes.
//!
//! - **Write path:** Uses `lopdf` (already in the dep tree) for surgical
//!   content stream edits. `pdf-writer` is used for full-page serialization
//!   when needed.
//!
//! - **Rendering:** Fallback native renderer drawing bounding boxes using `imageproc`.

use crate::engine::layout::{DocumentLayout, PageLayout};
use crate::pdf::engine::*;
use std::path::Path;

/// Pure-Rust PDF engine backed by `oxidize-pdf` + `lopdf`.
#[derive(Debug, Default)]
pub struct OxidizePdfEngine;

impl OxidizePdfEngine {
    pub fn new() -> Self {
        Self
    }

    /// Load a PDF document via lopdf (which is already a dependency) and
    /// count pages.
    fn page_count(&self, path: &Path) -> Result<usize, EngineError> {
        let doc =
            lopdf::Document::load(path).map_err(|e| EngineError::LoadFailed(format!("{e}")))?;
        Ok(doc.get_pages().len())
    }

    /// Extract text blocks from a single page by walking the content stream.
    ///
    /// This parses the raw PDF operators (Tj, TJ, Tm, Tf, Td, TD, T*, etc.)
    /// to reconstruct positioned text spans. Each span becomes a `TextBlock`
    /// with its bounding box estimated from the text matrix and font metrics.
    fn extract_text_blocks_from_page(
        &self,
        path: &Path,
        page_num: usize,
    ) -> Result<Vec<TextBlock>, EngineError> {
        let document = lopdf::Document::load(path)
            .map_err(|error| EngineError::LoadFailed(format!("{error}")))?;
        let pages = document.get_pages();
        let page_id = *pages.get(&(page_num as u32 + 1)).ok_or_else(|| {
            EngineError::ExtractFailed(format!(
                "Page {page_num} not found (document has {} pages)",
                pages.len()
            ))
        })?;
        let page_box = effective_page_box(&document, page_id)?;
        let content = document.get_page_content(page_id).map_err(|error| {
            EngineError::ExtractFailed(format!("Failed to get page content: {error}"))
        })?;
        let operations = lopdf::content::Content::decode(&content)
            .map_err(|error| {
                EngineError::ExtractFailed(format!("Failed to decode content stream: {error}"))
            })?
            .operations;
        let rotation = inherited_page_rotation(&document, page_id);
        let targets =
            collect_native_text_targets(&operations, page_box, rotation).map_err(|error| {
                EngineError::ExtractFailed(format!("Failed to map positioned text: {error}"))
            })?;
        Ok(targets
            .into_iter()
            .map(|target| TextBlock {
                page: page_num,
                text: target.text,
                bbox: target.bbox,
                font: target.font,
                size: target.size,
                obj_id: Some(format!(
                    "ObjId({}, {}):op{}",
                    page_id.0, page_id.1, target.operation_index
                )),
            })
            .collect())
    }
}

/// Helper: extract f32 from a lopdf Object (Integer or Real).
fn operand_to_f32(obj: &lopdf::Object) -> Option<f32> {
    match obj {
        lopdf::Object::Integer(i) => Some(*i as f32),
        lopdf::Object::Real(f) => Some(*f),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
struct CanonicalPageBox {
    x_min: f32,
    y_min: f32,
    x_max: f32,
    y_max: f32,
}

impl CanonicalPageBox {
    fn width(self) -> f32 {
        self.x_max - self.x_min
    }

    fn height(self) -> f32 {
        self.y_max - self.y_min
    }

    fn content_point_to_top_left(
        self,
        x: f32,
        y: f32,
        rotation: i32,
    ) -> Result<(f32, f32), EngineError> {
        let unrotated_x = x - self.x_min;
        let unrotated_y = self.y_max - y;
        match rotation.rem_euclid(360) {
            0 => Ok((unrotated_x, unrotated_y)),
            90 => Ok((self.height() - unrotated_y, unrotated_x)),
            180 => Ok((self.width() - unrotated_x, self.height() - unrotated_y)),
            270 => Ok((unrotated_y, self.width() - unrotated_x)),
            value => Err(EngineError::ApplyFailed(format!(
                "unsupported page rotation {value}; expected a multiple of 90 degrees"
            ))),
        }
    }

    fn content_rect_to_top_left(
        self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        rotation: i32,
    ) -> Result<[f32; 4], EngineError> {
        let corners = [
            self.content_point_to_top_left(x0, y0, rotation)?,
            self.content_point_to_top_left(x1, y0, rotation)?,
            self.content_point_to_top_left(x0, y1, rotation)?,
            self.content_point_to_top_left(x1, y1, rotation)?,
        ];
        let min_x = corners
            .iter()
            .map(|point| point.0)
            .fold(f32::INFINITY, f32::min);
        let max_x = corners
            .iter()
            .map(|point| point.0)
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = corners
            .iter()
            .map(|point| point.1)
            .fold(f32::INFINITY, f32::min);
        let max_y = corners
            .iter()
            .map(|point| point.1)
            .fold(f32::NEG_INFINITY, f32::max);
        Ok([min_x, min_y, max_x, max_y])
    }

    #[cfg(test)]
    fn content_span_to_top_left(self, x: f32, y: f32, width: f32, height: f32) -> [f32; 4] {
        self.content_rect_to_top_left(x, y, x + width, y + height, 0)
            .expect("zero-degree page rotation is always supported")
    }

    #[cfg(test)]
    fn top_left_to_content(self, bbox: [f32; 4]) -> [f32; 4] {
        [
            self.x_min + bbox[0],
            self.y_max - bbox[3],
            self.x_min + bbox[2],
            self.y_max - bbox[1],
        ]
    }

    fn is_valid(self) -> bool {
        self.x_max > self.x_min && self.y_max > self.y_min
    }
}

fn effective_page_box(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<CanonicalPageBox, EngineError> {
    for key in [b"CropBox".as_slice(), b"MediaBox".as_slice()] {
        if let Some(page_box) = inherited_page_box(doc, page_id, key) {
            if page_box.is_valid() {
                return Ok(page_box);
            }
        }
    }
    Err(EngineError::ExtractFailed(format!(
        "Page {page_id:?} has no valid inherited CropBox or MediaBox"
    )))
}

fn inherited_page_box(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
    key: &[u8],
) -> Option<CanonicalPageBox> {
    let mut current = page_id;
    let mut visited = std::collections::HashSet::new();
    loop {
        if !visited.insert(current) {
            return None;
        }
        let dictionary = doc.get_dictionary(current).ok()?;
        if let Ok(value) = dictionary.get(key) {
            if let Some(page_box) = object_as_page_box(doc, value) {
                return Some(page_box);
            }
        }
        current = dictionary
            .get(b"Parent")
            .and_then(lopdf::Object::as_reference)
            .ok()?;
    }
}

fn object_as_page_box(doc: &lopdf::Document, object: &lopdf::Object) -> Option<CanonicalPageBox> {
    let resolved = match object {
        lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
        value => value,
    };
    let values = resolved.as_array().ok()?;
    if values.len() != 4 {
        return None;
    }
    let x0 = operand_to_f32(&values[0])?;
    let y0 = operand_to_f32(&values[1])?;
    let x1 = operand_to_f32(&values[2])?;
    let y1 = operand_to_f32(&values[3])?;
    Some(CanonicalPageBox {
        x_min: x0.min(x1),
        y_min: y0.min(y1),
        x_max: x0.max(x1),
        y_max: y0.max(y1),
    })
}

/// Helper: extract a String from the first string operand.
fn extract_string_operand(operands: &[lopdf::Object]) -> Option<String> {
    for op in operands {
        if let lopdf::Object::String(bytes, _) = op {
            return Some(String::from_utf8_lossy(bytes).to_string());
        }
    }
    None
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeBatchEdit {
    page: usize,
    rect: [f32; 4],
    old_text: String,
    new_text: String,
}

#[derive(Debug, Clone)]
struct NativeTextTarget {
    operation_index: usize,
    text: String,
    bbox: [f32; 4],
    font: String,
    size: f32,
}

type PdfMatrix = [f32; 6];

fn matrix_multiply(left: PdfMatrix, right: PdfMatrix) -> PdfMatrix {
    [
        left[0] * right[0] + left[2] * right[1],
        left[1] * right[0] + left[3] * right[1],
        left[0] * right[2] + left[2] * right[3],
        left[1] * right[2] + left[3] * right[3],
        left[0] * right[4] + left[2] * right[5] + left[4],
        left[1] * right[4] + left[3] * right[5] + left[5],
    ]
}

fn transform_point(matrix: PdfMatrix, x: f32, y: f32) -> (f32, f32) {
    (
        matrix[0] * x + matrix[2] * y + matrix[4],
        matrix[1] * x + matrix[3] * y + matrix[5],
    )
}

fn operation_text_and_advance(
    operation: &lopdf::content::Operation,
    font_size: f32,
) -> Option<(String, f32)> {
    match operation.operator.as_str() {
        "Tj" => {
            let text = extract_string_operand(&operation.operands)?;
            let advance = text.chars().count() as f32 * font_size * 0.5;
            Some((text, advance))
        }
        "TJ" => {
            let lopdf::Object::Array(items) = operation.operands.first()? else {
                return None;
            };
            let mut text = String::new();
            let mut advance = 0.0;
            for item in items {
                match item {
                    lopdf::Object::String(bytes, _) => {
                        let part = String::from_utf8_lossy(bytes);
                        advance += part.chars().count() as f32 * font_size * 0.5;
                        text.push_str(&part);
                    }
                    lopdf::Object::Integer(value) => {
                        advance -= *value as f32 / 1000.0 * font_size;
                    }
                    lopdf::Object::Real(value) => {
                        advance -= *value / 1000.0 * font_size;
                    }
                    _ => {}
                }
            }
            Some((text, advance.max(0.0)))
        }
        _ => None,
    }
}

fn canonical_text_bbox(
    page_box: CanonicalPageBox,
    rotation: i32,
    ctm: PdfMatrix,
    text_matrix: PdfMatrix,
    advance: f32,
    font_size: f32,
) -> Result<[f32; 4], EngineError> {
    let render = matrix_multiply(ctm, text_matrix);
    let corners = [
        transform_point(render, 0.0, 0.0),
        transform_point(render, advance, 0.0),
        transform_point(render, 0.0, font_size),
        transform_point(render, advance, font_size),
    ];
    let min_x = corners
        .iter()
        .map(|point| point.0)
        .fold(f32::INFINITY, f32::min);
    let max_x = corners
        .iter()
        .map(|point| point.0)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = corners
        .iter()
        .map(|point| point.1)
        .fold(f32::INFINITY, f32::min);
    let max_y = corners
        .iter()
        .map(|point| point.1)
        .fold(f32::NEG_INFINITY, f32::max);
    page_box.content_rect_to_top_left(min_x, min_y, max_x, max_y, rotation)
}

fn collect_native_text_targets(
    operations: &[lopdf::content::Operation],
    page_box: CanonicalPageBox,
    rotation: i32,
) -> Result<Vec<NativeTextTarget>, EngineError> {
    let identity = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut ctm = identity;
    let mut graphics_stack = Vec::new();
    let mut tm = identity;
    let mut tlm = identity;
    let mut current_font = String::from("Unknown");
    let mut font_size = 12.0;
    let mut text_leading = 0.0;
    let mut in_text = false;
    let mut targets = Vec::new();

    for (operation_index, operation) in operations.iter().enumerate() {
        match operation.operator.as_str() {
            "q" => graphics_stack.push(ctm),
            "Q" => {
                ctm = graphics_stack.pop().ok_or_else(|| {
                    EngineError::ApplyFailed("unbalanced PDF graphics-state restore".into())
                })?;
            }
            "cm" => {
                if operation.operands.len() != 6 {
                    return Err(EngineError::ApplyFailed(
                        "malformed six-operand CTM transformation".into(),
                    ));
                }
                let mut transform = identity;
                for (index, operand) in operation.operands.iter().enumerate() {
                    transform[index] = operand_to_f32(operand).ok_or_else(|| {
                        EngineError::ApplyFailed("non-numeric CTM operand".into())
                    })?;
                }
                ctm = matrix_multiply(ctm, transform);
            }
            "BT" => {
                in_text = true;
                tm = identity;
                tlm = identity;
            }
            "ET" => in_text = false,
            "Tf" if in_text => {
                if operation.operands.len() >= 2 {
                    if let lopdf::Object::Name(name) = &operation.operands[0] {
                        current_font = String::from_utf8_lossy(name).to_string();
                    }
                    font_size = operand_to_f32(&operation.operands[1]).ok_or_else(|| {
                        EngineError::ApplyFailed("non-numeric text font size".into())
                    })?;
                }
            }
            "Tl" if in_text => {
                if let Some(operand) = operation.operands.first() {
                    text_leading = operand_to_f32(operand).ok_or_else(|| {
                        EngineError::ApplyFailed("non-numeric text leading".into())
                    })?;
                }
            }
            "Tm" if in_text => {
                if operation.operands.len() != 6 {
                    return Err(EngineError::ApplyFailed(
                        "malformed six-operand text matrix".into(),
                    ));
                }
                for (index, operand) in operation.operands.iter().enumerate() {
                    tm[index] = operand_to_f32(operand).ok_or_else(|| {
                        EngineError::ApplyFailed("non-numeric text-matrix operand".into())
                    })?;
                }
                tlm = tm;
            }
            "Td" | "TD" if in_text => {
                if operation.operands.len() < 2 {
                    return Err(EngineError::ApplyFailed(
                        "malformed text-line translation".into(),
                    ));
                }
                let tx = operand_to_f32(&operation.operands[0]).ok_or_else(|| {
                    EngineError::ApplyFailed("non-numeric text-line x translation".into())
                })?;
                let ty = operand_to_f32(&operation.operands[1]).ok_or_else(|| {
                    EngineError::ApplyFailed("non-numeric text-line y translation".into())
                })?;
                if operation.operator == "TD" {
                    text_leading = -ty;
                }
                tlm[4] += tlm[0] * tx + tlm[2] * ty;
                tlm[5] += tlm[1] * tx + tlm[3] * ty;
                tm = tlm;
            }
            "T*" if in_text => {
                let shift = if text_leading == 0.0 {
                    font_size
                } else {
                    text_leading
                };
                tlm[4] -= tlm[2] * shift;
                tlm[5] -= tlm[3] * shift;
                tm = tlm;
            }
            "Tj" | "TJ" if in_text => {
                if let Some((text, advance)) = operation_text_and_advance(operation, font_size) {
                    if !text.trim().is_empty() {
                        targets.push(NativeTextTarget {
                            operation_index,
                            text,
                            bbox: canonical_text_bbox(
                                page_box, rotation, ctm, tm, advance, font_size,
                            )?,
                            font: current_font.clone(),
                            size: font_size,
                        });
                    }
                    tm[4] += tm[0] * advance;
                    tm[5] += tm[1] * advance;
                }
            }
            "'" | "\"" if in_text => {
                return Err(EngineError::ApplyFailed(
                    "native exact editor does not support quote text-show operators".into(),
                ));
            }
            _ => {}
        }
    }
    if !graphics_stack.is_empty() {
        return Err(EngineError::ApplyFailed(
            "unbalanced PDF graphics-state save".into(),
        ));
    }
    Ok(targets)
}

fn normalized_text_identity(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn inherited_page_rotation(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> i32 {
    let mut current = page_id;
    let mut visited = std::collections::HashSet::new();
    while visited.insert(current) {
        let Ok(dictionary) = doc.get_dictionary(current) else {
            break;
        };
        if let Ok(value) = dictionary.get(b"Rotate") {
            if let Ok(rotation) = value.as_i64() {
                return rotation.rem_euclid(360) as i32;
            }
        }
        let Ok(parent) = dictionary
            .get(b"Parent")
            .and_then(lopdf::Object::as_reference)
        else {
            break;
        };
        current = parent;
    }
    0
}

fn save_lopdf_atomically(
    document: &mut lopdf::Document,
    output: &Path,
    expected_pages: usize,
) -> Result<(), EngineError> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| {
        EngineError::ApplyFailed(format!("Failed to create output directory: {error}"))
    })?;
    let temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| EngineError::ApplyFailed(format!("Failed to stage output: {error}")))?;
    let temporary_path = temporary.into_temp_path();
    let staged_path: &Path = temporary_path.as_ref();
    document
        .save(staged_path)
        .map_err(|error| EngineError::ApplyFailed(format!("Failed to save staged PDF: {error}")))?;
    let staged = lopdf::Document::load(staged_path)
        .map_err(|error| EngineError::ApplyFailed(format!("Staged PDF is unreadable: {error}")))?;
    if staged.get_pages().len() != expected_pages {
        return Err(EngineError::ApplyFailed(format!(
            "Staged PDF page count changed from {expected_pages} to {}",
            staged.get_pages().len()
        )));
    }
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(staged_path)
        .and_then(|file| file.sync_all())
        .map_err(|error| {
            EngineError::ApplyFailed(format!("Failed to flush staged PDF: {error}"))
        })?;
    temporary_path.persist(output).map_err(|error| {
        EngineError::ApplyFailed(format!("Failed to atomically publish PDF: {error}"))
    })?;
    #[cfg(unix)]
    {
        let _ = std::fs::File::open(parent).and_then(|directory| directory.sync_all());
    }
    Ok(())
}

/// Pdfium library resolver: finds or downloads the Pdfium shared library and
/// caches the resolved path so we only do the lookup once per process.
///
/// Search order:
///  1. `pdfium_lib/` next to the executable (shipped binary wins)
///  2. System library (PATH / LD_LIBRARY_PATH)
///  3. Auto-download from official GitHub releases (opt-in via
///     `PDFIUM_AUTO_DOWNLOAD=true` env var)
pub mod pdfium_resolver {
    use std::path::PathBuf;
    use std::sync::OnceLock;

    /// Cached result: `Ok(path)` where path is the directory containing the
    /// library, or `Err(reason)` if Pdfium could not be located.
    static RESOLVED: OnceLock<Result<PathBuf, String>> = OnceLock::new();

    /// Platform-specific library filename.
    fn lib_filename() -> &'static str {
        if cfg!(target_os = "windows") {
            "pdfium.dll"
        } else if cfg!(target_os = "macos") {
            "libpdfium.dylib"
        } else {
            "libpdfium.so"
        }
    }

    /// Directory next to the running executable (works for both debug and
    /// release layouts).
    fn exe_adjacent_dir() -> Option<PathBuf> {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    }

    /// Try to find the Pdfium library in well-known local directories.
    fn find_local() -> Option<PathBuf> {
        let name = lib_filename();

        // 1. pdfium_lib/ relative to CWD (project root at dev time)
        let cwd_lib = PathBuf::from("pdfium_lib").join(name);
        if cwd_lib.exists() {
            return Some(PathBuf::from("pdfium_lib"));
        }

        // 2. pdfium_lib/ next to the executable
        if let Some(exe_dir) = exe_adjacent_dir() {
            let candidate = exe_dir.join("pdfium_lib").join(name);
            if candidate.exists() {
                return Some(exe_dir.join("pdfium_lib"));
            }
            // 3. Directly next to the executable
            let candidate = exe_dir.join(name);
            if candidate.exists() {
                return Some(exe_dir);
            }
        }

        None
    }

    /// Download the Pdfium binary from the official bblanchon/pdfium-binaries
    /// GitHub releases. This is gated behind `PDFIUM_AUTO_DOWNLOAD=true`.
    ///
    /// Downloads to `pdfium_lib/` in the current working directory.
    #[cfg(not(test))]
    fn auto_download() -> Result<PathBuf, String> {
        let enabled = std::env::var("PDFIUM_AUTO_DOWNLOAD")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);
        if !enabled {
            return Err(
                "Pdfium not found locally. Set PDFIUM_AUTO_DOWNLOAD=true to auto-download.".into(),
            );
        }

        let (os_tag, ext) = if cfg!(target_os = "windows") {
            ("win-x64", "dll")
        } else if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                ("mac-arm64", "dylib")
            } else {
                ("mac-x64", "dylib")
            }
        } else {
            ("linux-x64", "so")
        };

        // Use a known stable Pdfium release (chromium/6721)
        let url = format!(
            "https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-{os_tag}.tgz"
        );

        tracing::info!("[pdfium] Auto-downloading Pdfium from {}", url);

        // Download using blocking reqwest (we're already on a blocking thread)
        let response = reqwest::blocking::get(&url)
            .map_err(|e| format!("Failed to download Pdfium from {url}: {e}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "Pdfium download failed: HTTP {}",
                response.status()
            ));
        }

        let bytes = response
            .bytes()
            .map_err(|e| format!("Failed to read Pdfium download: {e}"))?;

        // Extract the library from the tgz archive
        let dest = PathBuf::from("pdfium_lib");
        std::fs::create_dir_all(&dest).map_err(|e| format!("Failed to create pdfium_lib/: {e}"))?;

        let decoder = flate2::read::GzDecoder::new(std::io::Cursor::new(&bytes));
        let mut archive = tar::Archive::new(decoder);

        let lib_name = if ext == "dll" {
            "pdfium.dll"
        } else if ext == "dylib" {
            "libpdfium.dylib"
        } else {
            "libpdfium.so"
        };

        // Extract just the library file from the archive
        let mut found = false;
        for entry in archive.entries().map_err(|e| format!("tar error: {e}"))? {
            let mut entry = entry.map_err(|e| format!("tar entry error: {e}"))?;
            let path = entry
                .path()
                .map_err(|e| format!("tar path error: {e}"))?
                .to_path_buf();
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if file_name == lib_name {
                let out_path = dest.join(lib_name);
                entry
                    .unpack(&out_path)
                    .map_err(|e| format!("Failed to extract {lib_name}: {e}"))?;
                found = true;
                tracing::info!("[pdfium] Extracted {} to {:?}", lib_name, out_path);
                break;
            }
        }

        if !found {
            return Err(format!("Pdfium archive did not contain {lib_name}"));
        }

        Ok(dest)
    }

    #[cfg(test)]
    fn auto_download() -> Result<PathBuf, String> {
        Err("Auto-download disabled in tests".into())
    }

    /// Probe only installed or system Pdfium libraries. This function never
    /// downloads files and is safe for readiness, doctor, and capability UI.
    pub fn probe_local() -> Result<PathBuf, String> {
        if let Some(dir) = find_local() {
            return Ok(dir);
        }
        if pdfium_render::prelude::Pdfium::bind_to_system_library().is_ok() {
            return Ok(PathBuf::new());
        }
        Err("Pdfium is not installed locally or available as a system library".into())
    }

    /// Resolve the Pdfium library path, caching the result. Optional download
    /// remains an explicit resolver policy and is never used by `probe_local`.
    pub fn resolve() -> Result<PathBuf, String> {
        RESOLVED
            .get_or_init(|| probe_local().or_else(|_| auto_download()))
            .clone()
    }
}

/// Recommendation #1 - faithful page rasterisation using `pdfium-render`.
///
/// Uses [`pdfium_resolver`] to find or download the Pdfium library. The
/// resolver caches the library path so resolution/download happens at most
/// once per process; the actual DLL load is also effectively cached by the OS.
/// Renders the requested page at `dpi` using anti-aliasing flags pinned
/// identically to the fidelity verifier (`use_lcd_text_rendering(false)`
/// + smoothing on) so previews match what the verifier scores.
fn render_page_with_pdfium(
    path: &Path,
    page: usize,
    dpi: f32,
) -> Result<RenderedPage, EngineError> {
    use pdfium_render::prelude::*;

    let lib_dir = pdfium_resolver::resolve()
        .map_err(|e| EngineError::RenderFailed(format!("Pdfium unavailable: {e}")))?;

    let bindings = if lib_dir.as_os_str().is_empty() {
        // System library already validated in resolver
        Pdfium::bind_to_system_library()
            .map_err(|e| EngineError::RenderFailed(format!("Failed to bind system pdfium: {e}")))?
    } else {
        let lib_path =
            Pdfium::pdfium_platform_library_name_at_path(lib_dir.to_string_lossy().as_ref());
        Pdfium::bind_to_library(lib_path)
            .or_else(|_| Pdfium::bind_to_system_library())
            .map_err(|e| EngineError::RenderFailed(format!("Failed to bind pdfium: {e}")))?
    };

    let pdfium = Pdfium::new(bindings);

    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| EngineError::RenderFailed(format!("Failed to load PDF: {e}")))?;

    let pages = document.pages();
    let page_count = pages.len() as usize;
    if page >= page_count {
        return Err(EngineError::RenderFailed(format!(
            "Page {page} out of range (document has {page_count} pages)"
        )));
    }

    let pdf_page = pages
        .get(page as u16)
        .map_err(|e| EngineError::RenderFailed(format!("Failed to get page {page}: {e}")))?;

    let width_pts = pdf_page.width().value;
    let height_pts = pdf_page.height().value;

    let dpi = if dpi.is_finite() && dpi > 0.0 {
        dpi
    } else {
        150.0
    };
    let target_width = ((width_pts * dpi / 72.0).round() as i32).max(1);

    let config = PdfRenderConfig::new()
        .set_target_width(target_width)
        .set_clear_color(PdfColor::WHITE)
        .use_lcd_text_rendering(false)
        .set_text_smoothing(true)
        .set_path_smoothing(true)
        .set_image_smoothing(true)
        .render_annotations(true)
        .render_form_data(true);

    let image = pdf_page
        .render_with_config(&config)
        .map_err(|e| EngineError::RenderFailed(format!("pdfium render failed: {e}")))?
        .as_image()
        .into_rgba8();

    let mut png_bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .map_err(|e| EngineError::RenderFailed(format!("Failed to encode PNG: {e}")))?;

    Ok(RenderedPage {
        png_bytes,
        width_pts,
        height_pts,
    })
}

impl PdfEngine for OxidizePdfEngine {
    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            supports_redaction: true,
            supports_cjk: false, // Phase 3 - needs skrifa CID font mapping
            supports_embedded_fonts: true,
            estimated_fidelity: 0.85,
        }
    }

    fn render_page(&self, path: &Path, page: usize, dpi: f32) -> Result<RenderedPage, EngineError> {
        // Recommendation #1: faithful, pure-Rust(ish) rasterisation via
        // `pdfium-render` (already a dependency, already used by the fidelity
        // verifier). This makes the native engine the primary preview path so
        // previews no longer depend on the GIL-locked Python actor, while
        // PyMuPDF stays as the automatic fallback in the selector.
        render_page_with_pdfium(path, page, dpi)
    }

    fn get_text_blocks(&self, path: &Path, page: usize) -> Result<Vec<TextBlock>, EngineError> {
        self.extract_text_blocks_from_page(path, page)
    }

    fn find_text_block_at_click(
        &self,
        path: &Path,
        page: usize,
        x: f32,
        y: f32,
    ) -> Result<Option<TextBlock>, EngineError> {
        let blocks = self.get_text_blocks(path, page)?;
        Ok(blocks
            .into_iter()
            .find(|b| x >= b.bbox[0] && x <= b.bbox[2] && y >= b.bbox[1] && y <= b.bbox[3]))
    }

    fn apply_change(
        &self,
        input: &Path,
        output: &Path,
        page: usize,
        bbox: [f32; 4],
        new_text: &str,
        old_text: &str,
        font_path: Option<&Path>,
    ) -> Result<ReplaceOutcome, EngineError> {
        let edits_json = serde_json::json!([{
            "page": page,
            "rect": bbox,
            "old_text": old_text,
            "new_text": new_text,
        }])
        .to_string();
        let applied = self.apply_many_edits(input, output, &edits_json, font_path)?;
        if applied != 1 {
            return Err(EngineError::ApplyFailed(format!(
                "native single edit applied {applied}/1 targets"
            )));
        }
        Ok(ReplaceOutcome {
            success: true,
            font_used: "original-content-stream-font".into(),
            overflow: false,
            obj_id: None,
        })
    }

    fn analyze_layout(&self, path: &Path) -> Result<DocumentLayout, EngineError> {
        let page_count = self.page_count(path)?;

        let mut pages = Vec::with_capacity(page_count);
        for i in 0..page_count {
            let blocks = self
                .extract_text_blocks_from_page(path, i)
                .unwrap_or_default();

            // Simple heuristic: check for header/footer by position
            let has_header = blocks.iter().any(|b| b.bbox[1] < 72.0); // top inch
            let has_footer = blocks.iter().any(|b| b.bbox[1] > 720.0); // bottom inch

            let dominant_font = blocks
                .first()
                .map(|b| b.font.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            pages.push(PageLayout {
                page_number: i + 1,
                has_header,
                has_footer,
                has_page_number: false,
                table_columns: 0,
                main_text_style: "normal".to_string(),
                dominant_font,
            });
        }

        let has_consistent_headers = pages.iter().all(|p| p.has_header);
        let has_consistent_footers = pages.iter().all(|p| p.has_footer);

        Ok(DocumentLayout {
            total_pages: page_count,
            pages,
            has_consistent_headers,
            has_consistent_footers,
            overall_style: "standard".to_string(),
            layout_confidence: 0.7,
        })
    }

    fn apply_many_edits(
        &self,
        input: &std::path::Path,
        output: &std::path::Path,
        edits_json: &str,
        _font_path: Option<&std::path::Path>,
    ) -> Result<usize, EngineError> {
        let edits: Vec<NativeBatchEdit> = serde_json::from_str(edits_json).map_err(|error| {
            EngineError::ApplyFailed(format!("Invalid typed edits JSON: {error}"))
        })?;
        if edits.is_empty() {
            return Err(EngineError::ApplyFailed("empty edit batch".into()));
        }
        for (index, edit) in edits.iter().enumerate() {
            if edit.old_text.trim().is_empty() {
                return Err(EngineError::ApplyFailed(format!(
                    "edit {index} is missing stable old_text identity"
                )));
            }
            if !edit.new_text.is_ascii() {
                return Err(EngineError::FontCoverageMissing(
                    "Native engine requires ASCII for safe subset coverage; complex chars detected"
                        .into(),
                ));
            }
            if !edit.rect.iter().all(|value| value.is_finite())
                || edit.rect[2] <= edit.rect[0]
                || edit.rect[3] <= edit.rect[1]
            {
                return Err(EngineError::ApplyFailed(format!(
                    "edit {index} has invalid canonical rectangle {:?}",
                    edit.rect
                )));
            }
        }

        let mut document = lopdf::Document::load(input)
            .map_err(|error| EngineError::LoadFailed(format!("{error}")))?;
        let pages = document.get_pages();
        let expected_pages = pages.len();
        let mut edits_by_page: std::collections::BTreeMap<usize, Vec<usize>> =
            std::collections::BTreeMap::new();
        for (index, edit) in edits.iter().enumerate() {
            edits_by_page.entry(edit.page).or_default().push(index);
        }

        for (page_index, edit_indices) in edits_by_page {
            let page_id = *pages
                .get(&(page_index as u32 + 1))
                .ok_or_else(|| EngineError::ApplyFailed(format!("Page {page_index} not found")))?;
            let rotation = inherited_page_rotation(&document, page_id);
            let page_box = effective_page_box(&document, page_id)?;
            let content_bytes = document.get_page_content(page_id).map_err(|error| {
                EngineError::ApplyFailed(format!(
                    "Failed to read page {page_index} content: {error}"
                ))
            })?;
            if content_bytes.is_empty() {
                return Err(EngineError::ApplyFailed(format!(
                    "page {page_index} has no editable text content"
                )));
            }
            let mut content = lopdf::content::Content::decode(&content_bytes).map_err(|error| {
                EngineError::ApplyFailed(format!("Failed to decode page {page_index}: {error}"))
            })?;
            let targets = collect_native_text_targets(&content.operations, page_box, rotation)?;
            let mut selected_operations = std::collections::HashSet::new();
            let mut replacements = Vec::new();

            for edit_index in edit_indices {
                let edit = &edits[edit_index];
                let identity = normalized_text_identity(&edit.old_text);
                let candidates: Vec<&NativeTextTarget> = targets
                    .iter()
                    .filter(|target| {
                        normalized_text_identity(&target.text) == identity
                            && bbox_overlap_fraction(edit.rect, target.bbox) >= 0.5
                    })
                    .collect();
                if candidates.is_empty() {
                    return Err(EngineError::ApplyFailed(format!(
                        "edit {edit_index} stable target not found on page {page_index}: old_text={:?}, rect={:?}",
                        edit.old_text, edit.rect
                    )));
                }
                if candidates.len() != 1 {
                    return Err(EngineError::ApplyFailed(format!(
                        "edit {edit_index} is ambiguous on page {page_index}: {} operators match old_text={:?} and rect={:?}",
                        candidates.len(), edit.old_text, edit.rect
                    )));
                }
                let operation_index = candidates[0].operation_index;
                if !selected_operations.insert(operation_index) {
                    return Err(EngineError::ApplyFailed(format!(
                        "multiple edits select page {page_index} operation {operation_index}"
                    )));
                }
                replacements.push((operation_index, edit_index));
            }

            for (operation_index, edit_index) in replacements {
                let operation = content.operations.get_mut(operation_index).ok_or_else(|| {
                    EngineError::ApplyFailed(format!(
                        "resolved operation {operation_index} disappeared on page {page_index}"
                    ))
                })?;
                operation.operator = "Tj".to_string();
                operation.operands = vec![lopdf::Object::String(
                    edits[edit_index].new_text.as_bytes().to_vec(),
                    lopdf::StringFormat::Literal,
                )];
            }
            let encoded = content.encode().map_err(|error| {
                EngineError::ApplyFailed(format!("Failed to encode page {page_index}: {error}"))
            })?;
            document
                .change_page_content(page_id, encoded)
                .map_err(|error| {
                    EngineError::ApplyFailed(format!("Failed to update page {page_index}: {error}"))
                })?;
        }

        save_lopdf_atomically(&mut document, output, expected_pages)?;
        Ok(edits.len())
    }

    fn clone_pages(
        &self,
        input: &std::path::Path,
        output: &std::path::Path,
        page_indices: Vec<usize>,
    ) -> Result<usize, EngineError> {
        if page_indices.is_empty() {
            return Err(EngineError::ApplyFailed("empty page-clone request".into()));
        }
        let unique = page_indices
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if unique.len() != page_indices.len() {
            return Err(EngineError::ApplyFailed(
                "duplicate page indices are not an exact clone request".into(),
            ));
        }

        let mut document = lopdf::Document::load(input)
            .map_err(|error| EngineError::LoadFailed(format!("{error}")))?;
        let original_pages = document.page_iter().collect::<Vec<_>>();
        for &index in &unique {
            if index >= original_pages.len() {
                return Err(EngineError::ApplyFailed(format!(
                    "clone page index {index} is out of range for {} pages",
                    original_pages.len()
                )));
            }
        }

        let mut clone_ids = std::collections::BTreeMap::new();
        for &index in &unique {
            let source_id = original_pages[index];
            let mut page_object = document
                .get_object(source_id)
                .map_err(|error| {
                    EngineError::ApplyFailed(format!(
                        "failed to read clone source page {index}: {error}"
                    ))
                })?
                .clone();
            let parent_id = page_object
                .as_dict()
                .and_then(|dictionary| dictionary.get(b"Parent"))
                .and_then(lopdf::Object::as_reference)
                .map_err(|error| {
                    EngineError::ApplyFailed(format!(
                        "clone source page {index} has no valid Parent: {error}"
                    ))
                })?;
            page_object
                .as_dict_mut()
                .map_err(|error| {
                    EngineError::ApplyFailed(format!(
                        "clone source page {index} is not a dictionary: {error}"
                    ))
                })?
                .set("Parent", lopdf::Object::Reference(parent_id));
            let clone_id = document.add_object(page_object);

            {
                let parent = document.get_dictionary_mut(parent_id).map_err(|error| {
                    EngineError::ApplyFailed(format!(
                        "failed to open parent page tree for page {index}: {error}"
                    ))
                })?;
                let kids = parent
                    .get_mut(b"Kids")
                    .and_then(lopdf::Object::as_array_mut)
                    .map_err(|error| {
                        EngineError::ApplyFailed(format!(
                            "parent page tree for page {index} has invalid Kids: {error}"
                        ))
                    })?;
                let position = kids
                    .iter()
                    .position(|item| {
                        item.as_reference()
                            .map(|candidate| candidate == source_id)
                            .unwrap_or(false)
                    })
                    .ok_or_else(|| {
                        EngineError::ApplyFailed(format!(
                            "source page {index} is absent from its parent Kids array"
                        ))
                    })?;
                kids.insert(position + 1, lopdf::Object::Reference(clone_id));
            }

            let mut current = Some(parent_id);
            let mut visited = std::collections::HashSet::new();
            while let Some(tree_id) = current {
                if !visited.insert(tree_id) {
                    return Err(EngineError::ApplyFailed(
                        "cycle detected in page-tree Parent chain".into(),
                    ));
                }
                let tree = document.get_dictionary_mut(tree_id).map_err(|error| {
                    EngineError::ApplyFailed(format!("failed to update page-tree count: {error}"))
                })?;
                let count =
                    tree.get(b"Count")
                        .and_then(lopdf::Object::as_i64)
                        .map_err(|error| {
                            EngineError::ApplyFailed(format!(
                                "page-tree node has invalid Count: {error}"
                            ))
                        })?;
                tree.set("Count", count + 1);
                current = tree
                    .get(b"Parent")
                    .and_then(lopdf::Object::as_reference)
                    .ok();
            }
            clone_ids.insert(index, clone_id);
        }

        let mut expected_order = Vec::with_capacity(original_pages.len() + unique.len());
        for (index, page_id) in original_pages.iter().copied().enumerate() {
            expected_order.push(page_id);
            if let Some(clone_id) = clone_ids.get(&index) {
                expected_order.push(*clone_id);
            }
        }
        let actual_order = document.page_iter().collect::<Vec<_>>();
        if actual_order != expected_order {
            return Err(EngineError::ApplyFailed(
                "cloned page order does not match immediate-after-source contract".into(),
            ));
        }
        save_lopdf_atomically(&mut document, output, expected_order.len())?;
        Ok(unique.len())
    }

    fn remove_pages(
        &self,
        input: &std::path::Path,
        output: &std::path::Path,
        page_indices: Vec<usize>,
    ) -> Result<usize, EngineError> {
        if page_indices.is_empty() {
            return Err(EngineError::ApplyFailed(
                "empty page-removal request".into(),
            ));
        }
        let unique = page_indices
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if unique.len() != page_indices.len() {
            return Err(EngineError::ApplyFailed(
                "duplicate page indices are not an exact removal request".into(),
            ));
        }

        let mut document = lopdf::Document::load(input)
            .map_err(|error| EngineError::LoadFailed(format!("{error}")))?;
        let original_pages = document.page_iter().collect::<Vec<_>>();
        for &index in &unique {
            if index >= original_pages.len() {
                return Err(EngineError::ApplyFailed(format!(
                    "remove page index {index} is out of range for {} pages",
                    original_pages.len()
                )));
            }
        }
        if unique.len() >= original_pages.len() {
            return Err(EngineError::ApplyFailed(
                "removing every page would create an invalid PDF".into(),
            ));
        }

        let page_numbers = unique
            .iter()
            .map(|index| *index as u32 + 1)
            .collect::<Vec<_>>();
        document.delete_pages(&page_numbers);
        let expected_order = original_pages
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(index, page_id)| (!unique.contains(&index)).then_some(page_id))
            .collect::<Vec<_>>();
        let actual_order = document.page_iter().collect::<Vec<_>>();
        if actual_order != expected_order {
            return Err(EngineError::ApplyFailed(
                "remaining page order changed during exact removal".into(),
            ));
        }
        save_lopdf_atomically(&mut document, output, expected_order.len())?;
        Ok(unique.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities() {
        let engine = OxidizePdfEngine::new();
        let caps = engine.capabilities();
        assert!(caps.supports_redaction);
        assert!(caps.supports_embedded_fonts);
        assert!(!caps.supports_cjk); // Not yet
    }

    #[test]
    fn operand_to_f32_converts_correctly() {
        assert_eq!(operand_to_f32(&lopdf::Object::Integer(42)), Some(42.0));
        assert_eq!(operand_to_f32(&lopdf::Object::Real(2.5)), Some(2.5));
        assert_eq!(operand_to_f32(&lopdf::Object::Boolean(true)), None);
    }

    #[test]
    fn content_and_top_left_coordinates_round_trip() {
        let page_box = CanonicalPageBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 605.0,
            y_max: 862.0,
        };
        let canonical = page_box.content_span_to_top_left(82.0, 740.0, 120.0, 12.0);
        assert_eq!(canonical, [72.0, 110.0, 192.0, 122.0]);
        assert_eq!(
            page_box.top_left_to_content(canonical),
            [82.0, 740.0, 202.0, 752.0]
        );
    }

    #[test]
    fn crop_origin_rotation_mappings_are_exact() {
        let page_box = CanonicalPageBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 110.0,
            y_max: 220.0,
        };
        let content_rect = [20.0, 30.0, 40.0, 50.0];
        assert_eq!(
            page_box
                .content_rect_to_top_left(
                    content_rect[0],
                    content_rect[1],
                    content_rect[2],
                    content_rect[3],
                    90,
                )
                .unwrap(),
            [10.0, 10.0, 30.0, 30.0]
        );
        assert_eq!(
            page_box
                .content_rect_to_top_left(
                    content_rect[0],
                    content_rect[1],
                    content_rect[2],
                    content_rect[3],
                    180,
                )
                .unwrap(),
            [70.0, 10.0, 90.0, 30.0]
        );
        assert_eq!(
            page_box
                .content_rect_to_top_left(
                    content_rect[0],
                    content_rect[1],
                    content_rect[2],
                    content_rect[3],
                    270,
                )
                .unwrap(),
            [170.0, 70.0, 190.0, 90.0]
        );
    }

    #[test]
    fn synthetic_fixture_bbox_matches_pymupdf_top_left_space() {
        let page_box = CanonicalPageBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 595.0,
            y_max: 842.0,
        };
        assert_eq!(
            page_box.content_span_to_top_left(72.0, 720.0, 198.0, 12.0),
            [72.0, 110.0, 270.0, 122.0]
        );
    }
}
