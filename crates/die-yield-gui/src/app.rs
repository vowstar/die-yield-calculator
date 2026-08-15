use crate::{delivery, report, theme};
use die_yield_core::{
    FabricationInputs, InputField, ValidationErrors, WaferAnalysis, YieldModel, analyze,
};
use die_yield_render::{MIN_VISIBLE_SCRIBE_POINTS, WaferPalette, WaferScene, paint_wafer};
use eframe::egui;
use egui::{Align, Color32, Layout, Margin, RichText, Stroke, vec2};
use serde::{Deserialize, Serialize};

const WIDE_LAYOUT_THRESHOLD: f32 = 990.0;
const HEADER_CONTROL_HEIGHT: f32 = 38.0;
const PROJECT_URL: &str = "https://github.com/vowstar/die-yield-calculator";
pub(crate) const DEFECT_DENSITY_POLICY: &str = "effective full-process random-fatal-defect density; baseline/per-mask values needing a separate process-complexity factor are unsupported";
const WAFER_PRESETS_MM: [f64; 8] = [76.0, 100.0, 125.0, 150.0, 200.0, 300.0, 330.0, 450.0];
const NOMINAL_WAFER_INCHES: [(f64, u16); 10] = [
    (50.0, 2),
    (75.0, 3),
    (76.0, 3),
    (100.0, 4),
    (125.0, 5),
    (150.0, 6),
    (200.0, 8),
    (300.0, 12),
    (330.0, 13),
    (450.0, 18),
];

#[derive(Clone, Copy)]
enum SectionGlyph {
    Wafer,
    DieGrid,
    Alignment,
    ProbeArray,
}

#[derive(Debug)]
struct ReportNotice {
    successful: bool,
    message: String,
}

#[derive(Debug, Default)]
struct NumericFocusState {
    restore: Option<InputField>,
    sync_buffer: Option<InputField>,
}

/// Interactive die-yield workbench shared by native and browser builds.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct YieldWorkbench {
    inputs: FabricationInputs,
    lock_die_ratio: bool,
    link_scribe_lanes: bool,
    #[serde(skip)]
    analysis: Option<WaferAnalysis>,
    #[serde(skip)]
    validation: Option<ValidationErrors>,
    #[serde(skip)]
    report_open: bool,
    #[serde(skip)]
    report_notice: Option<ReportNotice>,
    #[serde(skip)]
    numeric_focus: NumericFocusState,
}

impl Default for YieldWorkbench {
    fn default() -> Self {
        let mut workbench = Self {
            inputs: FabricationInputs::default(),
            lock_die_ratio: false,
            link_scribe_lanes: true,
            analysis: None,
            validation: None,
            report_open: false,
            report_notice: None,
            numeric_focus: NumericFocusState::default(),
        };
        workbench.recalculate();
        workbench
    }
}

impl YieldWorkbench {
    /// Creates the application and restores persisted settings when available.
    #[must_use]
    pub fn new(context: &eframe::CreationContext<'_>) -> Self {
        theme::install(&context.egui_ctx);
        let mut workbench: Self = context
            .storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default();
        workbench.recalculate();
        workbench
    }

    fn recalculate(&mut self) {
        let was_invalid = self.validation.is_some();
        match analyze(&self.inputs) {
            Ok(analysis) => {
                self.analysis = Some(analysis);
                self.validation = None;
            }
            Err(errors) => {
                self.validation = Some(errors);
            }
        }
        if was_invalid != self.validation.is_some() {
            self.numeric_focus.sync_buffer = self.numeric_focus.restore;
        }
    }

    fn show_header(&mut self, ui: &mut egui::Ui) {
        let mut reset = false;
        let mut report = false;
        if ui.available_width() < 590.0 {
            ui.vertical(|ui| {
                ui.horizontal(header_brand);
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    report |= report_button(ui);
                    reset |= reset_button(ui);
                });
            });
        } else {
            ui.horizontal(|ui| {
                header_brand(ui);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    reset |= reset_button(ui);
                    report |= report_button(ui);
                });
            });
        }
        if report {
            self.report_open = true;
            self.report_notice = None;
        }
        if reset {
            *self = Self::default();
        }
    }

    fn show_workspace(&mut self, ui: &mut egui::Ui) {
        let wide = ui.available_width() >= WIDE_LAYOUT_THRESHOLD;
        if wide {
            self.show_summary(ui, true);
            ui.add_space(8.0);
            let available = ui.available_width();
            let settings_width = (available * 0.37).clamp(340.0, 430.0);
            let visual_width = available - settings_width - 16.0;
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    vec2(visual_width, 0.0),
                    Layout::top_down(Align::Min),
                    |ui| self.show_visual_card(ui),
                );
                ui.add_space(6.0);
                ui.allocate_ui_with_layout(
                    vec2(settings_width, 0.0),
                    Layout::top_down(Align::Min),
                    |ui| self.show_settings_card(ui),
                );
            });
        } else {
            self.show_summary(ui, false);
            ui.add_space(8.0);
            self.show_settings_card(ui);
            ui.add_space(6.0);
            self.show_visual_card(ui);
        }
    }

    fn show_summary(&self, ui: &mut egui::Ui, wide: bool) {
        let Some(analysis) = self.analysis.as_ref() else {
            unavailable_results(ui);
            return;
        };
        if self.validation.is_some() {
            paused_results_notice(ui);
            ui.add_space(8.0);
        }
        let summary = analysis.summary;
        let values: [(&str, String, String, Color32); 3] = [
            (
                "GROSS DIES / WAFER",
                format_integer(summary.geometric_usable),
                "Complete sites before modeled defects".to_owned(),
                theme::BLUE,
            ),
            (
                "ESTIMATED DIE YIELD",
                format!("{:.2}%", summary.yield_fraction * 100.0),
                format!(
                    "{} model",
                    yield_model_label(analysis.normalized_inputs.process.yield_model)
                ),
                theme::ACCENT,
            ),
            (
                "EXPECTED GOOD / WAFER",
                format!("≈{}", format_integer(summary.expected_good)),
                format!(
                    "{} gross × full-precision yield = {:.3}",
                    format_integer(summary.geometric_usable),
                    summary.expected_good_exact
                ),
                theme::CORAL,
            ),
        ];

        let columns = if wide { 3 } else { 1 };
        let width = (ui.available_width() - 12.0 * (columns as f32 - 1.0)) / columns as f32;
        for row in values.chunks(columns) {
            ui.horizontal(|ui| {
                for (index, (label, value, detail, accent)) in row.iter().enumerate() {
                    if index > 0 {
                        ui.add_space(2.0);
                    }
                    ui.allocate_ui_with_layout(
                        vec2(width, 106.0),
                        Layout::top_down(Align::Min),
                        |ui| metric_card(ui, label, value, detail, *accent),
                    );
                }
            });
        }
        ui.add_space(8.0);
        summary_scope_note(ui, &summary);
    }

    fn show_visual_card(&self, ui: &mut egui::Ui) {
        let displayed_diameter_mm = self
            .analysis
            .as_ref()
            .map_or(self.inputs.wafer.diameter_mm, |analysis| {
                analysis.normalized_inputs.wafer.diameter_mm
            });
        card().show(ui, |ui| {
            let heading = |ui: &mut egui::Ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Wafer map").size(18.0).strong());
                    ui.label(
                        RichText::new("Geometric placement with illustrative random loss")
                            .small()
                            .color(theme::TEXT_MUTED),
                    );
                });
            };
            if ui.available_width() < 480.0 {
                ui.vertical(|ui| {
                    heading(ui);
                    ui.add_space(6.0);
                    pill(
                        ui,
                        &format!("Ø {}", wafer_size_label(displayed_diameter_mm)),
                        theme::BLUE,
                    );
                });
            } else {
                ui.horizontal(|ui| {
                    heading(ui);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        pill(
                            ui,
                            &format!("Ø {}", wafer_size_label(displayed_diameter_mm)),
                            theme::BLUE,
                        );
                    });
                });
            }
            ui.add_space(12.0);

            if let Some(analysis) = &self.analysis {
                let scene = WaferScene::from_analysis(analysis);
                let palette = WaferPalette::default();
                ui.with_layout(Layout::top_down(Align::Center), |ui| {
                    let desired = (ui.available_width() - 4.0).clamp(260.0, 590.0);
                    paint_wafer(ui, &scene, desired);
                });
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    legend(ui, "Gross die", palette.productive);
                    legend(ui, "Illustrative loss", palette.defective);
                    legend(ui, "Partial boundary", palette.boundary);
                    legend(ui, "Edge band", palette.excluded);
                    legend(ui, "Scribe lane", palette.scribe);
                });
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "{} gross  •  {} partial  •  {} edge-band  •  {:.4} mm² active area",
                        format_integer(analysis.summary.geometric_usable),
                        format_integer(analysis.summary.partial),
                        format_integer(analysis.summary.edge_excluded),
                        analysis.summary.yield_area_mm2
                    ))
                    .small()
                    .color(theme::TEXT_MUTED),
                );
                ui.label(
                    RichText::new(format!(
                        "Loss locations are illustrative, not predicted. Scribe is shown at ≥{MIN_VISIBLE_SCRIBE_POINTS:.2} pt; the notch marker is not subtracted from geometry."
                    ))
                    .small()
                    .color(theme::TEXT_MUTED),
                );
            } else {
                unavailable_results(ui);
            }
        });
    }

    fn show_settings_card(&mut self, ui: &mut egui::Ui) {
        let before = self.inputs;
        card().show(ui, |ui| {
            ui.label(RichText::new("Calculation setup").size(18.0).strong());
            auto_update_note(ui);
            example_setup_notice(ui);

            ui.add_space(12.0);

            section_heading(ui, "ESSENTIALS", SectionGlyph::Wafer);
            ui.horizontal_wrapped(|ui| {
                for diameter in WAFER_PRESETS_MM {
                    let selected = (self.inputs.wafer.diameter_mm - diameter).abs() < f64::EPSILON;
                    let response = ui
                        .add(
                            egui::Button::new(wafer_size_label(diameter))
                                .selected(selected)
                                .corner_radius(7),
                        )
                        .on_hover_text(format!(
                            "Set wafer diameter to {}",
                            wafer_size_label(diameter)
                        ));
                    keep_focused_visible(&response);
                    if response.clicked() {
                        self.inputs.wafer.diameter_mm = diameter;
                    }
                }
            });
            input_row_f64_with_precision(
                ui,
                InputField::WaferDiameter,
                &format!(
                    "Diameter ({})",
                    wafer_inches_label(self.inputs.wafer.diameter_mm)
                ),
                &mut self.inputs.wafer.diameter_mm,
                25.0..=450.0,
                1.0,
                " mm",
                3,
                &mut self.numeric_focus,
            );
            self.show_field_error(ui, InputField::WaferDiameter);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Active die dimensions")
                        .color(theme::TEXT_MUTED),
                )
                .on_hover_text(
                    "Finished active dimensions used for random-defect yield. Scribe is entered separately.",
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let response = ui
                        .toggle_value(&mut self.lock_die_ratio, "Lock ratio")
                        .on_hover_text("Preserve the current die aspect ratio");
                    keep_focused_visible(&response);
                });
            });
            let old_width = self.inputs.die.width_mm;
            let old_height = self.inputs.die.height_mm;
            let width_changed = input_row_f64_with_precision(
                ui,
                InputField::DieWidth,
                "Active width",
                &mut self.inputs.die.width_mm,
                0.25..=450.0,
                0.001,
                " mm",
                6,
                &mut self.numeric_focus,
            );
            if width_changed && self.lock_die_ratio && old_width > 0.0 {
                self.inputs.die.height_mm = old_height * self.inputs.die.width_mm / old_width;
            }
            self.show_field_error(ui, InputField::DieWidth);
            let height_changed = input_row_f64_with_precision(
                ui,
                InputField::DieHeight,
                "Active height",
                &mut self.inputs.die.height_mm,
                0.25..=450.0,
                0.001,
                " mm",
                6,
                &mut self.numeric_focus,
            );
            if height_changed && self.lock_die_ratio && old_height > 0.0 {
                self.inputs.die.width_mm = old_width * self.inputs.die.height_mm / old_height;
            }
            self.show_field_error(ui, InputField::DieHeight);
            input_row_f64_with_precision(
                ui,
                InputField::DefectDensity,
                "Effective defect density (D₀)",
                &mut self.inputs.process.defect_density_cm2,
                0.0..=100.0,
                0.001,
                " /cm²",
                6,
                &mut self.numeric_focus,
            );
            self.show_field_error(ui, InputField::DefectDensity);
            ui.label(
                RichText::new(format!("D₀ basis: {DEFECT_DENSITY_POLICY}."))
                .small()
                .color(theme::TEXT_MUTED),
            );
            yield_model_row(ui, &mut self.inputs.process.yield_model);

            section_divider(ui);
            let manufacturing_header = egui::CollapsingHeader::new(format!(
                "Manufacturing geometry · {:.3} mm edge · {:.1}/{:.1} μm scribe",
                self.inputs.wafer.edge_exclusion_mm,
                self.inputs.die.column_lane_mm * 1_000.0,
                self.inputs.die.row_lane_mm * 1_000.0
            ))
            .id_salt("manufacturing_geometry")
            .default_open(false)
            .show(ui, |ui| {
                section_heading(ui, "FOOTPRINT INPUTS", SectionGlyph::DieGrid);
                input_row_f64_with_precision(
                    ui,
                    InputField::EdgeExclusion,
                    "Radial edge exclusion",
                    &mut self.inputs.wafer.edge_exclusion_mm,
                    0.0..=100.0,
                    0.01,
                    " mm",
                    6,
                    &mut self.numeric_focus,
                );
                self.show_field_error(ui, InputField::EdgeExclusion);
                let column_changed = input_row_micrometres(
                    ui,
                    InputField::ColumnLane,
                    "Column scribe",
                    &mut self.inputs.die.column_lane_mm,
                    &mut self.numeric_focus,
                );
                if column_changed && self.link_scribe_lanes {
                    self.inputs.die.row_lane_mm = self.inputs.die.column_lane_mm;
                }
                self.show_field_error(ui, InputField::ColumnLane);
                let row_changed = input_row_micrometres(
                    ui,
                    InputField::RowLane,
                    "Row scribe",
                    &mut self.inputs.die.row_lane_mm,
                    &mut self.numeric_focus,
                );
                if row_changed && self.link_scribe_lanes {
                    self.inputs.die.column_lane_mm = self.inputs.die.row_lane_mm;
                }
                self.show_field_error(ui, InputField::RowLane);
                let response = ui
                    .checkbox(&mut self.link_scribe_lanes, "Link X/Y scribes")
                    .on_hover_text("Keep both scribe-lane widths equal while editing");
                keep_focused_visible(&response);
                ui.label(
                    RichText::new(
                        "Gross placement uses active dimensions plus scribe pitch. A Gross die is counted only when its complete active rectangle fits inside the usable radius; scribe is spacing, not the boundary footprint. Yield uses active area only.",
                    )
                    .small()
                    .color(theme::TEXT_MUTED),
                );
            });
            keep_focused_visible(&manufacturing_header.header_response);

            section_divider(ui);
            let yield_header = egui::CollapsingHeader::new(format!(
                "Yield calculation · {}",
                yield_model_label(self.inputs.process.yield_model)
            ))
            .id_salt("yield_calculation")
            .default_open(false)
            .show(ui, |ui| {
                section_heading(ui, "MODEL DETAILS", SectionGlyph::Alignment);
                if self.inputs.process.yield_model == YieldModel::NegativeBinomial {
                    input_row_f64_with_precision(
                        ui,
                        InputField::ClusteringAlpha,
                        "Clustering alpha (α)",
                        &mut self.inputs.process.clustering_alpha,
                        f64::MIN_POSITIVE..=1.0e12,
                        0.1,
                        "",
                        6,
                        &mut self.numeric_focus,
                    );
                    self.show_field_error(ui, InputField::ClusteringAlpha);
                }
                ui.label(
                    RichText::new(yield_model_explanation(
                        self.inputs.process.yield_model,
                    ))
                    .small()
                    .color(theme::TEXT_MUTED),
                );
                if let Some(analysis) = &self.analysis {
                    let summary = analysis.summary;
                    ui.add_space(4.0);
                    detail_row(
                        ui,
                        "Yield area",
                        &format!(
                            "{:.6} mm² = {:.8} cm²",
                            summary.yield_area_mm2,
                            summary.yield_area_mm2 / 100.0
                        ),
                    );
                    detail_row(
                        ui,
                        "Exposure A·D₀",
                        &format!("{:.10}", summary.defect_exposure),
                    );
                    detail_row(
                        ui,
                        "Full-precision yield",
                        &format!("{:.10}", summary.yield_fraction),
                    );
                    detail_row(
                        ui,
                        "Unrounded expectation",
                        &format!("{:.6} good dies", summary.expected_good_exact),
                    );
                    ui.label(
                        RichText::new("The summary rounds the expectation to the nearest whole die.")
                            .small()
                            .color(theme::TEXT_MUTED),
                    );
                }
            });
            keep_focused_visible(&yield_header.header_response);

            section_divider(ui);
            let alignment_header = egui::CollapsingHeader::new("Grid alignment")
                .id_salt("grid_alignment")
                .default_open(false)
                .show(ui, |ui| {
                    section_heading(ui, "MANUAL PHASE", SectionGlyph::Alignment);
                    input_row_f64_with_precision(
                        ui,
                        InputField::OffsetX,
                        "Horizontal phase",
                        &mut self.inputs.process.offset_x_mm,
                        -450.0..=450.0,
                        0.001,
                        " mm",
                        6,
                        &mut self.numeric_focus,
                    );
                    self.show_field_error(ui, InputField::OffsetX);
                    input_row_f64_with_precision(
                        ui,
                        InputField::OffsetY,
                        "Vertical phase",
                        &mut self.inputs.process.offset_y_mm,
                        -450.0..=450.0,
                        0.001,
                        " mm",
                        6,
                        &mut self.numeric_focus,
                    );
                    self.show_field_error(ui, InputField::OffsetY);
                    let response = ui.checkbox(
                        &mut self.inputs.process.die_at_origin,
                        "Center a die at wafer origin",
                    );
                    keep_focused_visible(&response);
                    if let Some(analysis) = &self.analysis {
                        detail_row(
                            ui,
                            "Normalized phase",
                            &format!(
                                "{:.6} × {:.6} mm",
                                analysis.normalized_inputs.process.offset_x_mm,
                                analysis.normalized_inputs.process.offset_y_mm
                            ),
                        );
                    }
                });
            keep_focused_visible(&alignment_header.header_response);

            section_divider(ui);
            let probe_header = egui::CollapsingHeader::new(format!(
                "Idealized probe estimate · {} × {} sites",
                self.inputs.probe.columns, self.inputs.probe.rows
            ))
            .id_salt("probe_estimate")
            .default_open(false)
            .show(ui, |ui| {
                section_heading(ui, "FIXED GRID", SectionGlyph::ProbeArray);
                input_row_u32(
                    ui,
                    InputField::ProbeColumns,
                    "Columns per step",
                    &mut self.inputs.probe.columns,
                    1..=128,
                    &mut self.numeric_focus,
                );
                self.show_field_error(ui, InputField::ProbeColumns);
                input_row_u32(
                    ui,
                    InputField::ProbeRows,
                    "Rows per step",
                    &mut self.inputs.probe.rows,
                    1..=128,
                    &mut self.numeric_focus,
                );
                self.show_field_error(ui, InputField::ProbeRows);
                if let Some(analysis) = &self.analysis {
                    detail_row(
                        ui,
                        "Occupied grid blocks",
                        &format_integer(analysis.probe.touchdown_count),
                    );
                }
                ui.label(
                    RichText::new(
                        "Fixed rectangular grid estimate; probe reachability and test time are not modeled.",
                    )
                    .small()
                    .color(theme::TEXT_MUTED),
                );
            });
            keep_focused_visible(&probe_header.header_response);

            if let Some(analysis) = &self.analysis {
                section_divider(ui);
                let geometry_header = egui::CollapsingHeader::new("Geometry details")
                    .id_salt("geometry_details")
                    .default_open(false)
                    .show(ui, |ui| {
                        let normalized = analysis.normalized_inputs;
                        detail_row(
                            ui,
                            "Placement pitch",
                            &format!(
                                "{:.6} × {:.6} mm",
                                normalized.die.width_mm + normalized.die.column_lane_mm,
                                normalized.die.height_mm + normalized.die.row_lane_mm
                            ),
                        );
                        detail_row(
                            ui,
                            "Usable diameter",
                            &format!(
                                "{:.6} mm",
                                normalized.wafer.diameter_mm
                                    - 2.0 * normalized.wafer.edge_exclusion_mm
                            ),
                        );
                        detail_row(
                            ui,
                            "Partial boundary sites",
                            &format_integer(analysis.summary.partial),
                        );
                        detail_row(
                            ui,
                            "Edge-band sites",
                            &format_integer(analysis.summary.edge_excluded),
                        );
                    });
                keep_focused_visible(&geometry_header.header_response);
            }

            if self.validation.is_some() {
                section_divider(ui);
                self.show_validation(ui);
            }
        });

        if self.inputs != before {
            self.recalculate();
        }
    }

    fn show_validation(&self, ui: &mut egui::Ui) {
        let Some(errors) = &self.validation else {
            return;
        };
        egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(240, 106, 141, 18))
            .stroke(Stroke::new(1.0, Color32::from_rgb(111, 55, 75)))
            .corner_radius(10)
            .inner_margin(12)
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Fix the following inputs")
                        .strong()
                        .color(theme::CORAL),
                );
                ui.label(
                    RichText::new(
                        "Results continue to show the last valid setup until every error is fixed.",
                    )
                    .small()
                    .color(theme::TEXT_MUTED),
                );
                for error in errors.as_slice() {
                    ui.label(
                        RichText::new(format!("{}: {}", error.field.label(), error.message))
                            .small()
                            .color(theme::TEXT_MUTED),
                    );
                }
            });
    }

    fn show_field_error(&self, ui: &mut egui::Ui, field: InputField) {
        let Some(error) = self
            .validation
            .as_ref()
            .and_then(|errors| errors.as_slice().iter().find(|error| error.field == field))
        else {
            return;
        };
        ui.label(
            RichText::new(format!(
                "Error: {} Results remain from the last valid setup.",
                error.message
            ))
            .small()
            .strong()
            .color(theme::CORAL),
        );
    }

    fn show_report_dialog(&mut self, context: &egui::Context) {
        if !self.report_open {
            return;
        }

        let mut open = self.report_open;
        let mut close = context.input(|input| input.key_pressed(egui::Key::Escape));
        let mut export_format = None;
        let mut print = false;
        let available_width = context.content_rect().width();
        let compact = available_width < 480.0;
        let dialog_width = (available_width - 32.0).clamp(288.0, 440.0);
        egui::Window::new("report_export")
            .id(egui::Id::new("report_export_window"))
            .open(&mut open)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .default_width(dialog_width)
            .min_width(dialog_width)
            .max_width(dialog_width)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Export report").size(20.0).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        close |= ui
                            .add(egui::Button::new(RichText::new("×").size(20.0)).frame(false))
                            .on_hover_text("Close report export")
                            .clicked();
                    });
                });
                ui.label(
                    RichText::new(
                        "Choose a visual report or a machine-readable analysis snapshot.",
                    )
                    .color(theme::TEXT_MUTED),
                );
                ui.add_space(12.0);

                section_heading(ui, "EXPORT FORMAT", SectionGlyph::Wafer);
                let analysis_ready = self.analysis.is_some() && self.validation.is_none();
                let formats = [
                    (report::ReportFormat::Png, "PNG", "2× raster"),
                    (report::ReportFormat::Svg, "SVG", "portable vector"),
                    (report::ReportFormat::Pdf, "PDF", "A4 document"),
                    (report::ReportFormat::Json, "JSON", "reproducible data"),
                ];
                if compact {
                    ui.vertical(|ui| {
                        for (format, label, detail) in formats {
                            if report_format_button(
                                ui,
                                analysis_ready,
                                format!("{label}\n{detail}"),
                                true,
                            ) {
                                export_format = Some(format);
                            }
                        }
                    });
                } else {
                    egui::Grid::new("report_format_grid")
                        .num_columns(2)
                        .spacing(vec2(8.0, 8.0))
                        .show(ui, |ui| {
                            for (index, (format, label, detail)) in formats.into_iter().enumerate()
                            {
                                if report_format_button(
                                    ui,
                                    analysis_ready,
                                    format!("{label}\n{detail}"),
                                    false,
                                ) {
                                    export_format = Some(format);
                                }
                                if index % 2 == 1 {
                                    ui.end_row();
                                }
                            }
                        });
                }

                ui.add_space(10.0);
                let print_response = ui.add_enabled(
                    analysis_ready,
                    egui::Button::new(RichText::new("Print report").strong().color(Color32::WHITE))
                        .fill(theme::ACCENT)
                        .stroke(Stroke::new(1.0, theme::ACCENT))
                        .min_size(vec2(ui.available_width(), 40.0))
                        .corner_radius(9),
                );
                if print_response.clicked() {
                    print = true;
                }
                ui.label(RichText::new(print_hint()).small().color(theme::TEXT_MUTED));

                if !analysis_ready {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Resolve the current validation errors before exporting.")
                            .color(theme::CORAL),
                    );
                }

                if let Some(notice) = &self.report_notice {
                    ui.add_space(8.0);
                    let color = if notice.successful {
                        theme::ACCENT
                    } else {
                        theme::CORAL
                    };
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(
                            color.r(),
                            color.g(),
                            color.b(),
                            18,
                        ))
                        .stroke(Stroke::new(
                            1.0,
                            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 70),
                        ))
                        .corner_radius(8)
                        .inner_margin(10)
                        .show(ui, |ui| {
                            ui.label(RichText::new(&notice.message).small().color(color));
                        });
                }
            });
        if close {
            open = false;
        }
        self.report_open = open;

        if let Some(format) = export_format {
            self.export_report(format);
        }
        if print {
            self.print_report();
        }
    }

    fn export_report(&mut self, format: report::ReportFormat) {
        let result = self
            .validation
            .is_none()
            .then_some(())
            .ok_or_else(|| "Resolve the current validation errors before exporting".to_owned())
            .and_then(|()| {
                self.analysis
                    .as_ref()
                    .ok_or_else(|| "No valid analysis is available".to_owned())
            })
            .and_then(|analysis| {
                report::generate(&self.inputs, analysis, format).map_err(|error| error.to_string())
            })
            .and_then(|file| delivery::save_report(&file));

        self.report_notice = match result {
            Ok(Some(message)) => Some(ReportNotice {
                successful: true,
                message,
            }),
            Ok(None) => None,
            Err(message) => Some(ReportNotice {
                successful: false,
                message,
            }),
        };
    }

    fn print_report(&mut self) {
        let result = self
            .validation
            .is_none()
            .then_some(())
            .ok_or_else(|| "Resolve the current validation errors before printing".to_owned())
            .and_then(|()| {
                self.analysis
                    .as_ref()
                    .ok_or_else(|| "No valid analysis is available".to_owned())
            })
            .and_then(|analysis| delivery::print_report(&self.inputs, analysis));

        self.report_notice = Some(match result {
            Ok(message) => ReportNotice {
                successful: true,
                message,
            },
            Err(message) => ReportNotice {
                successful: false,
                message,
            },
        });
    }
}

impl eframe::App for YieldWorkbench {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        let page_scroll = if self.report_open {
            0.0
        } else {
            let distance = context.content_rect().height() * 0.8;
            context.input_mut(|input| {
                if input.consume_key(egui::Modifiers::NONE, egui::Key::PageDown) {
                    -distance
                } else if input.consume_key(egui::Modifiers::NONE, egui::Key::PageUp) {
                    distance
                } else {
                    0.0
                }
            })
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::CANVAS).inner_margin(0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if page_scroll != 0.0 {
                            ui.scroll_with_delta(vec2(0.0, page_scroll));
                        }
                        ui.set_min_width(ui.available_width());
                        let horizontal_margin = if ui.available_width() < 640.0 { 14 } else { 24 };
                        egui::Frame::new()
                            .inner_margin(Margin::symmetric(horizontal_margin, 20))
                            .show(ui, |ui| {
                                let content_width = ui.available_width().min(1420.0);
                                ui.with_layout(Layout::top_down(Align::Center), |ui| {
                                    ui.allocate_ui_with_layout(
                                        vec2(content_width, 0.0),
                                        Layout::top_down(Align::Min),
                                        |ui| {
                                            self.show_header(ui);
                                            ui.add_space(20.0);
                                            self.show_workspace(ui);
                                            ui.add_space(18.0);
                                            project_footer(ui);
                                        },
                                    );
                                });
                            });
                    });
            });
        self.show_report_dialog(&context);
    }
}

fn project_footer(ui: &mut egui::Ui) {
    let notice = || {
        RichText::new("Random-defect yield estimate  •  Results are for planning")
            .small()
            .color(theme::TEXT_MUTED)
    };
    let source_link = |ui: &mut egui::Ui| {
        ui.hyperlink_to(
            RichText::new("Source on GitHub")
                .small()
                .color(theme::TEXT_MUTED),
            PROJECT_URL,
        );
    };

    if ui.available_width() < 590.0 {
        ui.vertical(|ui| {
            ui.label(notice());
            source_link(ui);
        });
    } else {
        ui.horizontal(|ui| {
            ui.label(notice());
            ui.with_layout(Layout::right_to_left(Align::Center), source_link);
        });
    }
}

fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(12)
        .inner_margin(18)
        .shadow(egui::epaint::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(18),
        })
}

fn paused_results_notice(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            theme::AMBER.r(),
            theme::AMBER.g(),
            theme::AMBER.b(),
            18,
        ))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(
                theme::AMBER.r(),
                theme::AMBER.g(),
                theme::AMBER.b(),
                80,
            ),
        ))
        .corner_radius(10)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.label(
                RichText::new("Results paused — showing the last valid setup")
                    .strong()
                    .color(theme::AMBER),
            );
            ui.label(
                RichText::new("Fix the input errors before exporting or using these values.")
                    .small()
                    .color(theme::TEXT_MUTED),
            );
        });
}

fn unavailable_results(ui: &mut egui::Ui) {
    card().show(ui, |ui| {
        ui.label(RichText::new("Results unavailable").strong());
        ui.label(
            RichText::new("Enter a valid fabrication setup to calculate this section.")
                .color(theme::TEXT_MUTED),
        );
    });
}

fn header_brand(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(theme::ACCENT)
        .corner_radius(11)
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.label(RichText::new("YS").size(17.0).strong().color(theme::CANVAS));
        });

    ui.vertical(|ui| {
        ui.label(RichText::new("Yield Studio").size(20.0).strong());
        ui.label(
            RichText::new("Wafer planning, without the spreadsheet")
                .small()
                .color(theme::TEXT_MUTED),
        );
    });
}

fn reset_button(ui: &mut egui::Ui) -> bool {
    let response = ui
        .add_sized(
            [82.0, HEADER_CONTROL_HEIGHT],
            egui::Button::new(RichText::new("Reset").color(theme::TEXT_MUTED))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::new(1.0, theme::BORDER))
                .corner_radius(12),
        )
        .on_hover_text("Restore the example values");
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(response.rect.left() + 15.0, response.rect.center().y),
        vec2(15.0, 15.0),
    );
    let icon_color = ui.style().interact(&response).fg_stroke.color;
    paint_reset_glyph(ui.painter(), icon_rect, icon_color);
    paint_focus_ring(ui, &response, 12.0);

    response.clicked()
}

fn report_button(ui: &mut egui::Ui) -> bool {
    let response = ui
        .add_sized(
            [94.0, HEADER_CONTROL_HEIGHT],
            egui::Button::new(RichText::new("Report").color(theme::TEXT_MUTED))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::new(1.0, theme::BORDER))
                .corner_radius(12),
        )
        .on_hover_text("Export or print a styled analysis report");
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(response.rect.left() + 16.0, response.rect.center().y),
        vec2(16.0, 16.0),
    );
    let icon_color = ui.style().interact(&response).fg_stroke.color;
    paint_report_glyph(ui.painter(), icon_rect, icon_color);
    paint_focus_ring(ui, &response, 12.0);

    response.clicked()
}

fn paint_focus_ring(ui: &egui::Ui, response: &egui::Response, radius: f32) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.expand(1.0),
            radius,
            Stroke::new(2.0, theme::ACCENT),
            egui::StrokeKind::Outside,
        );
    }
}

fn report_format_button(ui: &mut egui::Ui, enabled: bool, label: String, compact: bool) -> bool {
    let width = if compact { ui.available_width() } else { 190.0 };
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).line_height(Some(17.0)))
            .min_size(vec2(width, if compact { 46.0 } else { 54.0 }))
            .corner_radius(9),
    );
    keep_focused_visible(&response);
    response.clicked()
}

fn metric_card(ui: &mut egui::Ui, label: &str, value: &str, detail: &str, accent: Color32) {
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(14)
        .inner_margin(15)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(label).size(11.0).strong().color(accent));
            ui.add_space(3.0);
            ui.label(RichText::new(value).size(25.0).strong().color(theme::TEXT));
            ui.label(RichText::new(detail).small().color(theme::TEXT_MUTED));
        });
}

fn summary_scope_note(ui: &mut egui::Ui, summary: &die_yield_core::YieldSummary) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            theme::BLUE.r(),
            theme::BLUE.g(),
            theme::BLUE.b(),
            10,
        ))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(
                theme::BLUE.r(),
                theme::BLUE.g(),
                theme::BLUE.b(),
                45,
            ),
        ))
        .corner_radius(9)
        .inner_margin(10)
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "Geometry scope: Gross counts complete active rectangles inside the usable radius. {} partial-boundary and {} edge-band sites are excluded.",
                    format_integer(summary.partial),
                    format_integer(summary.edge_excluded)
                ))
                .small()
                .color(theme::TEXT_MUTED),
            );
            ui.label(
                RichText::new(
                    "Random-defect loss is applied only to Gross dies; red map locations are illustrative, not predicted.",
                )
                .small()
                .color(theme::TEXT_MUTED),
            );
        });
}

fn section_heading(ui: &mut egui::Ui, text: &str, glyph: SectionGlyph) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::hover());
        paint_section_glyph(ui.painter(), rect, glyph, theme::BLUE);
        ui.label(RichText::new(text).size(11.0).strong().color(theme::BLUE));
    });
}

fn auto_update_note(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (rect, response) = ui.allocate_exact_size(vec2(8.0, 8.0), egui::Sense::hover());
        ui.painter()
            .circle_filled(rect.center(), 3.0, theme::ACCENT);
        ui.label(
            RichText::new("Auto-updating as values change")
                .small()
                .color(theme::TEXT_MUTED),
        );
        response.on_hover_text("Results refresh after any input changes");
    });
}

fn example_setup_notice(ui: &mut egui::Ui) {
    ui.add_space(5.0);
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            theme::BLUE.r(),
            theme::BLUE.g(),
            theme::BLUE.b(),
            12,
        ))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(theme::BLUE.r(), theme::BLUE.g(), theme::BLUE.b(), 55),
        ))
        .corner_radius(8)
        .inner_margin(10)
        .show(ui, |ui| {
            ui.label(
                RichText::new("Example values are loaded")
                    .strong()
                    .color(theme::BLUE),
            );
            ui.label(
                RichText::new(
                    "Verify every dimension, edge policy, and D₀ before using the estimate.",
                )
                .small()
                .color(theme::TEXT_MUTED),
            );
        });
}

fn paint_section_glyph(
    painter: &egui::Painter,
    rect: egui::Rect,
    glyph: SectionGlyph,
    color: Color32,
) {
    let center = rect.center();
    let stroke = Stroke::new(1.25, color);
    let soft_fill = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 24);

    match glyph {
        SectionGlyph::Wafer => {
            painter.circle_filled(center, 7.0, soft_fill);
            painter.circle_stroke(center, 7.0, stroke);
            painter.line_segment(
                [
                    egui::pos2(center.x - 2.2, center.y + 5.5),
                    egui::pos2(center.x, center.y + 3.6),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x, center.y + 3.6),
                    egui::pos2(center.x + 2.2, center.y + 5.5),
                ],
                stroke,
            );
        }
        SectionGlyph::DieGrid => {
            for offset in [
                vec2(-3.5, -3.5),
                vec2(3.5, -3.5),
                vec2(-3.5, 3.5),
                vec2(3.5, 3.5),
            ] {
                let cell = egui::Rect::from_center_size(center + offset, vec2(5.2, 5.2));
                painter.rect_filled(cell, 1, soft_fill);
                painter.rect_stroke(cell, 1, stroke, egui::StrokeKind::Inside);
            }
        }
        SectionGlyph::Alignment => {
            painter.circle_stroke(center, 5.1, stroke);
            painter.line_segment(
                [
                    egui::pos2(center.x - 8.0, center.y),
                    egui::pos2(center.x + 8.0, center.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x, center.y - 8.0),
                    egui::pos2(center.x, center.y + 8.0),
                ],
                stroke,
            );
            painter.circle_filled(egui::pos2(center.x + 5.0, center.y - 5.0), 1.5, color);
        }
        SectionGlyph::ProbeArray => {
            let outline = egui::Rect::from_center_size(center, vec2(15.0, 13.0));
            painter.rect_filled(outline, 2, soft_fill);
            painter.rect_stroke(outline, 2, stroke, egui::StrokeKind::Inside);
            for y in [-3.2, 0.0, 3.2] {
                for x in [-4.0, 0.0, 4.0] {
                    painter.circle_filled(center + vec2(x, y), 1.0, color);
                }
            }
        }
    }
}

fn paint_reset_glyph(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    let center = rect.center();
    let radius = 5.2;
    let stroke = Stroke::new(1.25, color);
    let points = (0..=14)
        .map(|step| {
            let angle = -0.35 + step as f32 * 4.8 / 14.0;
            center + vec2(angle.cos(), angle.sin()) * radius
        })
        .collect::<Vec<_>>();
    painter.add(egui::Shape::line(points, stroke));
    let tip = center + vec2((-0.35_f32).cos(), (-0.35_f32).sin()) * radius;
    painter.line_segment([tip, tip + vec2(-3.4, -0.4)], stroke);
    painter.line_segment([tip, tip + vec2(-1.2, 3.0)], stroke);
}

fn paint_report_glyph(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    let center = rect.center();
    let stroke = Stroke::new(1.25, color);
    painter.line_segment(
        [
            egui::pos2(center.x, center.y - 6.0),
            egui::pos2(center.x, center.y + 2.0),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(center.x - 3.0, center.y - 0.5),
            egui::pos2(center.x, center.y + 2.5),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(center.x, center.y + 2.5),
            egui::pos2(center.x + 3.0, center.y - 0.5),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(center.x - 6.0, center.y + 4.0),
            egui::pos2(center.x - 6.0, center.y + 6.5),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(center.x - 6.0, center.y + 6.5),
            egui::pos2(center.x + 6.0, center.y + 6.5),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(center.x + 6.0, center.y + 6.5),
            egui::pos2(center.x + 6.0, center.y + 4.0),
        ],
        stroke,
    );
}

#[cfg(target_arch = "wasm32")]
fn print_hint() -> &'static str {
    "Uses the browser print dialog; choose a printer or Save as PDF."
}

#[cfg(not(target_arch = "wasm32"))]
fn print_hint() -> &'static str {
    "Opens a print-ready PDF in the default viewer for system printing."
}

fn section_divider(ui: &mut egui::Ui) {
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);
}

#[expect(
    clippy::too_many_arguments,
    reason = "the shared row keeps each numeric format and focus behavior explicit"
)]
fn input_row_f64_with_precision(
    ui: &mut egui::Ui,
    field: InputField,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    speed: f64,
    suffix: &str,
    max_decimals: usize,
    numeric_focus: &mut NumericFocusState,
) -> bool {
    let mut changed = false;
    ui.push_id(label, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).color(theme::TEXT_MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let response = ui.add_sized(
                    [128.0, 32.0],
                    egui::DragValue::new(value)
                        .range(range)
                        .speed(speed)
                        .max_decimals(max_decimals)
                        .suffix(suffix),
                );
                keep_focused_visible(&response);
                preserve_numeric_focus(field, numeric_focus, &response);
                changed = response.changed();
            });
        });
    });
    changed
}

fn input_row_micrometres(
    ui: &mut egui::Ui,
    field: InputField,
    label: &str,
    value_mm: &mut f64,
    numeric_focus: &mut NumericFocusState,
) -> bool {
    let mut value_um = *value_mm * 1_000.0;
    let changed = input_row_f64_with_precision(
        ui,
        field,
        label,
        &mut value_um,
        0.0..=10_000.0,
        0.1,
        " μm",
        3,
        numeric_focus,
    );
    if changed {
        *value_mm = value_um / 1_000.0;
    }
    changed
}

fn input_row_u32(
    ui: &mut egui::Ui,
    field: InputField,
    label: &str,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
    numeric_focus: &mut NumericFocusState,
) -> bool {
    let mut changed = false;
    ui.push_id(label, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).color(theme::TEXT_MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let response = ui.add_sized(
                    [128.0, 32.0],
                    egui::DragValue::new(value).range(range).speed(0.2),
                );
                keep_focused_visible(&response);
                preserve_numeric_focus(field, numeric_focus, &response);
                changed = response.changed();
            });
        });
    });
    changed
}

fn preserve_numeric_focus(
    field: InputField,
    numeric_focus: &mut NumericFocusState,
    response: &egui::Response,
) {
    if numeric_focus.sync_buffer == Some(field) {
        response
            .ctx
            .data_mut(|data| data.remove::<String>(response.id));
        numeric_focus.sync_buffer = None;
    }
    if numeric_focus.restore == Some(field) {
        response.request_focus();
        numeric_focus.restore = None;
    }
    if response.changed() && response.has_focus() {
        numeric_focus.restore = Some(field);
    }
}

fn yield_model_row(ui: &mut egui::Ui, model: &mut YieldModel) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Random-defect model").color(theme::TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let response = egui::ComboBox::from_id_salt("yield_model_selector")
                .selected_text(yield_model_label(*model))
                .width(178.0)
                .show_ui(ui, |ui| {
                    for candidate in [
                        YieldModel::MurphyTriangular,
                        YieldModel::Poisson,
                        YieldModel::NegativeBinomial,
                        YieldModel::Seeds,
                    ] {
                        let response =
                            ui.selectable_value(model, candidate, yield_model_label(candidate));
                        keep_focused_visible(&response);
                    }
                })
                .response;
            keep_focused_visible(&response);
        });
    });
}

fn yield_model_label(model: YieldModel) -> &'static str {
    match model {
        YieldModel::Poisson => "Poisson",
        YieldModel::MurphyTriangular => "Murphy triangular",
        YieldModel::Seeds => "Seeds",
        YieldModel::NegativeBinomial => "Negative binomial",
    }
}

fn yield_model_explanation(model: YieldModel) -> &'static str {
    match model {
        YieldModel::Poisson => "Y = exp(−A·D₀). Assumes independent, uniformly random defects.",
        YieldModel::MurphyTriangular => {
            "Y = [(1 − exp(−A·D₀)) / (A·D₀)]². Models a triangular defect-density distribution."
        }
        YieldModel::Seeds => "Y = 1 / (1 + A·D₀). Equivalent to negative binomial with α = 1.",
        YieldModel::NegativeBinomial => {
            "Y = (1 + A·D₀ / α)^(−α). Lower α represents stronger defect clustering."
        }
    }
}

fn detail_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(label).small().color(theme::TEXT_MUTED));
        ui.label(RichText::new(value).small().strong().color(theme::TEXT));
    });
}

fn keep_focused_visible(response: &egui::Response) {
    if response.gained_focus() {
        response.scroll_to_me(Some(Align::Center));
    }
}

fn pill(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            22,
        ))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 85),
        ))
        .corner_radius(12)
        .inner_margin(Margin::symmetric(9, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(10.5).strong().color(color));
        });
}

fn legend(ui: &mut egui::Ui, label: &str, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 9.0), egui::Sense::hover());
    ui.painter().rect_filled(rect.shrink(0.5), 1, color);
    ui.label(RichText::new(label).small().color(theme::TEXT_MUTED));
}

pub(crate) fn wafer_size_label(diameter_mm: f64) -> String {
    format!(
        "{} mm ({})",
        compact_decimal(diameter_mm, 2),
        wafer_inches_label(diameter_mm)
    )
}

fn wafer_inches_label(diameter_mm: f64) -> String {
    if let Some((_, inches)) = NOMINAL_WAFER_INCHES
        .iter()
        .find(|(nominal_mm, _)| (diameter_mm - nominal_mm).abs() <= 0.25)
    {
        format!("{inches} in")
    } else {
        format!("{} in", compact_decimal(diameter_mm / 25.4, 2))
    }
}

fn compact_decimal(value: f64, precision: usize) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, RawInput, Rect};

    #[test]
    fn responsive_ui_accepts_parameter_matrix() {
        let cases = [
            (76.0, 0.0, 4.0, 5.0, 0.0, 0.002, 1, 1),
            (150.0, 0.1, 10.0, 8.0, 0.001, 0.25, 2, 4),
            (300.0, 1.0, 15.0, 12.0, 0.12, 0.12, 4, 4),
            (450.0, 5.0, 24.0, 18.0, 1.0, 0.0, 8, 12),
        ];

        for width in [360.0, 480.0, 820.0, 1440.0] {
            for (diameter, density, die_width, die_height, column_lane, row_lane, columns, rows) in
                cases
            {
                let mut workbench = YieldWorkbench::default();
                workbench.inputs.wafer.diameter_mm = diameter;
                workbench.inputs.wafer.edge_exclusion_mm = (diameter * 0.02).max(1.0);
                workbench.inputs.process.defect_density_cm2 = density;
                workbench.inputs.die.width_mm = die_width;
                workbench.inputs.die.height_mm = die_height;
                workbench.inputs.die.column_lane_mm = column_lane;
                workbench.inputs.die.row_lane_mm = row_lane;
                workbench.inputs.probe.columns = columns;
                workbench.inputs.probe.rows = rows;
                workbench.recalculate();
                workbench.report_open = true;
                assert!(workbench.analysis.is_some());

                let context = egui::Context::default();
                theme::install(&context);
                let output = context.run_ui(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 900.0))),
                        ..Default::default()
                    },
                    |ui| {
                        workbench.show_workspace(ui);
                        project_footer(ui);
                        workbench.show_report_dialog(ui.ctx());
                    },
                );
                output.drop_without_applying_deltas();
            }
        }
    }

    #[test]
    fn invalid_setup_preserves_the_last_valid_result_for_recovery() {
        let mut workbench = YieldWorkbench::default();
        let last_valid = workbench
            .analysis
            .clone()
            .expect("default setup should have a result");
        workbench.inputs.wafer.diameter_mm = 10.0;
        workbench.inputs.process.defect_density_cm2 = -1.0;
        workbench.recalculate();

        assert_eq!(workbench.analysis, Some(last_valid));
        assert!(workbench.validation.is_some());

        let context = egui::Context::default();
        let output = context.run_ui(RawInput::default(), |ui| {
            workbench.show_workspace(ui);
        });
        output.drop_without_applying_deltas();
    }

    #[test]
    fn integer_formatting_is_readable() {
        assert_eq!(format_integer(0), "0");
        assert_eq!(format_integer(999), "999");
        assert_eq!(format_integer(1_000), "1,000");
        assert_eq!(format_integer(12_345_678), "12,345,678");
    }

    #[test]
    fn standard_wafer_sizes_include_nominal_inches() {
        let cases = [
            (50.0, "50 mm (2 in)"),
            (75.0, "75 mm (3 in)"),
            (76.0, "76 mm (3 in)"),
            (100.0, "100 mm (4 in)"),
            (125.0, "125 mm (5 in)"),
            (150.0, "150 mm (6 in)"),
            (200.0, "200 mm (8 in)"),
            (300.0, "300 mm (12 in)"),
            (330.0, "330 mm (13 in)"),
            (450.0, "450 mm (18 in)"),
        ];

        for (diameter, expected) in cases {
            assert_eq!(wafer_size_label(diameter), expected);
        }
    }

    #[test]
    fn custom_wafer_size_uses_a_readable_conversion() {
        assert_eq!(wafer_size_label(254.0), "254 mm (10 in)");
        assert_eq!(wafer_size_label(123.4), "123.4 mm (4.86 in)");
    }
}
