//! Shared, in-memory report generation for native and browser exports.

use crate::{app::wafer_size_label, theme};
use base64::prelude::*;
use die_yield_core::{FabricationInputs, WaferAnalysis};
use die_yield_render::{CellTone, WaferPalette, WaferScene};
use egui::Color32;
use resvg::tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};
use std::{fmt, fmt::Write as _};

const REPORT_WIDTH: u32 = 1240;
const REPORT_HEIGHT: u32 = 1754;
const PNG_SCALE: u32 = 2;
const MAP_PIXELS: u32 = 800;

/// Available report file formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportFormat {
    /// High-resolution raster report.
    Png,
    /// Portable vector report with an embedded font and wafer-map image.
    Svg,
    /// A4 print-ready document.
    Pdf,
}

impl ReportFormat {
    /// File extension without a leading dot.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
            Self::Pdf => "pdf",
        }
    }

    /// Internet media type for downloads.
    #[must_use]
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Svg => "image/svg+xml",
            Self::Pdf => "application/pdf",
        }
    }
}

/// Generated report bytes and download metadata.
#[derive(Debug)]
pub struct ReportFile {
    /// Suggested filename.
    pub filename: String,
    /// Internet media type.
    pub mime_type: &'static str,
    /// Complete file contents.
    pub bytes: Vec<u8>,
}

/// Report generation failure suitable for surfacing in the interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportError(String);

impl ReportError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ReportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ReportError {}

/// Generates one complete report in the requested format.
pub fn generate(
    inputs: &FabricationInputs,
    analysis: &WaferAnalysis,
    format: ReportFormat,
) -> Result<ReportFile, ReportError> {
    let svg = build_svg(inputs, analysis)?;
    let bytes = match format {
        ReportFormat::Svg => svg.into_bytes(),
        ReportFormat::Png => svg_to_png(&svg)?,
        ReportFormat::Pdf => svg_to_pdf(&svg)?,
    };

    Ok(ReportFile {
        filename: format!("yield-studio-report.{}", format.extension()),
        mime_type: format.mime_type(),
        bytes,
    })
}

/// Builds the styled SVG source also used by the browser print view.
pub fn build_svg(
    inputs: &FabricationInputs,
    analysis: &WaferAnalysis,
) -> Result<String, ReportError> {
    let map_png = render_map_png(analysis)?;
    let map_data = BASE64_STANDARD.encode(map_png);
    let font_data = BASE64_STANDARD.encode(epaint_default_fonts::UBUNTU_LIGHT);
    let summary = analysis.summary;
    let scene = WaferScene::from_analysis(analysis);
    let palette = WaferPalette::default();

    let mut svg = String::with_capacity(font_data.len() + map_data.len() + 24_000);
    push_fmt(
        &mut svg,
        format_args!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{REPORT_WIDTH}" height="{REPORT_HEIGHT}" viewBox="0 0 {REPORT_WIDTH} {REPORT_HEIGHT}">
<defs>
  <style><![CDATA[
    @font-face {{ font-family: 'Ubuntu'; src: url(data:font/ttf;base64,{font_data}) format('truetype'); font-weight: 300; }}
    text {{ font-family: 'Ubuntu', sans-serif; }}
  ]]></style>
  <clipPath id="map-clip"><rect x="103" y="365" width="630" height="630" rx="24"/></clipPath>
</defs>
<rect width="1240" height="1754" fill="{}"/>
<rect x="56" y="48" width="56" height="56" rx="16" fill="{}"/>
<text x="84" y="84" text-anchor="middle" font-size="21" fill="white">YS</text>
<text x="132" y="70" font-size="30" fill="{}">Wafer Yield Report</text>
<text x="132" y="99" font-size="15" fill="{}">A print-ready analysis snapshot from Yield Studio</text>
<rect x="1008" y="55" width="176" height="40" rx="20" fill="{}" stroke="{}"/>
<text x="1096" y="81" text-anchor="middle" font-size="13" fill="{}">ANALYSIS REPORT</text>
"#,
            hex(theme::CANVAS),
            hex(theme::ACCENT),
            hex(theme::TEXT),
            hex(theme::TEXT_MUTED),
            rgba_hex(theme::ACCENT, 20),
            rgba_hex(theme::ACCENT, 75),
            hex(theme::ACCENT),
        ),
    );

    metric_card(
        &mut svg,
        56.0,
        "MODEL YIELD",
        &format!("{:.2}%", summary.yield_fraction * 100.0),
        "Murphy estimate",
        theme::ACCENT,
    );
    metric_card(
        &mut svg,
        342.0,
        "EXPECTED GOOD",
        &format_integer(summary.expected_good),
        &format!("of {} usable", format_integer(summary.geometric_usable)),
        theme::BLUE,
    );
    metric_card(
        &mut svg,
        628.0,
        "DIE LOSS",
        &format_integer(summary.expected_defective),
        &format!("{} boundary sites", format_integer(summary.partial)),
        theme::CORAL,
    );
    metric_card(
        &mut svg,
        914.0,
        "TOUCHDOWNS",
        &format_integer(analysis.probe.touchdown_count),
        &format!("{} sites per step", analysis.probe.sites_per_touchdown),
        theme::AMBER,
    );

    push_fmt(
        &mut svg,
        format_args!(
            r#"<rect x="56" y="288" width="724" height="770" rx="20" fill="{}" stroke="{}"/>
<text x="88" y="330" font-size="22" fill="{}">Wafer map</text>
<text x="88" y="355" font-size="14" fill="{}">Geometric placement and modeled process loss</text>
<rect x="592" y="309" width="156" height="40" rx="14" fill="{}" stroke="{}"/>
<text x="670" y="335" text-anchor="middle" font-size="13" fill="{}">Ø {}</text>
<image x="103" y="365" width="630" height="630" clip-path="url(#map-clip)" href="data:image/png;base64,{map_data}"/>
"#,
            hex(theme::SURFACE),
            hex(theme::BORDER),
            hex(theme::TEXT),
            hex(theme::TEXT_MUTED),
            rgba_hex(theme::BLUE, 16),
            rgba_hex(theme::BLUE, 60),
            hex(theme::BLUE),
            wafer_size_label(inputs.wafer.diameter_mm),
        ),
    );
    legend_item(&mut svg, 88.0, 1027.0, palette.productive, "Expected good");
    legend_item(&mut svg, 225.0, 1027.0, palette.defective, "Modeled loss");
    legend_item(&mut svg, 353.0, 1027.0, palette.boundary, "Boundary");
    legend_item(&mut svg, 451.0, 1027.0, palette.excluded, "Edge band");
    legend_item(&mut svg, 564.0, 1027.0, palette.scribe, "Scribe lane");

    push_fmt(
        &mut svg,
        format_args!(
            r#"<rect x="804" y="288" width="380" height="770" rx="20" fill="{}" stroke="{}"/>
<text x="832" y="330" font-size="22" fill="{}">Process parameters</text>
<text x="832" y="354" font-size="14" fill="{}">Values captured with this analysis</text>
"#,
            hex(theme::SURFACE),
            hex(theme::BORDER),
            hex(theme::TEXT),
            hex(theme::TEXT_MUTED),
        ),
    );

    parameter_heading(&mut svg, 832.0, 390.0, "WAFER", SectionMark::Wafer);
    parameter_row(
        &mut svg,
        832.0,
        424.0,
        "Diameter",
        &wafer_size_label(inputs.wafer.diameter_mm),
    );
    parameter_row(
        &mut svg,
        832.0,
        460.0,
        "Edge exclusion",
        &measurement(inputs.wafer.edge_exclusion_mm, "mm"),
    );

    parameter_heading(&mut svg, 832.0, 510.0, "DIE & SCRIBE", SectionMark::Die);
    parameter_row(
        &mut svg,
        832.0,
        544.0,
        "Active width",
        &measurement(inputs.die.width_mm, "mm"),
    );
    parameter_row(
        &mut svg,
        832.0,
        580.0,
        "Active height",
        &measurement(inputs.die.height_mm, "mm"),
    );
    parameter_row(
        &mut svg,
        832.0,
        616.0,
        "Column scribe",
        &measurement(inputs.die.column_lane_mm, "mm"),
    );
    parameter_row(
        &mut svg,
        832.0,
        652.0,
        "Row scribe",
        &measurement(inputs.die.row_lane_mm, "mm"),
    );

    parameter_heading(
        &mut svg,
        832.0,
        702.0,
        "PROCESS & ALIGNMENT",
        SectionMark::Alignment,
    );
    parameter_row(
        &mut svg,
        832.0,
        736.0,
        "Defect density",
        &measurement(inputs.process.defect_density_cm2, "/cm²"),
    );
    parameter_row(
        &mut svg,
        832.0,
        772.0,
        "Horizontal phase",
        &measurement(inputs.process.offset_x_mm, "mm"),
    );
    parameter_row(
        &mut svg,
        832.0,
        808.0,
        "Vertical phase",
        &measurement(inputs.process.offset_y_mm, "mm"),
    );
    parameter_row(
        &mut svg,
        832.0,
        844.0,
        "Die at origin",
        if inputs.process.die_at_origin {
            "Yes"
        } else {
            "No"
        },
    );

    parameter_heading(&mut svg, 832.0, 894.0, "PROBE ARRAY", SectionMark::Probe);
    parameter_row(
        &mut svg,
        832.0,
        928.0,
        "Columns per step",
        &inputs.probe.columns.to_string(),
    );
    parameter_row(
        &mut svg,
        832.0,
        964.0,
        "Rows per step",
        &inputs.probe.rows.to_string(),
    );

    push_fmt(
        &mut svg,
        format_args!(
            r#"<rect x="56" y="1082" width="1128" height="242" rx="20" fill="{}" stroke="{}"/>
<text x="88" y="1125" font-size="18" fill="{}">Fabrication summary</text>
"#,
            hex(theme::SURFACE),
            hex(theme::BORDER),
            hex(theme::TEXT),
        ),
    );
    summary_value(
        &mut svg,
        88.0,
        1170.0,
        "MAPPED SITES",
        scene.cells.len() as u64,
    );
    summary_value(
        &mut svg,
        348.0,
        1170.0,
        "GEOMETRIC USABLE",
        summary.geometric_usable,
    );
    summary_value(&mut svg, 608.0, 1170.0, "EDGE BAND", summary.edge_excluded);
    summary_value(&mut svg, 868.0, 1170.0, "BOUNDARY", summary.partial);
    push_fmt(
        &mut svg,
        format_args!(
            r#"<line x1="88" y1="1244" x2="1152" y2="1244" stroke="{}"/>
<text x="88" y="1280" font-size="14" fill="{}">Active die area</text>
<text x="244" y="1280" font-size="14" fill="{}">{:.3} mm²</text>
<text x="462" y="1280" font-size="14" fill="{}">Grid pitch</text>
<text x="566" y="1280" font-size="14" fill="{}">{} × {} mm</text>
<text x="850" y="1280" font-size="14" fill="{}">Usable diameter</text>
<text x="1005" y="1280" font-size="14" fill="{}">{} mm</text>
"#,
            hex(theme::BORDER),
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT),
            inputs.die.width_mm * inputs.die.height_mm,
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT),
            compact(inputs.die.width_mm + inputs.die.column_lane_mm, 3),
            compact(inputs.die.height_mm + inputs.die.row_lane_mm, 3),
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT),
            compact(scene.usable_diameter_mm, 3),
        ),
    );

    push_fmt(
        &mut svg,
        format_args!(
            r#"<rect x="56" y="1348" width="1128" height="272" rx="20" fill="{}" stroke="{}"/>
<text x="88" y="1392" font-size="18" fill="{}">Reading the report</text>
<rect x="88" y="1424" width="4" height="132" rx="2" fill="{}"/>
<text x="112" y="1450" font-size="15" fill="{}">Model</text>
<text x="112" y="1478" font-size="14" fill="{}">Murphy yield estimates random-defect loss from active die area and defect density.</text>
<text x="112" y="1522" font-size="15" fill="{}">Map</text>
<text x="112" y="1550" font-size="14" fill="{}">The wafer image is a deterministic snapshot of the current geometry and modeled defects.</text>
<text x="112" y="1588" font-size="13" fill="{}">Planning estimate only. Validate production decisions with process-specific characterization.</text>
<line x1="56" y1="1672" x2="1184" y2="1672" stroke="{}"/>
<text x="56" y="1708" font-size="13" fill="{}">Yield Studio · Wafer planning, without the spreadsheet</text>
<text x="1184" y="1708" text-anchor="end" font-size="13" fill="{}">A4 portrait · 150 dpi reference scale</text>
</svg>"#,
            hex(theme::SURFACE),
            hex(theme::BORDER),
            hex(theme::TEXT),
            hex(theme::ACCENT),
            hex(theme::BLUE),
            hex(theme::TEXT_MUTED),
            hex(theme::BLUE),
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT_MUTED),
            hex(theme::BORDER),
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT_MUTED),
        ),
    );

    Ok(svg)
}

fn svg_to_png(svg: &str) -> Result<Vec<u8>, ReportError> {
    let mut options = resvg::usvg::Options {
        font_family: "Ubuntu".to_owned(),
        ..Default::default()
    };
    options
        .fontdb_mut()
        .load_font_data(epaint_default_fonts::UBUNTU_LIGHT.to_vec());
    let tree = resvg::usvg::Tree::from_str(svg, &options)
        .map_err(|error| ReportError::new(format!("unable to parse report SVG: {error}")))?;
    let mut pixmap = Pixmap::new(REPORT_WIDTH * PNG_SCALE, REPORT_HEIGHT * PNG_SCALE)
        .ok_or_else(|| ReportError::new("unable to allocate the PNG report canvas"))?;
    let mut target = pixmap.as_mut();
    resvg::render(
        &tree,
        Transform::from_scale(PNG_SCALE as f32, PNG_SCALE as f32),
        &mut target,
    );
    pixmap
        .encode_png()
        .map_err(|error| ReportError::new(format!("unable to encode PNG report: {error}")))
}

fn svg_to_pdf(svg: &str) -> Result<Vec<u8>, ReportError> {
    let mut options = svg2pdf::usvg::Options {
        font_family: "Ubuntu".to_owned(),
        ..Default::default()
    };
    options
        .fontdb_mut()
        .load_font_data(epaint_default_fonts::UBUNTU_LIGHT.to_vec());
    let tree = svg2pdf::usvg::Tree::from_str(svg, &options)
        .map_err(|error| ReportError::new(format!("unable to parse report for PDF: {error}")))?;
    svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions { dpi: 150.0 },
    )
    .map_err(|error| ReportError::new(format!("unable to encode PDF report: {error}")))
}

fn render_map_png(analysis: &WaferAnalysis) -> Result<Vec<u8>, ReportError> {
    let scene = WaferScene::from_analysis(analysis);
    let palette = WaferPalette::default();
    let mut pixmap = Pixmap::new(MAP_PIXELS, MAP_PIXELS)
        .ok_or_else(|| ReportError::new("unable to allocate the wafer-map image"))?;
    pixmap.fill(skia_color(palette.backdrop));

    let center = MAP_PIXELS as f32 * 0.5;
    let radius = center - 28.0;
    let scale = radius / (scene.diameter_mm as f32 * 0.5);

    fill_circle(
        &mut pixmap,
        center,
        center,
        radius + 4.0,
        Color32::from_rgba_unmultiplied(0, 124, 116, 14),
    );
    fill_circle(&mut pixmap, center, center, radius, palette.wafer);
    fill_circle(
        &mut pixmap,
        center,
        center,
        radius * 0.82,
        palette.wafer_highlight,
    );
    stroke_line(
        &mut pixmap,
        center - radius,
        center,
        center + radius,
        center,
        palette.guide,
        1.0,
    );
    stroke_line(
        &mut pixmap,
        center,
        center - radius,
        center,
        center + radius,
        palette.guide,
        1.0,
    );

    for cell in &scene.cells {
        let pitch_width = (cell.size_mm[0] + scene.scribe_lane_mm[0]) as f32 * scale;
        let pitch_height = (cell.size_mm[1] + scene.scribe_lane_mm[1]) as f32 * scale;
        let lane_width = (scene.scribe_lane_mm[0] as f32 * scale)
            .max(1.5)
            .min((pitch_width - 0.4).max(0.0));
        let lane_height = (scene.scribe_lane_mm[1] as f32 * scale)
            .max(1.5)
            .min((pitch_height - 0.4).max(0.0));
        let width = (pitch_width - lane_width).max(0.0);
        let height = (pitch_height - lane_height).max(0.0);
        let x = center + cell.center_mm[0] as f32 * scale - width * 0.5;
        let y = center - cell.center_mm[1] as f32 * scale - height * 0.5;
        let color = match cell.tone {
            CellTone::Productive => palette.productive,
            CellTone::Defective => palette.defective,
            CellTone::Boundary => palette.boundary,
            CellTone::Excluded => palette.excluded,
        };
        if let Some(rect) = Rect::from_xywh(x, y, width, height) {
            let mut paint = skia_paint(color);
            paint.anti_alias = false;
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        }
    }

    let usable_radius = radius * (scene.usable_diameter_mm / scene.diameter_mm) as f32;
    stroke_circle(
        &mut pixmap,
        center,
        center,
        usable_radius,
        palette.usable_outline,
        1.8,
    );
    stroke_circle(
        &mut pixmap,
        center,
        center,
        radius,
        palette.wafer_outline,
        2.2,
    );
    stroke_line(
        &mut pixmap,
        center - 10.0,
        center + radius - 1.0,
        center,
        center + radius - 8.0,
        palette.wafer_outline,
        2.2,
    );
    stroke_line(
        &mut pixmap,
        center,
        center + radius - 8.0,
        center + 10.0,
        center + radius - 1.0,
        palette.wafer_outline,
        2.2,
    );

    pixmap
        .encode_png()
        .map_err(|error| ReportError::new(format!("unable to encode wafer-map image: {error}")))
}

fn metric_card(svg: &mut String, x: f32, label: &str, value: &str, detail: &str, accent: Color32) {
    let label = xml_text(label);
    let value = xml_text(value);
    let detail = xml_text(detail);
    push_fmt(
        svg,
        format_args!(
            r#"<rect x="{x}" y="148" width="268" height="110" rx="18" fill="{}" stroke="{}"/>
<text x="{}" y="181" font-size="12" fill="{}">{label}</text>
<text x="{}" y="222" font-size="31" fill="{}">{value}</text>
<text x="{}" y="246" font-size="13" fill="{}">{detail}</text>
"#,
            hex(theme::SURFACE),
            hex(theme::BORDER),
            x + 22.0,
            hex(accent),
            x + 22.0,
            hex(theme::TEXT),
            x + 22.0,
            hex(theme::TEXT_MUTED),
        ),
    );
}

fn legend_item(svg: &mut String, x: f32, y: f32, color: Color32, label: &str) {
    let label = xml_text(label);
    push_fmt(
        svg,
        format_args!(
            r#"<rect x="{x}" y="{}" width="10" height="10" rx="2" fill="{}"/>
<text x="{}" y="{y}" font-size="12" fill="{}">{label}</text>
"#,
            y - 9.0,
            hex(color),
            x + 17.0,
            hex(theme::TEXT_MUTED),
        ),
    );
}

#[derive(Clone, Copy)]
enum SectionMark {
    Wafer,
    Die,
    Alignment,
    Probe,
}

fn parameter_heading(svg: &mut String, x: f32, y: f32, label: &str, mark: SectionMark) {
    section_mark(svg, x + 8.0, y - 5.0, mark);
    let label = xml_text(label);
    push_fmt(
        svg,
        format_args!(
            r#"<text x="{}" y="{y}" font-size="12" fill="{}">{label}</text>
"#,
            x + 28.0,
            hex(theme::BLUE),
        ),
    );
}

fn parameter_row(svg: &mut String, x: f32, y: f32, label: &str, value: &str) {
    let label = xml_text(label);
    let value = xml_text(value);
    push_fmt(
        svg,
        format_args!(
            r#"<text x="{x}" y="{y}" font-size="13" fill="{}">{label}</text>
<text x="1156" y="{y}" text-anchor="end" font-size="13" fill="{}">{value}</text>
"#,
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT),
        ),
    );
}

fn section_mark(svg: &mut String, x: f32, y: f32, mark: SectionMark) {
    let color = hex(theme::BLUE);
    match mark {
        SectionMark::Wafer => push_fmt(
            svg,
            format_args!(
                r#"<circle cx="{x}" cy="{y}" r="7" fill="none" stroke="{color}" stroke-width="1.3"/>
<path d="M {} {} L {x} {} L {} {}" fill="none" stroke="{color}" stroke-width="1.3"/>
"#,
                x - 2.2,
                y + 5.2,
                y + 3.3,
                x + 2.2,
                y + 5.2,
            ),
        ),
        SectionMark::Die => {
            for (dx, dy) in [(-5.5, -5.5), (1.0, -5.5), (-5.5, 1.0), (1.0, 1.0)] {
                push_fmt(
                    svg,
                    format_args!(
                        r#"<rect x="{}" y="{}" width="4.5" height="4.5" rx="0.8" fill="none" stroke="{color}" stroke-width="1.1"/>
"#,
                        x + dx,
                        y + dy,
                    ),
                );
            }
        }
        SectionMark::Alignment => push_fmt(
            svg,
            format_args!(
                r#"<circle cx="{x}" cy="{y}" r="5" fill="none" stroke="{color}" stroke-width="1.2"/>
<path d="M {} {y} H {} M {x} {} V {}" stroke="{color}" stroke-width="1.2"/>
"#,
                x - 8.0,
                x + 8.0,
                y - 8.0,
                y + 8.0,
            ),
        ),
        SectionMark::Probe => {
            for dy in [-4.0, 0.0, 4.0] {
                for dx in [-4.0, 0.0, 4.0] {
                    push_fmt(
                        svg,
                        format_args!(
                            r#"<circle cx="{}" cy="{}" r="1.2" fill="{color}"/>
"#,
                            x + dx,
                            y + dy,
                        ),
                    );
                }
            }
        }
    }
}

fn summary_value(svg: &mut String, x: f32, y: f32, label: &str, value: u64) {
    let label = xml_text(label);
    push_fmt(
        svg,
        format_args!(
            r#"<text x="{x}" y="{y}" font-size="12" fill="{}">{label}</text>
<text x="{x}" y="{}" font-size="28" fill="{}">{}</text>
"#,
            hex(theme::BLUE),
            y + 38.0,
            hex(theme::TEXT),
            format_integer(value),
        ),
    );
}

fn measurement(value: f64, unit: &str) -> String {
    format!("{} {unit}", compact(value, 3))
}

fn compact(value: f64, precision: usize) -> String {
    let mut formatted = format!("{value:.precision$}");
    if formatted.contains('.') {
        while formatted.ends_with('0') {
            formatted.pop();
        }
        if formatted.ends_with('.') {
            formatted.pop();
        }
    }
    formatted
}

fn format_integer(value: u64) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(character);
    }
    formatted
}

fn fill_circle(pixmap: &mut Pixmap, cx: f32, cy: f32, radius: f32, color: Color32) {
    let mut builder = PathBuilder::new();
    builder.push_circle(cx, cy, radius);
    if let Some(path) = builder.finish() {
        pixmap.fill_path(
            &path,
            &skia_paint(color),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn stroke_circle(pixmap: &mut Pixmap, cx: f32, cy: f32, radius: f32, color: Color32, width: f32) {
    let mut builder = PathBuilder::new();
    builder.push_circle(cx, cy, radius);
    if let Some(path) = builder.finish() {
        let stroke = Stroke {
            width,
            ..Stroke::default()
        };
        pixmap.stroke_path(
            &path,
            &skia_paint(color),
            &stroke,
            Transform::identity(),
            None,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn stroke_line(
    pixmap: &mut Pixmap,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    color: Color32,
    width: f32,
) {
    let mut builder = PathBuilder::new();
    builder.move_to(x1, y1);
    builder.line_to(x2, y2);
    if let Some(path) = builder.finish() {
        let stroke = Stroke {
            width,
            ..Stroke::default()
        };
        pixmap.stroke_path(
            &path,
            &skia_paint(color),
            &stroke,
            Transform::identity(),
            None,
        );
    }
}

fn skia_paint(color: Color32) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r(), color.g(), color.b(), color.a());
    paint
}

fn skia_color(color: Color32) -> resvg::tiny_skia::Color {
    resvg::tiny_skia::Color::from_rgba8(color.r(), color.g(), color.b(), color.a())
}

fn hex(color: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

fn rgba_hex(color: Color32, alpha: u8) -> String {
    format!(
        "#{:02x}{:02x}{:02x}{alpha:02x}",
        color.r(),
        color.g(),
        color.b()
    )
}

fn xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn push_fmt(output: &mut String, arguments: fmt::Arguments<'_>) {
    output
        .write_fmt(arguments)
        .expect("writing formatted content into a String cannot fail");
}

#[cfg(test)]
mod tests {
    use super::*;
    use die_yield_core::{FabricationInputs, analyze};

    #[test]
    fn report_formats_share_the_same_styled_snapshot() {
        let inputs = FabricationInputs::default();
        let analysis = analyze(&inputs).expect("default analysis should be valid");

        let svg = generate(&inputs, &analysis, ReportFormat::Svg).expect("SVG report");
        assert!(svg.bytes.starts_with(b"<svg"));
        let svg_text = String::from_utf8(svg.bytes).expect("generated SVG should be UTF-8");
        assert!(svg_text.contains("300 mm (12 in)"));
        assert!(svg_text.contains("data:image/png;base64,"));
        assert!(svg_text.contains("Column scribe"));

        let png = generate(&inputs, &analysis, ReportFormat::Png).expect("PNG report");
        assert!(png.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(
            u32::from_be_bytes(png.bytes[16..20].try_into().expect("width")),
            2480
        );
        assert_eq!(
            u32::from_be_bytes(png.bytes[20..24].try_into().expect("height")),
            3508
        );

        let pdf = generate(&inputs, &analysis, ReportFormat::Pdf).expect("PDF report");
        assert!(pdf.bytes.starts_with(b"%PDF-"));
        assert!(pdf.bytes.len() > 50_000);
    }

    #[test]
    fn wafer_map_export_handles_zero_and_asymmetric_scribe_lanes() {
        let mut inputs = FabricationInputs::default();
        inputs.die.column_lane_mm = 0.0;
        inputs.die.row_lane_mm = 0.001;
        inputs.process.defect_density_cm2 = 5.0;
        let analysis = analyze(&inputs).expect("report matrix input should be valid");

        let image = render_map_png(&analysis).expect("wafer map should render");
        assert!(image.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(image.len() > 10_000);
    }

    #[test]
    fn report_template_accepts_wafer_and_yield_matrix() {
        for (diameter, density, column_lane, row_lane) in [
            (76.0, 0.0, 0.0, 0.002),
            (150.0, 0.1, 0.001, 0.25),
            (300.0, 1.0, 0.12, 0.12),
            (450.0, 5.0, 1.0, 0.0),
        ] {
            let mut inputs = FabricationInputs::default();
            inputs.wafer.diameter_mm = diameter;
            inputs.wafer.edge_exclusion_mm = (diameter * 0.02).max(1.0);
            inputs.process.defect_density_cm2 = density;
            inputs.die.column_lane_mm = column_lane;
            inputs.die.row_lane_mm = row_lane;
            let analysis = analyze(&inputs).expect("report matrix input should be valid");

            let svg = build_svg(&inputs, &analysis).expect("report template should render");
            assert!(svg.contains(&wafer_size_label(diameter)));
            assert!(svg.contains(&format!("{:.2}%", analysis.summary.yield_fraction * 100.0)));
            assert!(svg.len() > 100_000);
        }
    }

    #[test]
    #[ignore = "manual visual audit helper"]
    fn writes_report_previews_when_requested() {
        let output_dir = std::env::var("DIE_YIELD_REPORT_PREVIEW_DIR")
            .expect("set DIE_YIELD_REPORT_PREVIEW_DIR for a manual report audit");
        let inputs = FabricationInputs::default();
        let analysis = analyze(&inputs).expect("default analysis should be valid");

        for format in [ReportFormat::Png, ReportFormat::Svg, ReportFormat::Pdf] {
            let file = generate(&inputs, &analysis, format).expect("report should generate");
            let path = std::path::Path::new(&output_dir).join(file.filename);
            std::fs::write(path, file.bytes).expect("report preview should be written");
        }
    }
}
