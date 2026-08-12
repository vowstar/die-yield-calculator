use crate::theme;
use die_yield_core::{FabricationInputs, ValidationErrors, WaferAnalysis, analyze};
use die_yield_render::{MIN_VISIBLE_SCRIBE_POINTS, WaferPalette, WaferScene, paint_wafer};
use eframe::egui;
use egui::{Align, Color32, Layout, Margin, RichText, Stroke, vec2};
use serde::{Deserialize, Serialize};

const WIDE_LAYOUT_THRESHOLD: f32 = 990.0;
const HEADER_CONTROL_HEIGHT: f32 = 38.0;
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
}

impl Default for YieldWorkbench {
    fn default() -> Self {
        let mut workbench = Self {
            inputs: FabricationInputs::default(),
            lock_die_ratio: false,
            link_scribe_lanes: true,
            analysis: None,
            validation: None,
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
        match analyze(&self.inputs) {
            Ok(analysis) => {
                self.analysis = Some(analysis);
                self.validation = None;
            }
            Err(errors) => {
                self.analysis = None;
                self.validation = Some(errors);
            }
        }
    }

    fn show_header(&mut self, ui: &mut egui::Ui) {
        let compact = ui.available_width() < 650.0;
        let mut reset = false;
        if compact {
            ui.vertical(|ui| {
                ui.horizontal(header_brand);
                ui.add_space(5.0);
                ui.horizontal(|ui| show_header_actions(ui, false, &mut reset));
            });
        } else {
            ui.horizontal(|ui| {
                header_brand(ui);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    show_header_actions(ui, true, &mut reset);
                });
            });
        }
        if reset {
            *self = Self::default();
        }
    }

    fn show_workspace(&mut self, ui: &mut egui::Ui) {
        let wide = ui.available_width() >= WIDE_LAYOUT_THRESHOLD;
        self.show_summary(ui, wide);
        ui.add_space(8.0);

        if wide {
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
            self.show_visual_card(ui);
            ui.add_space(6.0);
            self.show_settings_card(ui);
        }
    }

    fn show_summary(&self, ui: &mut egui::Ui, wide: bool) {
        let Some(analysis) = self.analysis.as_ref() else {
            self.show_validation(ui);
            return;
        };
        let summary = analysis.summary;
        let values: [(&str, String, String, Color32); 4] = [
            (
                "MODEL YIELD",
                format!("{:.2}%", summary.yield_fraction * 100.0),
                "Murphy estimate".to_owned(),
                theme::ACCENT,
            ),
            (
                "EXPECTED GOOD",
                format_integer(summary.expected_good),
                format!("of {} usable", format_integer(summary.geometric_usable)),
                theme::BLUE,
            ),
            (
                "DIE LOSS",
                format_integer(summary.expected_defective),
                format!("{} boundary sites", format_integer(summary.partial)),
                theme::CORAL,
            ),
            (
                "TOUCHDOWNS",
                format_integer(analysis.probe.touchdown_count),
                format!("{} sites per step", analysis.probe.sites_per_touchdown),
                theme::AMBER,
            ),
        ];

        let columns = if wide { 4 } else { 2 };
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
    }

    fn show_visual_card(&self, ui: &mut egui::Ui) {
        card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Wafer map").size(18.0).strong());
                    ui.label(
                        RichText::new("Geometric placement and modeled process loss")
                            .small()
                            .color(theme::TEXT_MUTED),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    pill(
                        ui,
                        &format!("Ø {}", wafer_size_label(self.inputs.wafer.diameter_mm)),
                        theme::BLUE,
                    );
                });
            });
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
                    legend(ui, "Expected good", palette.productive);
                    legend(ui, "Modeled loss", palette.defective);
                    legend(ui, "Boundary", palette.boundary);
                    legend(ui, "Edge band", palette.excluded);
                    legend(ui, "Scribe lane", palette.scribe);
                });
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "{} mapped sites  •  {:.2} mm² active area  •  scribe shown at ≥{MIN_VISIBLE_SCRIBE_POINTS:.2} pt",
                        format_integer(scene.cells.len() as u64),
                        self.inputs.die.width_mm * self.inputs.die.height_mm
                    ))
                    .small()
                    .color(theme::TEXT_MUTED),
                );
            } else {
                self.show_validation(ui);
            }
        });
    }

    fn show_settings_card(&mut self, ui: &mut egui::Ui) {
        let before = self.inputs;
        card().show(ui, |ui| {
            ui.label(RichText::new("Process setup").size(18.0).strong());
            ui.label(
                RichText::new("Changes update the map immediately")
                    .small()
                    .color(theme::TEXT_MUTED),
            );
            ui.add_space(12.0);

            section_heading(ui, "WAFER");
            ui.horizontal_wrapped(|ui| {
                for diameter in WAFER_PRESETS_MM {
                    let selected = (self.inputs.wafer.diameter_mm - diameter).abs() < f64::EPSILON;
                    if ui
                        .add(
                            egui::Button::new(wafer_size_label(diameter))
                                .selected(selected)
                                .corner_radius(7),
                        )
                        .on_hover_text(format!(
                            "Set wafer diameter to {}",
                            wafer_size_label(diameter)
                        ))
                        .clicked()
                    {
                        self.inputs.wafer.diameter_mm = diameter;
                        self.inputs.wafer.edge_exclusion_mm =
                            self.inputs.wafer.edge_exclusion_mm.min(diameter * 0.1);
                    }
                }
            });
            input_row_f64(
                ui,
                &format!(
                    "Diameter ({})",
                    wafer_inches_label(self.inputs.wafer.diameter_mm)
                ),
                &mut self.inputs.wafer.diameter_mm,
                25.0..=450.0,
                1.0,
                " mm",
            );
            input_row_f64(
                ui,
                "Edge exclusion",
                &mut self.inputs.wafer.edge_exclusion_mm,
                0.0..=100.0,
                0.1,
                " mm",
            );

            section_divider(ui);
            ui.horizontal(|ui| {
                section_heading(ui, "DIE & SCRIBE");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.toggle_value(&mut self.lock_die_ratio, "Lock ratio")
                        .on_hover_text("Preserve the current die aspect ratio");
                });
            });
            let old_width = self.inputs.die.width_mm;
            let old_height = self.inputs.die.height_mm;
            let width_changed = input_row_f64(
                ui,
                "Active width",
                &mut self.inputs.die.width_mm,
                0.25..=450.0,
                0.1,
                " mm",
            );
            if width_changed && self.lock_die_ratio && old_width > 0.0 {
                self.inputs.die.height_mm = old_height * self.inputs.die.width_mm / old_width;
            }
            let height_changed = input_row_f64(
                ui,
                "Active height",
                &mut self.inputs.die.height_mm,
                0.25..=450.0,
                0.1,
                " mm",
            );
            if height_changed && self.lock_die_ratio && old_height > 0.0 {
                self.inputs.die.width_mm = old_width * self.inputs.die.height_mm / old_height;
            }
            let column_changed = input_row_f64(
                ui,
                "Column lane",
                &mut self.inputs.die.column_lane_mm,
                0.0..=10.0,
                0.01,
                " mm",
            );
            if column_changed && self.link_scribe_lanes {
                self.inputs.die.row_lane_mm = self.inputs.die.column_lane_mm;
            }
            let row_changed = input_row_f64(
                ui,
                "Row lane",
                &mut self.inputs.die.row_lane_mm,
                0.0..=10.0,
                0.01,
                " mm",
            );
            if row_changed && self.link_scribe_lanes {
                self.inputs.die.column_lane_mm = self.inputs.die.row_lane_mm;
            }
            ui.checkbox(&mut self.link_scribe_lanes, "Link scribe lanes");

            section_divider(ui);
            section_heading(ui, "PROCESS & ALIGNMENT");
            input_row_f64(
                ui,
                "Defect density",
                &mut self.inputs.process.defect_density_cm2,
                0.0..=100.0,
                0.01,
                " /cm²",
            );
            input_row_f64(
                ui,
                "Horizontal phase",
                &mut self.inputs.process.offset_x_mm,
                -450.0..=450.0,
                0.05,
                " mm",
            );
            input_row_f64(
                ui,
                "Vertical phase",
                &mut self.inputs.process.offset_y_mm,
                -450.0..=450.0,
                0.05,
                " mm",
            );
            ui.checkbox(
                &mut self.inputs.process.die_at_origin,
                "Center a die at wafer origin",
            );

            section_divider(ui);
            section_heading(ui, "PROBE ARRAY");
            input_row_u32(
                ui,
                "Columns per step",
                &mut self.inputs.probe.columns,
                1..=128,
            );
            input_row_u32(ui, "Rows per step", &mut self.inputs.probe.rows, 1..=128);

            if self.validation.is_some() {
                ui.add_space(8.0);
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
                    RichText::new("Check the highlighted setup")
                        .strong()
                        .color(theme::CORAL),
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
}

impl eframe::App for YieldWorkbench {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::CANVAS).inner_margin(0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
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
                                            ui.label(
                                                RichText::new(
                                                    "Murphy yield model  •  Results are planning estimates",
                                                )
                                                .small()
                                                .color(theme::TEXT_MUTED),
                                            );
                                        },
                                    );
                                });
                            });
                    });
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

fn show_header_actions(ui: &mut egui::Ui, reverse_order: bool, reset: &mut bool) {
    let platform = if cfg!(target_arch = "wasm32") {
        "WEB"
    } else {
        "NATIVE"
    };
    let reset_button = |ui: &mut egui::Ui| {
        ui.add_sized(
            [66.0, HEADER_CONTROL_HEIGHT],
            egui::Button::new(RichText::new("Reset").color(theme::TEXT_MUTED))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::new(1.0, theme::BORDER))
                .corner_radius(12),
        )
        .on_hover_text("Restore the recommended starting values")
        .clicked()
    };

    if reverse_order {
        *reset |= reset_button(ui);
        header_pill(ui, platform, theme::BLUE);
        header_pill(ui, "LIVE MODEL", theme::ACCENT);
    } else {
        header_pill(ui, "LIVE MODEL", theme::ACCENT);
        header_pill(ui, platform, theme::BLUE);
        *reset |= reset_button(ui);
    }
}

fn header_pill(ui: &mut egui::Ui, text: &str, color: Color32) {
    let width = (text.chars().count() as f32 * 6.4 + 22.0).max(58.0);
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
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                vec2(width, HEADER_CONTROL_HEIGHT),
                Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| {
                    ui.label(RichText::new(text).size(10.5).strong().color(color));
                },
            );
        });
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

fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(11.0).strong().color(theme::BLUE));
}

fn section_divider(ui: &mut egui::Ui) {
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);
}

fn input_row_f64(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    speed: f64,
    suffix: &str,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(theme::TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            changed = ui
                .add_sized(
                    [128.0, 32.0],
                    egui::DragValue::new(value)
                        .range(range)
                        .speed(speed)
                        .max_decimals(3)
                        .suffix(suffix),
                )
                .changed();
        });
    });
    changed
}

fn input_row_u32(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(theme::TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            changed = ui
                .add_sized(
                    [128.0, 32.0],
                    egui::DragValue::new(value).range(range).speed(0.2),
                )
                .changed();
        });
    });
    changed
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

fn wafer_size_label(diameter_mm: f64) -> String {
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

        for width in [480.0, 820.0, 1440.0] {
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
                assert!(workbench.analysis.is_some());

                let context = egui::Context::default();
                theme::install(&context);
                let output = context.run_ui(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 900.0))),
                        ..Default::default()
                    },
                    |ui| workbench.show_workspace(ui),
                );
                output.drop_without_applying_deltas();
            }
        }
    }

    #[test]
    fn invalid_setup_surfaces_validation_without_stale_results() {
        let mut workbench = YieldWorkbench::default();
        workbench.inputs.wafer.diameter_mm = 10.0;
        workbench.inputs.process.defect_density_cm2 = -1.0;
        workbench.recalculate();

        assert!(workbench.analysis.is_none());
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
