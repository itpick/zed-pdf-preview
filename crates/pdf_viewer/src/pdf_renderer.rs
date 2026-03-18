use std::sync::Arc;

use anyhow::Result;
use gpui::{DevicePixels, RenderImage};
use rpdfium::{ArcDocument, ArcLibrary, BitmapFormat, OpenOptions, RenderConfig, RgbaColor};

/// Holds a parsed PDF document. All PDF operations go through this struct.
/// This is Send + Sync (rpdfium's Arc types are thread-safe).
pub struct PdfDocument {
    library: ArcLibrary,
    document: ArcDocument,
}

#[derive(Debug)]
pub enum PdfLoadError {
    ParseError(String),
    PasswordProtected,
}

impl std::fmt::Display for PdfLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfLoadError::ParseError(message) => write!(f, "Failed to parse PDF: {message}"),
            PdfLoadError::PasswordProtected => write!(f, "This PDF is password-protected"),
        }
    }
}

impl std::error::Error for PdfLoadError {}

impl PdfDocument {
    /// Detects encrypted PDFs and returns a specific error for them.
    pub fn open(data: Vec<u8>) -> std::result::Result<Self, PdfLoadError> {
        let library = ArcLibrary::new();
        match ArcDocument::open(library.clone(), data, &OpenOptions::default()) {
            Ok(document) => Ok(Self { library, document }),
            Err(error) => {
                let error_string = format!("{error}");
                if error_string.contains("assword")
                    || error_string.contains("ncrypt")
                    || error_string.contains("InvalidPassword")
                {
                    Err(PdfLoadError::PasswordProtected)
                } else {
                    Err(PdfLoadError::ParseError(error_string))
                }
            }
        }
    }

    pub fn page_count(&self) -> u32 {
        self.document.page_count()
    }

    /// Page dimensions in points (72 points = 1 inch).
    pub fn page_dimensions(&self, page_index: u32) -> Result<(f64, f64)> {
        let page = self.document.page(page_index)?;
        Ok((page.page_width(), page.page_height()))
    }

    /// Render a single page to RGBA pixels. CPU-intensive — call from background thread.
    pub fn render_page(&self, page_index: u32, dpi: f32) -> Result<RenderedPage> {
        let page = self.document.page(page_index)?;
        let width = (page.page_width() * dpi as f64 / 72.0) as u32;
        let height = (page.page_height() * dpi as f64 / 72.0) as u32;

        let config = RenderConfig {
            width,
            height,
            background: RgbaColor::WHITE,
            antialiasing: true,
            text_antialiasing: true,
            ..RenderConfig::default()
        };

        let bitmap = page.render(&config)?;
        let rgba = bitmap
            .convert_format(BitmapFormat::Rgba32)
            .ok_or_else(|| anyhow::anyhow!("Failed to convert bitmap to RGBA32"))?;

        Ok(RenderedPage {
            width: rgba.width,
            height: rgba.height,
            data: rgba.data,
        })
    }

    pub fn extract_page_text(&self, page_index: u32) -> Result<String> {
        let page = self.document.page(page_index)?;
        let text_page = page.text()?;
        Ok(text_page.all_page_text().to_string())
    }

    /// Find character index nearest to a point in page-space coordinates (points).
    pub fn char_index_at_point(
        &self,
        page_index: u32,
        x_points: f32,
        y_points: f32,
    ) -> Option<usize> {
        let page = self.document.page(page_index).ok()?;
        let text_page = page.text().ok()?;
        text_page.index_at_pos(x_points, y_points, 10.0, 10.0)
    }

    pub fn extract_text_range(
        &self,
        page_index: u32,
        start: usize,
        end: usize,
    ) -> Option<String> {
        let page = self.document.page(page_index).ok()?;
        let text_page = page.text().ok()?;
        let (from, to) = (start.min(end), start.max(end));
        let mut result = String::new();
        for index in from..=to {
            if let Some(character) = text_page.unicode(index) {
                result.push(character);
            }
        }
        Some(result)
    }
}

/// A rendered page as raw RGBA pixels.
pub struct RenderedPage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl RenderedPage {
    pub fn into_render_image(self) -> Arc<RenderImage> {
        Arc::new(RenderImage::new(vec![gpui::Frame::new(
            DevicePixels(self.width as i32),
            DevicePixels(self.height as i32),
            self.data,
        )]))
    }
}
