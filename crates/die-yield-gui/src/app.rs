use crate::{delivery, report, theme};
use die_yield_core::{
    FabricationInputs, InputField, ValidationErrors, WaferAnalysis, YieldModel, analyze,
};
use die_yield_render::{MIN_VISIBLE_SCRIBE_POINTS, WaferPalette, WaferScene, paint_wafer};
use eframe::egui;
use egui::{Align, Color32, Layout, Margin, RichText, Stroke, vec2};
use serde::{Deserialize, Serialize};

const WIDE_LAYOUT_THRESHOLD: f32 = 960.0;
const STACKED_CONTROL_THRESHOLD: f32 = 340.0;
const TOUCH_SAFE_VIEWPORT_THRESHOLD: f32 = 1_008.0;
const CONTROL_HEIGHT: f32 = 44.0;
const HEADER_CONTROL_HEIGHT: f32 = CONTROL_HEIGHT;
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
    invalid_text: Option<InputField>,
    rejected_text: Option<InputField>,
    touch_editor: Option<TouchNumericEditor>,
}

#[derive(Debug)]
struct TouchNumericEditor {
    field: InputField,
    label: String,
    value_text: String,
    suffix: String,
    minimum: f64,
    maximum: f64,
    scale_to_model: f64,
    integer: bool,
    focus_input: bool,
    error: Option<String>,
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
    focus_report_dialog: bool,
    #[serde(skip)]
    restore_report_focus: bool,
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
            focus_report_dialog: false,
            restore_report_focus: false,
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
        configure_spoken_feedback(&context.egui_ctx);
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
        let (report_response, reset) = if ui.available_width() < 590.0 {
            ui.vertical(|ui| {
                ui.horizontal(header_brand);
                ui.add_space(5.0);
                ui.horizontal(header_controls).inner
            })
            .inner
        } else {
            ui.horizontal(|ui| {
                header_brand(ui);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let controls_width = 94.0 + ui.spacing().item_spacing.x + 82.0;
                    ui.allocate_ui_with_layout(
                        vec2(controls_width, HEADER_CONTROL_HEIGHT),
                        Layout::left_to_right(Align::Center),
                        header_controls,
                    )
                    .inner
                })
                .inner
            })
            .inner
        };

        if self.restore_report_focus {
            report_response.request_focus();
            paint_focus_ring(ui, &report_response, 12.0);
            self.restore_report_focus = false;
        }
        if report_response.clicked() {
            self.report_open = true;
            self.report_notice = None;
            self.focus_report_dialog = true;
        }
        if reset {
            *self = Self::default();
        }
    }

    fn show_workspace(&mut self, ui: &mut egui::Ui) {
        let wide = uses_wide_layout(ui.available_width());
        if wide {
            self.show_summary(ui, true);
            ui.add_space(8.0);
            let available = ui.available_width();
            let settings_width = (available * 0.37).clamp(340.0, 430.0);
            let visual_width = available - settings_width - ui.spacing().item_spacing.x;
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    vec2(visual_width, 0.0),
                    Layout::top_down(Align::Min),
                    |ui| self.show_visual_card(ui),
                );
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
                    let desired = wafer_map_size(ui.available_width());
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
                ui.add(
                    egui::Label::new(RichText::new(format!(
                        "{} gross  •  {} partial  •  {} edge-band  •  {:.4} mm² active area",
                        format_integer(analysis.summary.geometric_usable),
                        format_integer(analysis.summary.partial),
                        format_integer(analysis.summary.edge_excluded),
                        analysis.summary.yield_area_mm2
                    ))
                    .small()
                    .color(theme::TEXT_MUTED))
                    .wrap(),
                );
                ui.add(
                    egui::Label::new(RichText::new(format!(
                        "Loss locations are illustrative, not predicted. Scribe is shown at ≥{MIN_VISIBLE_SCRIBE_POINTS:.2} pt; the notch marker is not subtracted from geometry."
                    ))
                    .small()
                    .color(theme::TEXT_MUTED))
                    .wrap(),
                );
            } else {
                unavailable_results(ui);
            }
        });
    }

    fn show_settings_card(&mut self, ui: &mut egui::Ui) {
        let before = self.inputs;
        card().show(ui, |ui| {
            ui.visuals_mut().collapsing_header_frame = true;
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
            let manufacturing_header = egui::CollapsingHeader::new("Manufacturing geometry")
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
            let yield_header = egui::CollapsingHeader::new("Yield calculation")
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
            let probe_header = egui::CollapsingHeader::new("Idealized probe estimate")
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
        if self.numeric_focus.invalid_text == Some(field) {
            let requirement = if field_requires_integer(field) {
                "a finite whole number"
            } else {
                "a finite number"
            };
            ui.label(
                RichText::new(format!(
                    "Enter {requirement}. This text has not been applied."
                ))
                .small()
                .strong()
                .color(theme::CORAL),
            );
            return;
        }
        if self.numeric_focus.rejected_text == Some(field) {
            ui.label(
                RichText::new("The invalid entry was not applied; the previous value remains.")
                    .small()
                    .strong()
                    .color(theme::CORAL),
            );
            return;
        }

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

        let mut export_format = None;
        let mut print = false;
        let available_width = context.content_rect().width();
        let compact = available_width < 480.0;
        let dialog_outer_width = (available_width - 32.0).clamp(288.0, 440.0);
        let dialog_content_width = dialog_outer_width - 36.0;
        let request_focus = self.focus_report_dialog;
        let modal = egui::Modal::new(egui::Id::new("report_export_modal"))
            .frame(card())
            .show(context, |ui| {
                ui.set_width(dialog_content_width);
                ui.ctx().accesskit_node_builder(ui.id(), |builder| {
                    builder.set_role(egui::accesskit::Role::Dialog);
                    builder.set_label("Export report");
                    builder.set_modal();
                });
                let mut close = false;
                let mut focus_applied = false;
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Export report").size(20.0).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let close_response = ui
                            .add_sized(
                                [CONTROL_HEIGHT, CONTROL_HEIGHT],
                                egui::Button::new(RichText::new("×").size(20.0)).frame(false),
                            )
                            .on_hover_text("Close report export");
                        close_response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                true,
                                "Close report export",
                            )
                        });
                        if request_focus && !ui.is_sizing_pass() {
                            close_response.request_focus();
                            focus_applied = true;
                        }
                        paint_focus_ring(ui, &close_response, 8.0);
                        close |= close_response.clicked();
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
                        .min_size(vec2(ui.available_width(), CONTROL_HEIGHT))
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
                (close, focus_applied)
            });

        if modal.inner.1 {
            self.focus_report_dialog = false;
        }
        if modal.inner.0 || modal.should_close() {
            self.report_open = false;
            self.focus_report_dialog = false;
            self.restore_report_focus = true;
        }

        if let Some(format) = export_format {
            self.export_report(format);
        }
        if print {
            self.print_report();
        }
    }

    fn show_touch_numeric_dialog(&mut self, context: &egui::Context) {
        let Some(mut editor) = self.numeric_focus.touch_editor.take() else {
            return;
        };

        let mut cancel = false;
        let mut apply = false;
        let available_width = context.content_rect().width();
        let dialog_outer_width = (available_width - 32.0).clamp(288.0, 420.0);
        let dialog_content_width = dialog_outer_width - 36.0;
        let modal = egui::Modal::new(egui::Id::new("touch_numeric_editor_modal"))
            .frame(card())
            .show(context, |ui| {
                ui.set_width(dialog_content_width);
                ui.ctx().accesskit_node_builder(ui.id(), |builder| {
                    builder.set_role(egui::accesskit::Role::Dialog);
                    builder.set_label(format!("Edit {}", editor.label));
                    builder.set_modal();
                });
                ui.label(
                    RichText::new(format!("Edit {}", editor.label))
                        .size(20.0)
                        .strong(),
                );
                ui.label(
                    RichText::new(touch_numeric_range_text(&editor))
                        .small()
                        .color(theme::TEXT_MUTED),
                );
                ui.add_space(10.0);

                let input_id = ui.make_persistent_id("touch_numeric_value");
                let submit_from_input = ui.memory(|memory| memory.has_focus(input_id))
                    && ui.input(|input| input.key_pressed(egui::Key::Enter));
                let input_response = if editor.suffix.is_empty() {
                    ui.add_sized(
                        [ui.available_width(), CONTROL_HEIGHT],
                        egui::TextEdit::singleline(&mut editor.value_text).id(input_id),
                    )
                } else {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let suffix_width = ui.fonts_mut(|fonts| {
                            fonts
                                .layout_no_wrap(
                                    editor.suffix.clone(),
                                    egui::TextStyle::Body.resolve(ui.style()),
                                    theme::TEXT_MUTED,
                                )
                                .size()
                                .x
                        });
                        let width =
                            (ui.available_width() - suffix_width - ui.spacing().item_spacing.x)
                                .max(80.0);
                        let response = ui.add_sized(
                            [width, CONTROL_HEIGHT],
                            egui::TextEdit::singleline(&mut editor.value_text).id(input_id),
                        );
                        ui.label(RichText::new(&editor.suffix).color(theme::TEXT_MUTED));
                        response
                    })
                    .inner
                };
                input_response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::TextEdit,
                        true,
                        format!("{} value", editor.label),
                    )
                });
                if editor.focus_input && !ui.is_sizing_pass() {
                    input_response.request_focus();
                    editor.focus_input = false;
                }
                apply |= submit_from_input;

                if let Some(error) = &editor.error {
                    ui.label(RichText::new(error).small().strong().color(theme::CORAL));
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_sized(
                            [96.0, CONTROL_HEIGHT],
                            egui::Button::new("Cancel").corner_radius(9),
                        )
                        .clicked()
                    {
                        cancel = true;
                    }
                    if ui
                        .add_sized(
                            [96.0, CONTROL_HEIGHT],
                            egui::Button::new(
                                RichText::new("Apply").strong().color(Color32::WHITE),
                            )
                            .fill(theme::ACCENT)
                            .stroke(Stroke::new(1.0, theme::ACCENT))
                            .corner_radius(9),
                        )
                        .clicked()
                    {
                        apply = true;
                    }
                });
            });

        if modal.should_close() || cancel {
            self.numeric_focus.restore = Some(editor.field);
            return;
        }

        if apply {
            match validated_touch_numeric_value(&editor) {
                Ok(value) => {
                    self.apply_touch_numeric_value(editor.field, value);
                    self.numeric_focus.restore = Some(editor.field);
                    return;
                }
                Err(error) => {
                    editor.error = Some(error);
                    editor.focus_input = true;
                }
            }
        }

        self.numeric_focus.touch_editor = Some(editor);
    }

    fn apply_touch_numeric_value(&mut self, field: InputField, value: f64) {
        match field {
            InputField::DieWidth => {
                let old_width = self.inputs.die.width_mm;
                if self.lock_die_ratio && old_width > 0.0 {
                    self.inputs.die.height_mm *= value / old_width;
                }
                self.inputs.die.width_mm = value;
            }
            InputField::DieHeight => {
                let old_height = self.inputs.die.height_mm;
                if self.lock_die_ratio && old_height > 0.0 {
                    self.inputs.die.width_mm *= value / old_height;
                }
                self.inputs.die.height_mm = value;
            }
            InputField::ColumnLane => {
                self.inputs.die.column_lane_mm = value;
                if self.link_scribe_lanes {
                    self.inputs.die.row_lane_mm = value;
                }
            }
            InputField::RowLane => {
                self.inputs.die.row_lane_mm = value;
                if self.link_scribe_lanes {
                    self.inputs.die.column_lane_mm = value;
                }
            }
            InputField::WaferDiameter => self.inputs.wafer.diameter_mm = value,
            InputField::EdgeExclusion => self.inputs.wafer.edge_exclusion_mm = value,
            InputField::DefectDensity => self.inputs.process.defect_density_cm2 = value,
            InputField::ClusteringAlpha => self.inputs.process.clustering_alpha = value,
            InputField::OffsetX => self.inputs.process.offset_x_mm = value,
            InputField::OffsetY => self.inputs.process.offset_y_mm = value,
            InputField::ProbeColumns => self.inputs.probe.columns = value as u32,
            InputField::ProbeRows => self.inputs.probe.rows = value as u32,
            InputField::GridDensity => return,
        }
        self.numeric_focus.invalid_text = None;
        self.numeric_focus.rejected_text = None;
        self.recalculate();
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

fn touch_numeric_range_text(editor: &TouchNumericEditor) -> String {
    if editor.minimum == f64::MIN_POSITIVE {
        format!(
            "Allowed range: >0 to {}{}",
            compact_decimal(editor.maximum, 6),
            editor.suffix
        )
    } else {
        format!(
            "Allowed range: {} to {}{}",
            compact_decimal(editor.minimum, 6),
            compact_decimal(editor.maximum, 6),
            editor.suffix
        )
    }
}

fn validated_touch_numeric_value(editor: &TouchNumericEditor) -> Result<f64, String> {
    let Some(value) = parse_numeric_text(&editor.value_text) else {
        return Err(if editor.integer {
            "Enter a finite whole number.".to_owned()
        } else {
            "Enter a finite number.".to_owned()
        });
    };
    if editor.integer && value.fract() != 0.0 {
        return Err("Enter a finite whole number.".to_owned());
    }
    if value < editor.minimum || value > editor.maximum {
        return Err("Enter a value within the displayed range.".to_owned());
    }
    Ok(value * editor.scale_to_model)
}

impl eframe::App for YieldWorkbench {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        handle_spoken_feedback_shortcut(&context);
        let modal_open = self.report_open || self.numeric_focus.touch_editor.is_some();
        let page_scroll = if modal_open {
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
                let scroll_output = egui::ScrollArea::vertical()
                    .id_salt("main_page_scroll")
                    .scroll_source(if modal_open {
                        egui::scroll_area::ScrollSource::NONE
                    } else {
                        egui::scroll_area::ScrollSource::ALL
                    })
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
                if modal_open
                    && let Some(state) = egui::scroll_area::State::load(&context, scroll_output.id)
                {
                    let mut stationary_state = egui::scroll_area::State::default();
                    stationary_state.offset = state.offset;
                    stationary_state.store(&context, scroll_output.id);
                }
            });
        self.show_report_dialog(&context);
        self.show_touch_numeric_dialog(&context);
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
            spoken_feedback_control(ui);
        });
    } else {
        ui.horizontal(|ui| {
            ui.label(notice());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                source_link(ui);
                spoken_feedback_control(ui);
            });
        });
    }
}

#[cfg(target_arch = "wasm32")]
fn spoken_feedback_control(ui: &mut egui::Ui) {
    let enabled = ui.ctx().options(|options| options.screen_reader);
    let label = if enabled {
        "Spoken feedback: on"
    } else {
        "Spoken feedback: off"
    };
    let response = ui
        .add_sized(
            [168.0, CONTROL_HEIGHT],
            egui::Button::new(label).selected(enabled).corner_radius(7),
        )
        .on_hover_text("Toggle app-spoken keyboard feedback (Ctrl+Alt+S)");
    if response.clicked() {
        set_spoken_feedback(ui.ctx(), !enabled);
    }
    keep_focused_visible(&response);
}

#[cfg(not(target_arch = "wasm32"))]
fn spoken_feedback_control(_ui: &mut egui::Ui) {}

#[cfg(target_arch = "wasm32")]
fn handle_spoken_feedback_shortcut(context: &egui::Context) {
    let modifiers = egui::Modifiers::CTRL | egui::Modifiers::ALT;
    if context.input_mut(|input| input.consume_key(modifiers, egui::Key::S)) {
        let enabled = context.options(|options| options.screen_reader);
        set_spoken_feedback(context, !enabled);
    }
}

#[cfg(target_arch = "wasm32")]
fn set_spoken_feedback(context: &egui::Context, enabled: bool) {
    context.options_mut(|options| options.screen_reader = enabled);
    context.output_mut(|output| {
        output.events.push(egui::output::OutputEvent::ValueChanged(
            egui::WidgetInfo::selected(
                egui::WidgetType::Checkbox,
                true,
                enabled,
                "Spoken feedback",
            ),
        ));
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_spoken_feedback_shortcut(_context: &egui::Context) {}

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

fn header_controls(ui: &mut egui::Ui) -> (egui::Response, bool) {
    let report = report_button(ui);
    let reset = reset_button(ui);
    (report, reset)
}

fn report_button(ui: &mut egui::Ui) -> egui::Response {
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

    response
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

#[cfg(target_arch = "wasm32")]
fn configure_spoken_feedback(context: &egui::Context) {
    let enabled = web_sys::window()
        .and_then(|window| window.location().search().ok())
        .is_some_and(|query| spoken_feedback_requested(&query));
    context.options_mut(|options| options.screen_reader = enabled);
}

#[cfg(not(target_arch = "wasm32"))]
fn configure_spoken_feedback(_context: &egui::Context) {}

#[cfg(any(test, target_arch = "wasm32"))]
fn spoken_feedback_requested(query: &str) -> bool {
    query
        .trim_start_matches('?')
        .split('&')
        .filter_map(|parameter| parameter.split_once('='))
        .any(|(name, value)| name == "spoken" && matches!(value, "1" | "true"))
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
    let touch_editor = prefers_touch_numeric_editor(ui);
    let response = ui
        .push_id(label, |ui| {
            labeled_control_row(ui, label, 128.0, |ui, width| {
                if touch_editor {
                    touch_numeric_button_f64(
                        ui,
                        field,
                        label,
                        *value,
                        range,
                        suffix,
                        max_decimals,
                        width,
                        numeric_focus,
                    )
                } else {
                    ui.add_sized(
                        [width, CONTROL_HEIGHT],
                        egui::DragValue::new(value)
                            .range(range)
                            .speed(speed)
                            .max_decimals(max_decimals)
                            .suffix(suffix)
                            .custom_parser(move |text| parse_numeric_text_for_field(field, text))
                            .update_while_editing(false),
                    )
                }
            })
        })
        .inner;
    keep_focused_visible(&response);
    track_numeric_text(field, numeric_focus, &response);
    preserve_numeric_focus(field, numeric_focus, &response);
    response.changed()
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
    let touch_editor = prefers_touch_numeric_editor(ui);
    let response = ui
        .push_id(label, |ui| {
            labeled_control_row(ui, label, 128.0, |ui, width| {
                if touch_editor {
                    touch_numeric_button_u32(ui, field, label, *value, range, width, numeric_focus)
                } else {
                    ui.add_sized(
                        [width, CONTROL_HEIGHT],
                        egui::DragValue::new(value)
                            .range(range)
                            .speed(0.2)
                            .custom_parser(move |text| parse_numeric_text_for_field(field, text))
                            .update_while_editing(false),
                    )
                }
            })
        })
        .inner;
    keep_focused_visible(&response);
    track_numeric_text(field, numeric_focus, &response);
    preserve_numeric_focus(field, numeric_focus, &response);
    response.changed()
}

#[expect(
    clippy::too_many_arguments,
    reason = "the touch editor preserves the field's units and validation range"
)]
fn touch_numeric_button_f64(
    ui: &mut egui::Ui,
    field: InputField,
    label: &str,
    value: f64,
    range: std::ops::RangeInclusive<f64>,
    suffix: &str,
    max_decimals: usize,
    width: f32,
    numeric_focus: &mut NumericFocusState,
) -> egui::Response {
    let value_text = compact_decimal(value, max_decimals);
    let response = ui.add_sized(
        [width, CONTROL_HEIGHT],
        egui::Button::new(format!("{value_text}{suffix}"))
            .corner_radius(7)
            .sense(egui::Sense::click()),
    );
    if response.clicked() {
        numeric_focus.touch_editor = Some(TouchNumericEditor {
            field,
            label: label.to_owned(),
            value_text,
            suffix: suffix.to_owned(),
            minimum: *range.start(),
            maximum: *range.end(),
            scale_to_model: if matches!(field, InputField::ColumnLane | InputField::RowLane) {
                0.001
            } else {
                1.0
            },
            integer: false,
            focus_input: true,
            error: None,
        });
    }
    response
}

fn touch_numeric_button_u32(
    ui: &mut egui::Ui,
    field: InputField,
    label: &str,
    value: u32,
    range: std::ops::RangeInclusive<u32>,
    width: f32,
    numeric_focus: &mut NumericFocusState,
) -> egui::Response {
    let value_text = value.to_string();
    let response = ui.add_sized(
        [width, CONTROL_HEIGHT],
        egui::Button::new(&value_text)
            .corner_radius(7)
            .sense(egui::Sense::click()),
    );
    if response.clicked() {
        numeric_focus.touch_editor = Some(TouchNumericEditor {
            field,
            label: label.to_owned(),
            value_text,
            suffix: String::new(),
            minimum: *range.start() as f64,
            maximum: *range.end() as f64,
            scale_to_model: 1.0,
            integer: true,
            focus_input: true,
            error: None,
        });
    }
    response
}

fn labeled_control_row(
    ui: &mut egui::Ui,
    label: &str,
    control_width: f32,
    add_control: impl FnOnce(&mut egui::Ui, f32) -> egui::Response,
) -> egui::Response {
    if stacks_labeled_control(ui.available_width()) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let label_response = ui.label(RichText::new(label).color(theme::TEXT_MUTED));
            let width = ui.available_width();
            add_control(ui, width).labelled_by(label_response.id)
        })
        .inner
    } else {
        ui.horizontal(|ui| {
            let label_response = ui.label(RichText::new(label).color(theme::TEXT_MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                add_control(ui, control_width)
            })
            .inner
            .labelled_by(label_response.id)
        })
        .inner
    }
}

fn uses_wide_layout(available_width: f32) -> bool {
    available_width >= WIDE_LAYOUT_THRESHOLD
}

fn stacks_labeled_control(available_width: f32) -> bool {
    available_width < STACKED_CONTROL_THRESHOLD
}

fn prefers_touch_numeric_editor(ui: &egui::Ui) -> bool {
    uses_touch_safe_viewport(ui.ctx().content_rect().width())
        || ui.input(|input| input.has_touch_screen())
        || platform_has_touch_screen()
}

fn uses_touch_safe_viewport(viewport_width: f32) -> bool {
    viewport_width < TOUCH_SAFE_VIEWPORT_THRESHOLD
}

#[cfg(target_arch = "wasm32")]
fn platform_has_touch_screen() -> bool {
    web_sys::window().is_some_and(|window| window.navigator().max_touch_points() > 0)
}

#[cfg(not(target_arch = "wasm32"))]
fn platform_has_touch_screen() -> bool {
    false
}

fn wafer_map_size(available_width: f32) -> f32 {
    (available_width - 4.0).clamp(240.0, 590.0)
}

fn track_numeric_text(
    field: InputField,
    numeric_focus: &mut NumericFocusState,
    response: &egui::Response,
) {
    if response.has_focus() {
        let invalid = response
            .ctx
            .data(|data| data.get_temp::<String>(response.id))
            .is_some_and(|text| parse_numeric_text_for_field(field, &text).is_none());
        if invalid {
            numeric_focus.invalid_text = Some(field);
            numeric_focus.rejected_text = None;
        } else {
            if numeric_focus.invalid_text == Some(field) {
                numeric_focus.invalid_text = None;
            }
            if numeric_focus.rejected_text == Some(field) {
                numeric_focus.rejected_text = None;
            }
        }
    } else if response.lost_focus() && numeric_focus.invalid_text == Some(field) {
        numeric_focus.invalid_text = None;
        numeric_focus.rejected_text = Some(field);
    }
}

fn parse_numeric_text_for_field(field: InputField, text: &str) -> Option<f64> {
    let value = parse_numeric_text(text)?;
    (!field_requires_integer(field) || value.fract() == 0.0).then_some(value)
}

fn field_requires_integer(field: InputField) -> bool {
    matches!(field, InputField::ProbeColumns | InputField::ProbeRows)
}

fn parse_numeric_text(text: &str) -> Option<f64> {
    let normalized: String = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .map(|character| if character == '−' { '-' } else { character })
        .collect();
    normalized
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
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
    let response = labeled_control_row(ui, "Random-defect model", 178.0, |ui, width| {
        ui.allocate_ui_with_layout(
            vec2(width, CONTROL_HEIGHT),
            Layout::top_down(Align::Min),
            |ui| {
                let response = egui::ComboBox::from_id_salt("yield_model_selector")
                    .selected_text(yield_model_label(*model))
                    .width(width)
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
                response
            },
        )
        .inner
    });
    keep_focused_visible(&response);
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

        for width in [
            320.0, 360.0, 390.0, 412.0, 768.0, 820.0, 960.0, 976.0, 1132.0, 1440.0,
        ] {
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
                workbench.inputs.process.yield_model = YieldModel::NegativeBinomial;
                workbench.inputs.process.clustering_alpha = 2.5;
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
    fn responsive_breakpoints_match_page_and_control_constraints() {
        assert!(!uses_wide_layout(959.99));
        assert!(uses_wide_layout(960.0));
        assert!(uses_wide_layout(976.0));

        assert!(stacks_labeled_control(339.99));
        assert!(!stacks_labeled_control(340.0));

        assert!(uses_touch_safe_viewport(1_007.99));
        assert!(!uses_touch_safe_viewport(1_008.0));

        assert_eq!(wafer_map_size(256.0), 252.0);
        assert_eq!(wafer_map_size(594.0), 590.0);
    }

    #[test]
    fn narrow_numeric_controls_do_not_capture_drag_gestures() {
        let context = egui::Context::default();
        theme::install(&context);
        let mut focus = NumericFocusState::default();
        let mut senses_drag = true;

        context
            .run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(320.0, 568.0))),
                    ..Default::default()
                },
                |ui| {
                    assert!(prefers_touch_numeric_editor(ui));
                    let response = touch_numeric_button_f64(
                        ui,
                        InputField::DieWidth,
                        "Active width",
                        10.0,
                        0.25..=450.0,
                        " mm",
                        6,
                        240.0,
                        &mut focus,
                    );
                    senses_drag = response.sense.senses_drag();
                },
            )
            .drop_without_applying_deltas();

        assert!(!senses_drag);
    }

    #[test]
    fn narrow_desktop_column_keeps_inline_numeric_editing() {
        let context = egui::Context::default();
        theme::install(&context);

        context
            .run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1024.0, 768.0))),
                    ..Default::default()
                },
                |ui| {
                    ui.allocate_ui(vec2(300.0, 100.0), |ui| {
                        assert!(stacks_labeled_control(ui.available_width()));
                        assert!(!prefers_touch_numeric_editor(ui));
                    });
                },
            )
            .drop_without_applying_deltas();
    }

    #[test]
    fn touch_numeric_validation_preserves_units_and_integer_semantics() {
        let editor = TouchNumericEditor {
            field: InputField::ColumnLane,
            label: "Column scribe".to_owned(),
            value_text: "250".to_owned(),
            suffix: " μm".to_owned(),
            minimum: 0.0,
            maximum: 10_000.0,
            scale_to_model: 0.001,
            integer: false,
            focus_input: false,
            error: None,
        };
        assert_eq!(validated_touch_numeric_value(&editor), Ok(0.25));

        let out_of_range = TouchNumericEditor {
            value_text: "10001".to_owned(),
            ..editor
        };
        assert_eq!(
            validated_touch_numeric_value(&out_of_range),
            Err("Enter a value within the displayed range.".to_owned())
        );

        let integer_editor = TouchNumericEditor {
            field: InputField::ProbeColumns,
            label: "Columns per step".to_owned(),
            value_text: "2.5".to_owned(),
            suffix: String::new(),
            minimum: 1.0,
            maximum: 128.0,
            scale_to_model: 1.0,
            integer: true,
            focus_input: false,
            error: None,
        };
        assert_eq!(
            validated_touch_numeric_value(&integer_editor),
            Err("Enter a finite whole number.".to_owned())
        );
    }

    #[test]
    fn touch_numeric_apply_preserves_linked_geometry() {
        let mut workbench = YieldWorkbench {
            lock_die_ratio: true,
            link_scribe_lanes: true,
            ..Default::default()
        };

        workbench.apply_touch_numeric_value(InputField::DieWidth, 20.0);
        assert_eq!(workbench.inputs.die.width_mm, 20.0);
        assert_eq!(workbench.inputs.die.height_mm, 16.0);

        workbench.apply_touch_numeric_value(InputField::ColumnLane, 0.25);
        assert_eq!(workbench.inputs.die.column_lane_mm, 0.25);
        assert_eq!(workbench.inputs.die.row_lane_mm, 0.25);
        assert!(workbench.analysis.is_some());
    }

    #[test]
    fn numeric_entry_validation_matches_drag_value_semantics() {
        for valid in ["10", " 10.25 ", "−3.5", "1 000", "2e-3"] {
            assert!(
                parse_numeric_text(valid).is_some(),
                "{valid:?} should be valid"
            );
        }
        for invalid in ["", "-", "abc", "NaN", "inf", "1.2.3"] {
            assert!(
                parse_numeric_text(invalid).is_none(),
                "{invalid:?} should be invalid"
            );
        }

        assert_eq!(
            parse_numeric_text_for_field(InputField::ProbeColumns, "2"),
            Some(2.0)
        );
        assert_eq!(
            parse_numeric_text_for_field(InputField::ProbeColumns, "2.5"),
            None
        );
        assert_eq!(
            parse_numeric_text_for_field(InputField::DieWidth, "NaN"),
            None
        );
    }

    #[test]
    fn spoken_feedback_query_requires_an_explicit_enabled_value() {
        assert!(spoken_feedback_requested("?spoken=1"));
        assert!(spoken_feedback_requested("?mode=compact&spoken=true"));
        assert!(!spoken_feedback_requested(""));
        assert!(!spoken_feedback_requested("?spoken=0"));
        assert!(!spoken_feedback_requested("?unspoken=1"));
    }

    #[test]
    fn theme_uses_touch_sized_interactions() {
        let context = egui::Context::default();
        theme::install(&context);
        assert_eq!(
            context.style_of(egui::Theme::Light).spacing.interact_size.y,
            CONTROL_HEIGHT
        );
    }

    #[test]
    fn report_modal_exposes_dialog_semantics_and_initial_close_focus() {
        let context = egui::Context::default();
        context.enable_accesskit();
        theme::install(&context);
        let mut workbench = YieldWorkbench {
            report_open: true,
            focus_report_dialog: true,
            ..Default::default()
        };

        let raw_input = || RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500.0, 800.0))),
            ..Default::default()
        };
        context
            .run_ui(raw_input(), |ui| workbench.show_report_dialog(ui.ctx()))
            .drop_without_applying_deltas();
        let output = context.run_ui(raw_input(), |ui| workbench.show_report_dialog(ui.ctx()));

        let focused = context
            .memory(|memory| memory.focused())
            .expect("the close control should receive initial focus");
        assert!(context.memory(|memory| memory.top_modal_layer().is_some()));

        let update = output
            .platform_output
            .accesskit_update
            .as_ref()
            .expect("AccessKit output should be enabled");
        let focused_node = update
            .nodes
            .iter()
            .find(|(id, _)| *id == focused.accesskit_id())
            .map(|(_, node)| node)
            .expect("the focused control should have an AccessKit node");
        assert_eq!(focused_node.label(), Some("Close report export"));
        assert!(update.nodes.iter().any(|(_, node)| {
            node.role() == egui::accesskit::Role::Dialog
                && node.label() == Some("Export report")
                && node.is_modal()
        }));

        output.drop_without_applying_deltas();
    }

    #[test]
    fn touch_numeric_editor_applies_with_enter() {
        let context = egui::Context::default();
        theme::install(&context);
        let mut workbench = YieldWorkbench::default();
        workbench.numeric_focus.touch_editor = Some(TouchNumericEditor {
            field: InputField::DieWidth,
            label: "Active width".to_owned(),
            value_text: "20".to_owned(),
            suffix: " mm".to_owned(),
            minimum: 0.25,
            maximum: 450.0,
            scale_to_model: 1.0,
            integer: false,
            focus_input: true,
            error: None,
        });

        let raw_input = |events| RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(320.0, 568.0))),
            events,
            ..Default::default()
        };
        for _ in 0..2 {
            context
                .run_ui(raw_input(Vec::new()), |ui| {
                    workbench.show_touch_numeric_dialog(ui.ctx());
                })
                .drop_without_applying_deltas();
        }

        context
            .run_ui(
                raw_input(vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }]),
                |ui| workbench.show_touch_numeric_dialog(ui.ctx()),
            )
            .drop_without_applying_deltas();

        assert!(workbench.numeric_focus.touch_editor.is_none());
        assert_eq!(workbench.inputs.die.width_mm, 20.0);
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
