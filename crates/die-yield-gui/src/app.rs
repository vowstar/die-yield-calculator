use crate::{delivery, report, theme};
use die_yield_core::{FabricationInputs, ValidationErrors, WaferAnalysis, analyze};
use die_yield_render::{MIN_VISIBLE_SCRIBE_POINTS, WaferPalette, WaferScene, paint_wafer};
use eframe::egui;
use egui::{Align, Color32, Layout, Margin, RichText, Stroke, vec2};
use serde::{Deserialize, Serialize};

const WIDE_LAYOUT_THRESHOLD: f32 = 990.0;
const HEADER_CONTROL_HEIGHT: f32 = 38.0;
const PROJECT_URL: &str = "https://github.com/vowstar/die-yield-calculator";
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
            auto_update_note(ui);
            ui.add_space(12.0);

            section_heading(ui, "WAFER", SectionGlyph::Wafer);
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
                section_heading(ui, "DIE & SCRIBE", SectionGlyph::DieGrid);
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
            section_heading(ui, "PROCESS & ALIGNMENT", SectionGlyph::Alignment);
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
            section_heading(ui, "PROBE ARRAY", SectionGlyph::ProbeArray);
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

    fn show_report_dialog(&mut self, context: &egui::Context) {
        if !self.report_open {
            return;
        }

        let mut open = self.report_open;
        let mut close = false;
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
                            .add(
                                egui::Button::new(RichText::new("×").size(20.0))
                                    .frame(false),
                            )
                            .on_hover_text("Close report export")
                            .clicked();
                    });
                });
                ui.label(
                    RichText::new(
                        "A4 layout with the wafer map, key results, process parameters, and legend.",
                    )
                    .color(theme::TEXT_MUTED),
                );
                ui.add_space(12.0);

                section_heading(ui, "EXPORT FORMAT", SectionGlyph::Wafer);
                let mut show_format_buttons = |ui: &mut egui::Ui| {
                    for (format, label, detail) in [
                        (report::ReportFormat::Png, "PNG", "2× raster"),
                        (report::ReportFormat::Svg, "SVG", "portable vector"),
                        (report::ReportFormat::Pdf, "PDF", "A4 document"),
                    ] {
                        if report_format_button(
                            ui,
                            self.analysis.is_some(),
                            format!("{label}\n{detail}"),
                            compact,
                        ) {
                            export_format = Some(format);
                        }
                    }
                };
                if compact {
                    ui.vertical(&mut show_format_buttons);
                } else {
                    ui.horizontal(&mut show_format_buttons);
                }

                ui.add_space(10.0);
                let print_response = ui.add_enabled(
                    self.analysis.is_some(),
                    egui::Button::new(
                        RichText::new("Print report")
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(theme::ACCENT)
                    .stroke(Stroke::new(1.0, theme::ACCENT))
                    .min_size(vec2(ui.available_width(), 40.0))
                    .corner_radius(9),
                );
                if print_response.clicked() {
                    print = true;
                }
                ui.label(
                    RichText::new(print_hint())
                        .small()
                        .color(theme::TEXT_MUTED),
                );

                if self.analysis.is_none() {
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
                            Color32::from_rgba_unmultiplied(
                                color.r(),
                                color.g(),
                                color.b(),
                                70,
                            ),
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
            .analysis
            .as_ref()
            .ok_or_else(|| "No valid analysis is available".to_owned())
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
            .analysis
            .as_ref()
            .ok_or_else(|| "No valid analysis is available".to_owned())
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
        RichText::new("Murphy yield model  •  Results are planning estimates")
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
    let response = ui.add_sized(
        [82.0, HEADER_CONTROL_HEIGHT],
        egui::Button::new(RichText::new("Reset").color(theme::TEXT_MUTED))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .corner_radius(12),
    );
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(response.rect.left() + 15.0, response.rect.center().y),
        vec2(15.0, 15.0),
    );
    let icon_color = ui.style().interact(&response).fg_stroke.color;
    paint_reset_glyph(ui.painter(), icon_rect, icon_color);

    response
        .on_hover_text("Restore the recommended starting values")
        .clicked()
}

fn report_button(ui: &mut egui::Ui) -> bool {
    let response = ui.add_sized(
        [94.0, HEADER_CONTROL_HEIGHT],
        egui::Button::new(RichText::new("Report").color(theme::TEXT_MUTED))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .corner_radius(12),
    );
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(response.rect.left() + 16.0, response.rect.center().y),
        vec2(16.0, 16.0),
    );
    let icon_color = ui.style().interact(&response).fg_stroke.color;
    paint_report_glyph(ui.painter(), icon_rect, icon_color);

    response
        .on_hover_text("Export or print a styled analysis report")
        .clicked()
}

fn report_format_button(ui: &mut egui::Ui, enabled: bool, label: String, compact: bool) -> bool {
    let width = if compact { ui.available_width() } else { 126.0 };
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).line_height(Some(17.0)))
            .min_size(vec2(width, if compact { 46.0 } else { 54.0 }))
            .corner_radius(9),
    )
    .clicked()
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
