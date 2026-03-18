mod pdf_renderer;
pub use pdf_renderer::{PdfDocument, PdfLoadError, RenderedPage};

use gpui::App;

pub fn init(_cx: &mut App) {
    // Will register PdfView here
}
