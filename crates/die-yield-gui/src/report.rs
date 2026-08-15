//! Shared, in-memory report generation for native and browser exports.

use crate::{app::wafer_size_label, theme};
use base64::prelude::*;
use die_yield_core::{FabricationInputs, WaferAnalysis, YieldModel};
use die_yield_render::{CellTone, WaferPalette, WaferScene};
use egui::Color32;
use resvg::tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};
use std::{fmt, fmt::Write as _};

const REPORT_WIDTH: u32 = 1240;
const REPORT_HEIGHT: u32 = 1754;
const PNG_SCALE: u32 = 2;
const MAP_PIXELS: u32 = 800;
const CALCULATOR_PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
const GEOMETRY_METHOD_VERSION: &str = "gross-active-rectangle-grid/v1";
const GROSS_BOUNDARY_POLICY: &str =
    "complete active rectangle must fit inside usable radius (inclusive)";
const PITCH_POLICY: &str = "active dimensions plus scribe";
const RADIAL_EDGE_POLICY: &str = "usable radius equals wafer radius minus radial edge exclusion";
const WAFER_SHAPE_POLICY: &str = "circular wafer";
const NOTCH_POLICY: &str = "ignored by geometry";
const YIELD_AREA_POLICY: &str = "active die area only";
const PHASE_POLICY: &str = "normalized modulo one pitch into [-pitch/2, pitch/2)";

/// Available report file formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportFormat {
    /// High-resolution raster report.
    Png,
    /// Portable vector report with an embedded font and wafer-map image.
    Svg,
    /// A4 print-ready document.
    Pdf,
    /// Compact deterministic machine-readable analysis snapshot.
    Json,
}

impl ReportFormat {
    /// File extension without a leading dot.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
            Self::Pdf => "pdf",
            Self::Json => "json",
        }
    }

    /// Internet media type for downloads.
    #[must_use]
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Svg => "image/svg+xml",
            Self::Pdf => "application/pdf",
            Self::Json => "application/json",
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
    let bytes = match format {
        ReportFormat::Svg => build_svg(inputs, analysis)?.into_bytes(),
        ReportFormat::Png => svg_to_png(&build_svg(inputs, analysis)?)?,
        ReportFormat::Pdf => svg_to_pdf(&build_svg(inputs, analysis)?)?,
        ReportFormat::Json => build_json_snapshot(analysis).into_bytes(),
    };

    Ok(ReportFile {
        filename: format!("yield-studio-report.{}", format.extension()),
        mime_type: format.mime_type(),
        bytes,
    })
}

fn build_json_snapshot(analysis: &WaferAnalysis) -> String {
    let inputs = analysis.normalized_inputs;
    let summary = analysis.summary;
    let mut json = String::with_capacity(2_400);
    push_fmt(
        &mut json,
        format_args!(
            concat!(
                "{{\n",
                "  \"schema_version\": \"die-yield-report/v1\",\n",
                "  \"calculation_metadata\": {{\n",
                "    \"calculator_package_version\": \"{}\",\n",
                "    \"geometry_method_version\": \"{}\",\n",
                "    \"gross_boundary_policy\": \"{}\",\n",
                "    \"pitch_policy\": \"{}\",\n",
                "    \"radial_edge_policy\": \"{}\",\n",
                "    \"wafer_shape_policy\": \"{}\",\n",
                "    \"notch_policy\": \"{}\",\n",
                "    \"yield_area_policy\": \"{}\",\n",
                "    \"phase_policy\": \"{}\"\n",
                "  }},\n",
                "  \"normalized_inputs\": {{\n",
                "    \"wafer_diameter_mm\": {},\n",
                "    \"edge_exclusion_mm\": {},\n",
                "    \"die_active_width_mm\": {},\n",
                "    \"die_active_height_mm\": {},\n",
                "    \"column_scribe_mm\": {},\n",
                "    \"row_scribe_mm\": {},\n",
                "    \"defect_density_per_cm2\": {},\n",
                "    \"yield_model\": \"{}\",\n",
                "    \"clustering_alpha\": {},\n",
                "    \"horizontal_phase_mm\": {},\n",
                "    \"vertical_phase_mm\": {},\n",
                "    \"die_at_origin\": {},\n",
                "    \"probe_columns_per_step\": {},\n",
                "    \"probe_rows_per_step\": {}\n",
                "  }},\n",
                "  \"results\": {{\n",
                "    \"gross_dies_per_wafer\": {},\n",
                "    \"partial_boundary_sites\": {},\n",
                "    \"edge_band_sites\": {},\n",
                "    \"yield_area_mm2\": {},\n",
                "    \"defect_exposure_ad0\": {},\n",
                "    \"yield_fraction\": {},\n",
                "    \"expected_good_exact\": {},\n",
                "    \"expected_defective_exact\": {},\n",
                "    \"expected_good_rounded\": {},\n",
                "    \"displayed_defective\": {},\n",
                "    \"good_expectation_rounding\": \"nearest_whole_die\",\n",
                "    \"displayed_defective_policy\": \"gross_minus_rounded_good\"\n",
                "  }},\n",
                "  \"probe_summary\": {{\n",
                "    \"idealized_touchdown_count\": {},\n",
                "    \"sites_per_touchdown\": {}\n",
                "  }},\n",
                "  \"map_semantics\": {{\n",
                "    \"statistical_loss_locations\": \"illustrative_not_predicted\",\n",
                "    \"notch_subtracted_from_geometry\": false\n",
                "  }}\n",
                "}}\n"
            ),
            CALCULATOR_PACKAGE_VERSION,
            GEOMETRY_METHOD_VERSION,
            GROSS_BOUNDARY_POLICY,
            PITCH_POLICY,
            RADIAL_EDGE_POLICY,
            WAFER_SHAPE_POLICY,
            NOTCH_POLICY,
            YIELD_AREA_POLICY,
            PHASE_POLICY,
            json_number(inputs.wafer.diameter_mm),
            json_number(inputs.wafer.edge_exclusion_mm),
            json_number(inputs.die.width_mm),
            json_number(inputs.die.height_mm),
            json_number(inputs.die.column_lane_mm),
            json_number(inputs.die.row_lane_mm),
            json_number(inputs.process.defect_density_cm2),
            yield_model_key(inputs.process.yield_model),
            json_number(inputs.process.clustering_alpha),
            json_number(inputs.process.offset_x_mm),
            json_number(inputs.process.offset_y_mm),
            inputs.process.die_at_origin,
            inputs.probe.columns,
            inputs.probe.rows,
            summary.geometric_usable,
            summary.partial,
            summary.edge_excluded,
            json_number(summary.yield_area_mm2),
            json_number(summary.defect_exposure),
            json_number(summary.yield_fraction),
            json_number(summary.expected_good_exact),
            json_number(summary.expected_defective_exact),
            summary.expected_good,
            summary.expected_defective,
            analysis.probe.touchdown_count,
            analysis.probe.sites_per_touchdown,
        ),
    );
    json
}

/// Builds the styled SVG source also used by the browser print view.
pub fn build_svg(
    _inputs: &FabricationInputs,
    analysis: &WaferAnalysis,
) -> Result<String, ReportError> {
    let inputs = &analysis.normalized_inputs;
    let map_png = render_map_png(analysis)?;
    let map_data = BASE64_STANDARD.encode(map_png);
    let font_data = BASE64_STANDARD.encode(epaint_default_fonts::UBUNTU_LIGHT);
    let summary = analysis.summary;
    let scene = WaferScene::from_analysis(analysis);
    let palette = WaferPalette::default();
    let model = inputs.process.yield_model;
    let model_label = yield_model_label(model);
    let model_reading = xml_text(&yield_model_reading(model, inputs.process.clustering_alpha));
    let expectation_reading = xml_text(&format!(
        "Expected good is shown as ≈{}; full precision {:.17}. The approximation rounds to the nearest whole die.",
        format_integer(summary.expected_good),
        summary.expected_good_exact,
    ));
    let calculation_identity = xml_text(&format!(
        "Calculator package version: {CALCULATOR_PACKAGE_VERSION} · Geometry method version: {GEOMETRY_METHOD_VERSION}"
    ));
    let gross_boundary_policy =
        xml_text(&format!("Gross boundary policy: {GROSS_BOUNDARY_POLICY}"));
    let pitch_policy = xml_text(&format!("Pitch policy: {PITCH_POLICY}"));
    let radial_edge_policy = xml_text(&format!("Radial edge policy: {RADIAL_EDGE_POLICY}"));
    let wafer_and_yield_policy = xml_text(&format!(
        "Wafer shape policy: {WAFER_SHAPE_POLICY} · Notch policy: {NOTCH_POLICY} · Yield area policy: {YIELD_AREA_POLICY}"
    ));
    let phase_and_map_policy = xml_text(&format!(
        "Phase policy: {PHASE_POLICY} · Map loss locations are illustrative, not predicted"
    ));

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
        "GROSS DIES / WAFER",
        &format_integer(summary.geometric_usable),
        "Complete sites before modeled defects",
        theme::BLUE,
    );
    metric_card(
        &mut svg,
        342.0,
        "ESTIMATED DIE YIELD",
        &format!("{:.2}%", summary.yield_fraction * 100.0),
        &format!("{model_label} model"),
        theme::ACCENT,
    );
    metric_card(
        &mut svg,
        628.0,
        "EXPECTED GOOD / WAFER",
        &format!("≈{}", format_integer(summary.expected_good)),
        &format!("full precision {:.17}", summary.expected_good_exact),
        theme::CORAL,
    );
    metric_card(
        &mut svg,
        914.0,
        "IDEALIZED TOUCHDOWNS",
        &format_integer(analysis.probe.touchdown_count),
        &format!("{} sites per step", analysis.probe.sites_per_touchdown),
        theme::AMBER,
    );

    push_fmt(
        &mut svg,
        format_args!(
            r#"<rect x="56" y="288" width="724" height="770" rx="20" fill="{}" stroke="{}"/>
<text x="88" y="330" font-size="22" fill="{}">Wafer map</text>
<text x="88" y="355" font-size="14" fill="{}">Geometric placement with illustrative random loss</text>
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
    legend_item(&mut svg, 88.0, 1027.0, palette.productive, "Gross die");
    legend_item(
        &mut svg,
        198.0,
        1027.0,
        palette.defective,
        "Illustrative loss",
    );
    legend_item(
        &mut svg,
        342.0,
        1027.0,
        palette.boundary,
        "Partial boundary",
    );
    legend_item(&mut svg, 486.0, 1027.0, palette.excluded, "Edge band");
    legend_item(&mut svg, 590.0, 1027.0, palette.scribe, "Scribe lane");

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
        &dimension_mm(inputs.wafer.diameter_mm),
    );
    parameter_row(
        &mut svg,
        832.0,
        460.0,
        "Edge exclusion",
        &dimension_mm(inputs.wafer.edge_exclusion_mm),
    );

    parameter_heading(&mut svg, 832.0, 510.0, "DIE & SCRIBE", SectionMark::Die);
    parameter_row(
        &mut svg,
        832.0,
        544.0,
        "Active width",
        &dimension_mm(inputs.die.width_mm),
    );
    parameter_row(
        &mut svg,
        832.0,
        580.0,
        "Active height",
        &dimension_mm(inputs.die.height_mm),
    );
    parameter_row(
        &mut svg,
        832.0,
        616.0,
        "Column scribe",
        &scribe_dimension(inputs.die.column_lane_mm),
    );
    parameter_row(
        &mut svg,
        832.0,
        652.0,
        "Row scribe",
        &scribe_dimension(inputs.die.row_lane_mm),
    );

    parameter_heading(
        &mut svg,
        832.0,
        694.0,
        "YIELD & ALIGNMENT",
        SectionMark::Alignment,
    );
    parameter_row(
        &mut svg,
        832.0,
        724.0,
        "Defect density",
        &format!("{:.6} /cm²", inputs.process.defect_density_cm2),
    );
    parameter_row(&mut svg, 832.0, 756.0, "Yield model", model_label);
    parameter_row(
        &mut svg,
        832.0,
        788.0,
        "Clustering alpha (α)",
        &clustering_alpha_value(model, inputs.process.clustering_alpha),
    );
    parameter_row(
        &mut svg,
        832.0,
        820.0,
        "Horizontal phase",
        &dimension_mm(inputs.process.offset_x_mm),
    );
    parameter_row(
        &mut svg,
        832.0,
        852.0,
        "Vertical phase",
        &dimension_mm(inputs.process.offset_y_mm),
    );
    parameter_row(
        &mut svg,
        832.0,
        884.0,
        "Die at origin",
        if inputs.process.die_at_origin {
            "Yes"
        } else {
            "No"
        },
    );

    parameter_heading(&mut svg, 832.0, 926.0, "PROBE ARRAY", SectionMark::Probe);
    parameter_row(
        &mut svg,
        832.0,
        958.0,
        "Columns per step",
        &inputs.probe.columns.to_string(),
    );
    parameter_row(
        &mut svg,
        832.0,
        990.0,
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
        1158.0,
        "GROSS DIES",
        summary.geometric_usable,
    );
    summary_value(
        &mut svg,
        358.0,
        1158.0,
        "ILLUSTRATIVE LOSS",
        summary.expected_defective,
    );
    summary_value(&mut svg, 628.0, 1158.0, "PARTIAL BOUNDARY", summary.partial);
    summary_value(&mut svg, 898.0, 1158.0, "EDGE BAND", summary.edge_excluded);
    push_fmt(
        &mut svg,
        format_args!(
            r#"<line x1="88" y1="1228" x2="1152" y2="1228" stroke="{}"/>
<text x="88" y="1262" font-size="13" fill="{}">Yield area A</text>
<text x="182" y="1262" font-size="13" fill="{}">{:.6} mm² = {:.8} cm²</text>
<text x="558" y="1262" font-size="13" fill="{}">A·D₀</text>
<text x="614" y="1262" font-size="13" fill="{}">{:.10}</text>
<text x="850" y="1262" font-size="13" fill="{}">Usable diameter</text>
<text x="988" y="1262" font-size="13" fill="{}">{:.6} mm</text>
<text x="88" y="1298" font-size="13" fill="{}">Grid pitch</text>
<text x="182" y="1298" font-size="13" fill="{}">{:.6} × {:.6} mm</text>
<text x="558" y="1298" font-size="13" fill="{}">Yield basis</text>
<text x="650" y="1298" font-size="13" fill="{}">active die area only</text>
"#,
            hex(theme::BORDER),
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT),
            summary.yield_area_mm2,
            summary.yield_area_mm2 / 100.0,
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT),
            summary.defect_exposure,
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT),
            scene.usable_diameter_mm,
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT),
            inputs.die.width_mm + inputs.die.column_lane_mm,
            inputs.die.height_mm + inputs.die.row_lane_mm,
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT),
        ),
    );

    push_fmt(
        &mut svg,
        format_args!(
            r#"<rect x="56" y="1348" width="1128" height="330" rx="20" fill="{}" stroke="{}"/>
<text x="88" y="1384" font-size="18" fill="{}">Reading the report</text>
<rect x="88" y="1410" width="4" height="252" rx="2" fill="{}"/>
<text x="112" y="1432" font-size="15" fill="{}">Model</text>
<text x="112" y="1454" font-size="13" fill="{}">{model_reading}</text>
<text x="112" y="1482" font-size="15" fill="{}">Expectation</text>
<text x="112" y="1504" font-size="13" fill="{}">{expectation_reading}</text>
<text x="112" y="1532" font-size="15" fill="{}">Calculation basis</text>
<text x="112" y="1556" font-size="11.5" fill="{}">{calculation_identity}</text>
<text x="112" y="1577" font-size="11.5" fill="{}">{gross_boundary_policy}</text>
<text x="112" y="1598" font-size="11.5" fill="{}">{pitch_policy}</text>
<text x="112" y="1619" font-size="11.5" fill="{}">{radial_edge_policy}</text>
<text x="112" y="1640" font-size="11.5" fill="{}">{wafer_and_yield_policy}</text>
<text x="112" y="1661" font-size="11.5" fill="{}">{phase_and_map_policy}</text>
<text x="56" y="1697" font-size="13" fill="{}">Planning estimate only. Validate production decisions with process-specific characterization.</text>
<line x1="56" y1="1714" x2="1184" y2="1714" stroke="{}"/>
<text x="56" y="1742" font-size="13" fill="{}">Yield Studio · Wafer planning, without the spreadsheet</text>
<text x="1184" y="1742" text-anchor="end" font-size="13" fill="{}">A4 portrait · 150 dpi reference scale</text>
</svg>"#,
            hex(theme::SURFACE),
            hex(theme::BORDER),
            hex(theme::TEXT),
            hex(theme::ACCENT),
            hex(theme::BLUE),
            hex(theme::TEXT_MUTED),
            hex(theme::BLUE),
            hex(theme::TEXT_MUTED),
            hex(theme::BLUE),
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT_MUTED),
            hex(theme::TEXT_MUTED),
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

fn yield_model_label(model: YieldModel) -> &'static str {
    match model {
        YieldModel::Poisson => "Poisson",
        YieldModel::MurphyTriangular => "Murphy triangular",
        YieldModel::Seeds => "Seeds",
        YieldModel::NegativeBinomial => "Negative binomial",
    }
}

fn yield_model_key(model: YieldModel) -> &'static str {
    match model {
        YieldModel::Poisson => "poisson",
        YieldModel::MurphyTriangular => "murphy_triangular",
        YieldModel::Seeds => "seeds",
        YieldModel::NegativeBinomial => "negative_binomial",
    }
}

fn json_number(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.17}")
    } else {
        "null".to_owned()
    }
}

fn yield_model_reading(model: YieldModel, clustering_alpha: f64) -> String {
    match model {
        YieldModel::Poisson => {
            "Poisson: Y = exp(−A·D₀); assumes independent, uniformly random defects.".to_owned()
        }
        YieldModel::MurphyTriangular => {
            "Murphy triangular: Y = [(1 − exp(−A·D₀)) / (A·D₀)]².".to_owned()
        }
        YieldModel::Seeds => {
            "Seeds: Y = 1 / (1 + A·D₀); equivalent to negative binomial with α = 1.".to_owned()
        }
        YieldModel::NegativeBinomial => {
            format!("Negative binomial: Y = (1 + A·D₀ / α)^(−α), with α = {clustering_alpha:.6}.")
        }
    }
}

fn clustering_alpha_value(model: YieldModel, clustering_alpha: f64) -> String {
    if model == YieldModel::NegativeBinomial {
        format!("{clustering_alpha:.6}")
    } else {
        "not used".to_owned()
    }
}

fn dimension_mm(value: f64) -> String {
    format!("{value:.6} mm")
}

fn scribe_dimension(value_mm: f64) -> String {
    format!("{:.6} µm", value_mm * 1_000.0)
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
        assert!(svg_text.contains("GROSS DIES / WAFER"));
        assert!(svg_text.contains("Murphy triangular model"));
        assert!(svg_text.contains("300.000000 mm"));
        assert!(svg_text.contains("120.000000 µm"));

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

        let json = generate(&inputs, &analysis, ReportFormat::Json).expect("JSON report");
        assert_eq!(json.filename, "yield-studio-report.json");
        assert_eq!(json.mime_type, "application/json");
        assert!(json.bytes.starts_with(b"{\n"));
        assert!(json.bytes.ends_with(b"}\n"));
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
    fn report_supports_all_selected_yield_models() {
        for (model, alpha) in [
            (YieldModel::Poisson, 1.0),
            (YieldModel::MurphyTriangular, 1.0),
            (YieldModel::Seeds, 1.0),
            (YieldModel::NegativeBinomial, 2.345_678),
        ] {
            let mut inputs = FabricationInputs::default();
            inputs.process.yield_model = model;
            inputs.process.clustering_alpha = alpha;
            let analysis = analyze(&inputs).expect("model report input should be valid");

            let svg = build_svg(&inputs, &analysis).expect("model report should render");
            let model_label = yield_model_label(model);
            assert!(svg.contains(&format!("{model_label} model")));
            assert!(svg.contains(&yield_model_reading(model, alpha)));
            assert!(svg.contains(&format!("{:.2}%", analysis.summary.yield_fraction * 100.0)));
            assert!(svg.contains(&format!(
                "full precision {:.17}",
                analysis.summary.expected_good_exact
            )));

            let json = build_json_snapshot(&analysis);
            assert!(json.contains(&format!("\"yield_model\": \"{}\"", yield_model_key(model))));
        }
    }

    #[test]
    fn report_semantics_and_precision_match_the_analysis() {
        let mut inputs = FabricationInputs::default();
        inputs.wafer.diameter_mm = 200.123_456;
        inputs.wafer.edge_exclusion_mm = 2.123_456;
        inputs.die.width_mm = 10.123_456;
        inputs.die.height_mm = 8.654_321;
        inputs.die.column_lane_mm = 0.001_234;
        inputs.die.row_lane_mm = 0.000_007;
        inputs.process.defect_density_cm2 = 0.234_567;
        inputs.process.yield_model = YieldModel::NegativeBinomial;
        inputs.process.clustering_alpha = 2.345_678;
        inputs.process.offset_x_mm = 0.123_456;
        inputs.process.offset_y_mm = -0.654_321;
        let analysis = analyze(&inputs).expect("precision report input should be valid");
        let summary = analysis.summary;

        let svg = build_svg(&inputs, &analysis).expect("precision report should render");
        assert!(svg.contains(&dimension_mm(inputs.wafer.diameter_mm)));
        assert!(svg.contains(&dimension_mm(inputs.wafer.edge_exclusion_mm)));
        assert!(svg.contains(&dimension_mm(inputs.die.width_mm)));
        assert!(svg.contains(&dimension_mm(inputs.die.height_mm)));
        assert!(svg.contains(&scribe_dimension(inputs.die.column_lane_mm)));
        assert!(svg.contains(&scribe_dimension(inputs.die.row_lane_mm)));
        assert!(svg.contains(&dimension_mm(inputs.process.offset_x_mm)));
        assert!(svg.contains(&dimension_mm(inputs.process.offset_y_mm)));
        assert!(svg.contains(&format!("{:.6} mm²", summary.yield_area_mm2)));
        assert!(svg.contains(&format!("{:.8} cm²", summary.yield_area_mm2 / 100.0)));
        assert!(svg.contains(&format!("{:.10}", summary.defect_exposure)));
        assert!(svg.contains(&format_integer(summary.geometric_usable)));
        assert!(svg.contains(&format_integer(summary.expected_defective)));
        assert!(svg.contains(&format!("≈{}", format_integer(summary.expected_good))));
        assert!(svg.contains(&format!(
            "full precision {:.17}",
            summary.expected_good_exact
        )));
        assert!(svg.contains("rounds to the nearest whole die"));
        assert!(svg.contains("ILLUSTRATIVE LOSS"));
        assert!(svg.contains("PARTIAL BOUNDARY"));
        assert!(svg.contains("Map loss locations are illustrative, not predicted"));
        assert!(svg.contains(&format!("Notch policy: {NOTCH_POLICY}")));
        assert!(svg.contains("IDEALIZED TOUCHDOWNS"));
        assert!(svg.contains("active die area only"));
        assert!(!svg.contains("DIE LOSS"));
    }

    #[test]
    fn json_snapshot_is_deterministic_and_uses_normalized_inputs() {
        let mut inputs = FabricationInputs::default();
        let pitch_x = inputs.die.width_mm + inputs.die.column_lane_mm;
        inputs.process.offset_x_mm = 3.0 * pitch_x + 0.123_456;
        inputs.process.offset_y_mm = -0.654_321;
        inputs.process.yield_model = YieldModel::NegativeBinomial;
        inputs.process.clustering_alpha = 2.345_678;
        let analysis = analyze(&inputs).expect("JSON snapshot input should be valid");

        let first = build_json_snapshot(&analysis);
        let second = build_json_snapshot(&analysis);
        assert_eq!(first, second);
        assert!(first.contains("\"schema_version\": \"die-yield-report/v1\""));
        assert!(first.contains(&format!(
            "\"horizontal_phase_mm\": {}",
            json_number(analysis.normalized_inputs.process.offset_x_mm)
        )));
        assert!(first.contains("\"yield_model\": \"negative_binomial\""));
        assert!(first.contains(&format!(
            "\"clustering_alpha\": {}",
            json_number(analysis.normalized_inputs.process.clustering_alpha)
        )));
        assert!(first.contains(&format!(
            "\"gross_dies_per_wafer\": {}",
            analysis.summary.geometric_usable
        )));
        assert!(first.contains(&format!(
            "\"yield_fraction\": {}",
            json_number(analysis.summary.yield_fraction)
        )));
        assert!(first.contains(&format!(
            "\"expected_good_exact\": {}",
            json_number(analysis.summary.expected_good_exact)
        )));
        assert!(first.contains(&format!(
            "\"expected_good_rounded\": {}",
            analysis.summary.expected_good
        )));
        assert!(first.contains(&format!(
            "\"idealized_touchdown_count\": {}",
            analysis.probe.touchdown_count
        )));
        assert!(first.contains(&format!(
            "\"displayed_defective\": {}",
            analysis.summary.expected_defective
        )));
        assert!(first.contains("\"good_expectation_rounding\": \"nearest_whole_die\""));
        assert!(first.contains("\"displayed_defective_policy\": \"gross_minus_rounded_good\""));
        assert!(!first.contains("\"expected_defective_rounded\""));
        assert!(first.contains("\"statistical_loss_locations\": \"illustrative_not_predicted\""));
        assert!(first.contains("\"notch_subtracted_from_geometry\": false"));
    }

    #[test]
    fn whole_pitch_offsets_have_canonical_visual_and_json_reports() {
        let canonical_inputs = FabricationInputs::default();
        let canonical_analysis =
            analyze(&canonical_inputs).expect("canonical report input should be valid");
        let mut shifted_inputs = canonical_inputs;
        let pitch_x = shifted_inputs.die.width_mm + shifted_inputs.die.column_lane_mm;
        let pitch_y = shifted_inputs.die.height_mm + shifted_inputs.die.row_lane_mm;
        shifted_inputs.process.offset_x_mm = 3.0 * pitch_x;
        shifted_inputs.process.offset_y_mm = -2.0 * pitch_y;
        let shifted_analysis =
            analyze(&shifted_inputs).expect("whole-pitch report input should be valid");

        assert_eq!(shifted_analysis.normalized_inputs.process.offset_x_mm, 0.0);
        assert_eq!(shifted_analysis.normalized_inputs.process.offset_y_mm, 0.0);
        assert_eq!(
            build_json_snapshot(&canonical_analysis),
            build_json_snapshot(&shifted_analysis)
        );
        assert_eq!(
            build_svg(&canonical_inputs, &canonical_analysis).expect("canonical SVG report"),
            build_svg(&shifted_inputs, &shifted_analysis).expect("whole-pitch SVG report")
        );
    }

    #[test]
    fn calculation_metadata_is_shared_by_json_and_visual_reports() {
        let inputs = FabricationInputs::default();
        let analysis = analyze(&inputs).expect("metadata report input should be valid");
        let json = build_json_snapshot(&analysis);
        let svg = build_svg(&inputs, &analysis).expect("metadata SVG report should render");

        for (key, value) in [
            ("calculator_package_version", CALCULATOR_PACKAGE_VERSION),
            ("geometry_method_version", GEOMETRY_METHOD_VERSION),
            ("gross_boundary_policy", GROSS_BOUNDARY_POLICY),
            ("pitch_policy", PITCH_POLICY),
            ("radial_edge_policy", RADIAL_EDGE_POLICY),
            ("wafer_shape_policy", WAFER_SHAPE_POLICY),
            ("notch_policy", NOTCH_POLICY),
            ("yield_area_policy", YIELD_AREA_POLICY),
            ("phase_policy", PHASE_POLICY),
        ] {
            assert!(json.contains(&format!("\"{key}\": \"{value}\"")));
            assert!(svg.contains(&xml_text(value)));
        }
        assert!(!json.contains("\"commit"));
    }

    #[test]
    fn report_format_metadata_covers_machine_readable_json() {
        assert_eq!(ReportFormat::Json.extension(), "json");
        assert_eq!(ReportFormat::Json.mime_type(), "application/json");
    }

    #[test]
    #[ignore = "manual visual audit helper"]
    fn writes_report_previews_when_requested() {
        let output_dir = std::env::var("DIE_YIELD_REPORT_PREVIEW_DIR")
            .expect("set DIE_YIELD_REPORT_PREVIEW_DIR for a manual report audit");
        let inputs = FabricationInputs::default();
        let analysis = analyze(&inputs).expect("default analysis should be valid");

        for format in [
            ReportFormat::Png,
            ReportFormat::Svg,
            ReportFormat::Pdf,
            ReportFormat::Json,
        ] {
            let file = generate(&inputs, &analysis, format).expect("report should generate");
            let path = std::path::Path::new(&output_dir).join(file.filename);
            std::fs::write(path, file.bytes).expect("report preview should be written");
        }
    }
}
