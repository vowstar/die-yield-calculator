//! Platform-specific delivery of shared in-memory report files.

#[cfg(not(target_arch = "wasm32"))]
use crate::report::ReportFormat;
use crate::report::{self, ReportFile};
use die_yield_core::{FabricationInputs, WaferAnalysis};

/// Saves or downloads one generated report.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_report(file: &ReportFile) -> Result<Option<String>, String> {
    let extension = file.filename.rsplit('.').next().unwrap_or("report");
    let filter_name = file.mime_type.rsplit('/').next().unwrap_or(extension);
    let Some(path) = rfd::FileDialog::new()
        .add_filter(filter_name.to_uppercase(), &[extension])
        .set_file_name(&file.filename)
        .save_file()
    else {
        return Ok(None);
    };

    std::fs::write(&path, &file.bytes)
        .map_err(|error| format!("Unable to save {}: {error}", path.display()))?;
    Ok(Some(format!("Saved {}", path.display())))
}

/// Saves or downloads one generated report.
#[cfg(target_arch = "wasm32")]
pub fn save_report(file: &ReportFile) -> Result<Option<String>, String> {
    use eframe::wasm_bindgen::JsCast as _;

    let bytes = js_sys::Uint8Array::from(file.bytes.as_slice());
    let parts = js_sys::Array::new();
    parts.push(&bytes);
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(file.mime_type);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .map_err(js_error)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js_error)?;
    let window = web_sys::window().ok_or_else(|| "Browser window is unavailable".to_owned())?;
    let document = window
        .document()
        .ok_or_else(|| "Browser document is unavailable".to_owned())?;
    let anchor = document
        .create_element("a")
        .map_err(js_error)?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "Unable to create the download link".to_owned())?;
    anchor.set_href(&url);
    anchor.set_download(&file.filename);
    anchor.click();
    web_sys::Url::revoke_object_url(&url).map_err(js_error)?;

    Ok(Some(format!("Downloaded {}", file.filename)))
}

/// Opens the platform print flow for a generated report.
#[cfg(not(target_arch = "wasm32"))]
pub fn print_report(
    inputs: &FabricationInputs,
    analysis: &WaferAnalysis,
) -> Result<String, String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let pdf =
        report::generate(inputs, analysis, ReportFormat::Pdf).map_err(|error| error.to_string())?;
    let identifier = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("System clock is unavailable: {error}"))?
        .as_millis();
    let path = std::env::temp_dir().join(format!("yield-studio-report-{identifier}.pdf"));
    std::fs::write(&path, &pdf.bytes)
        .map_err(|error| format!("Unable to prepare the printable PDF: {error}"))?;
    open::that_detached(&path)
        .map_err(|error| format!("Unable to open the system PDF viewer: {error}"))?;

    Ok("Opened a print-ready PDF in the default viewer".to_owned())
}

/// Opens the platform print flow for a generated report.
#[cfg(target_arch = "wasm32")]
pub fn print_report(
    inputs: &FabricationInputs,
    analysis: &WaferAnalysis,
) -> Result<String, String> {
    use eframe::wasm_bindgen::JsValue;

    let svg = report::build_svg(inputs, analysis).map_err(|error| error.to_string())?;
    let html = format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>Yield Studio Report</title><style>
@page {{ size: A4 portrait; margin: 0; }}
html, body {{ margin: 0; padding: 0; background: #f3f6f5; }}
svg {{ display: block; width: 210mm; height: 297mm; }}
</style></head><body>{svg}<script>
window.addEventListener('load', () => {{
  URL.revokeObjectURL(location.href);
  setTimeout(() => window.print(), 150);
}});
</script></body></html>"#
    );
    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(&html));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("text/html;charset=utf-8");
    let blob =
        web_sys::Blob::new_with_str_sequence_and_options(&parts, &options).map_err(js_error)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js_error)?;
    let window = web_sys::window().ok_or_else(|| "Browser window is unavailable".to_owned())?;
    window
        .open_with_url_and_target(&url, "_blank")
        .map_err(js_error)?
        .ok_or_else(|| "The browser blocked the print window".to_owned())?;

    Ok("Opened the browser print dialog".to_owned())
}

#[cfg(target_arch = "wasm32")]
fn js_error(error: eframe::wasm_bindgen::JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| "Browser operation failed".to_owned())
}
