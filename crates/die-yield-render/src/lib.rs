//! Testable wafer-scene model and egui painter.

use die_yield_core::{DieClass, WaferAnalysis};
use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

/// Semantic color role of one die cell in a rendered scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellTone {
    /// Expected good die.
    Productive,
    /// Statistically expected defective die.
    Defective,
    /// Die intersecting the usable-radius boundary.
    Boundary,
    /// Candidate wholly inside the edge-exclusion band.
    Excluded,
}

/// Presentation-independent cell geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneCell {
    /// Centre relative to the wafer origin in millimetres.
    pub center_mm: [f64; 2],
    /// Active die size in millimetres.
    pub size_mm: [f64; 2],
    /// Semantic cell tone.
    pub tone: CellTone,
}

/// Complete scene consumed by the egui painter.
#[derive(Clone, Debug, PartialEq)]
pub struct WaferScene {
    /// Nominal wafer diameter in millimetres.
    pub diameter_mm: f64,
    /// Diameter after edge exclusion in millimetres.
    pub usable_diameter_mm: f64,
    /// Die cells intersecting the wafer.
    pub cells: Vec<SceneCell>,
}

impl WaferScene {
    /// Builds a stable visual scene from a calculation result.
    #[must_use]
    pub fn from_analysis(analysis: &WaferAnalysis) -> Self {
        let inputs = analysis.normalized_inputs;
        let cells = analysis
            .placements
            .iter()
            .map(|placement| SceneCell {
                center_mm: placement.center_mm,
                size_mm: placement.size_mm,
                tone: if placement.defective {
                    CellTone::Defective
                } else {
                    match placement.class {
                        DieClass::Usable => CellTone::Productive,
                        DieClass::Partial => CellTone::Boundary,
                        DieClass::EdgeExclusion => CellTone::Excluded,
                    }
                },
            })
            .collect();

        Self {
            diameter_mm: inputs.wafer.diameter_mm,
            usable_diameter_mm: inputs.wafer.diameter_mm - 2.0 * inputs.wafer.edge_exclusion_mm,
            cells,
        }
    }

    /// Counts cells with the requested semantic tone.
    #[must_use]
    pub fn count(&self, tone: CellTone) -> usize {
        self.cells.iter().filter(|cell| cell.tone == tone).count()
    }
}

/// Original dark palette used by the wafer visualization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaferPalette {
    /// Card backdrop around the wafer.
    pub backdrop: Color32,
    /// Outer wafer fill.
    pub wafer: Color32,
    /// Subtle centre glow.
    pub wafer_highlight: Color32,
    /// Wafer-edge line.
    pub wafer_outline: Color32,
    /// Usable-area boundary.
    pub usable_outline: Color32,
    /// Expected good die.
    pub productive: Color32,
    /// Expected defective die.
    pub defective: Color32,
    /// Usable-boundary intersection.
    pub boundary: Color32,
    /// Edge-exclusion die.
    pub excluded: Color32,
    /// Orientation crosshair.
    pub guide: Color32,
}

impl Default for WaferPalette {
    fn default() -> Self {
        Self {
            backdrop: Color32::from_rgb(9, 16, 29),
            wafer: Color32::from_rgb(18, 32, 50),
            wafer_highlight: Color32::from_rgb(23, 43, 64),
            wafer_outline: Color32::from_rgb(76, 105, 139),
            usable_outline: Color32::from_rgb(83, 214, 201),
            productive: Color32::from_rgb(54, 190, 178),
            defective: Color32::from_rgb(239, 99, 133),
            boundary: Color32::from_rgb(245, 183, 75),
            excluded: Color32::from_rgb(65, 83, 108),
            guide: Color32::from_rgba_unmultiplied(128, 160, 191, 55),
        }
    }
}

/// Paints a square wafer map and returns its hover response.
pub fn paint_wafer(ui: &mut Ui, scene: &WaferScene, desired_side: f32) -> Response {
    let side = desired_side.max(240.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    let palette = WaferPalette::default();
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 18, palette.backdrop);

    let plot = rect.shrink(24.0);
    let center = plot.center();
    let radius = plot.width().min(plot.height()) * 0.5;
    let scale = radius / (scene.diameter_mm as f32 * 0.5);

    painter.circle_filled(
        center,
        radius + 4.0,
        Color32::from_rgba_unmultiplied(41, 215, 199, 12),
    );
    painter.circle_filled(center, radius, palette.wafer);
    painter.circle_filled(center, radius * 0.82, palette.wafer_highlight);

    painter.line_segment(
        [
            pos2(center.x - radius, center.y),
            pos2(center.x + radius, center.y),
        ],
        Stroke::new(1.0, palette.guide),
    );
    painter.line_segment(
        [
            pos2(center.x, center.y - radius),
            pos2(center.x, center.y + radius),
        ],
        Stroke::new(1.0, palette.guide),
    );

    for cell in &scene.cells {
        let cell_rect = cell_rect(cell, center, scale);
        let fill = match cell.tone {
            CellTone::Productive => palette.productive,
            CellTone::Defective => palette.defective,
            CellTone::Boundary => palette.boundary,
            CellTone::Excluded => palette.excluded,
        };
        let corner_radius = (cell_rect.width().min(cell_rect.height()) * 0.12).clamp(0.0, 2.0);
        painter.rect_filled(cell_rect, corner_radius, fill);
    }

    let usable_radius = radius * (scene.usable_diameter_mm / scene.diameter_mm) as f32;
    painter.circle_stroke(
        center,
        usable_radius,
        Stroke::new(1.5, palette.usable_outline),
    );
    painter.circle_stroke(center, radius, Stroke::new(2.0, palette.wafer_outline));

    let notch_y = center.y + radius;
    painter.line_segment(
        [
            pos2(center.x - 8.0, notch_y - 1.0),
            pos2(center.x, notch_y - 7.0),
        ],
        Stroke::new(2.0, palette.wafer_outline),
    );
    painter.line_segment(
        [
            pos2(center.x, notch_y - 7.0),
            pos2(center.x + 8.0, notch_y - 1.0),
        ],
        Stroke::new(2.0, palette.wafer_outline),
    );
    painter.rect_stroke(
        rect,
        18,
        Stroke::new(1.0, Color32::from_rgb(31, 48, 69)),
        StrokeKind::Inside,
    );

    response.on_hover_ui(|ui| {
        ui.label(format!("{:.0} mm wafer", scene.diameter_mm));
        ui.label(format!("{} mapped sites", scene.cells.len()));
    })
}

fn cell_rect(cell: &SceneCell, center: Pos2, scale: f32) -> Rect {
    let cell_center = pos2(
        center.x + cell.center_mm[0] as f32 * scale,
        center.y - cell.center_mm[1] as f32 * scale,
    );
    let size = vec2(
        (cell.size_mm[0] as f32 * scale - 0.65).max(0.6),
        (cell.size_mm[1] as f32 * scale - 0.65).max(0.6),
    );
    Rect::from_center_size(cell_center, size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use die_yield_core::{FabricationInputs, analyze};

    #[test]
    fn scene_preserves_analysis_counts() {
        let analysis = analyze(&FabricationInputs::default()).expect("defaults should be valid");
        let scene = WaferScene::from_analysis(&analysis);

        assert_eq!(scene.count(CellTone::Productive), 708);
        assert_eq!(scene.count(CellTone::Defective), 59);
        assert_eq!(scene.count(CellTone::Boundary), 124);
        assert_eq!(scene.count(CellTone::Excluded), 40);
        assert_eq!(scene.cells.len(), analysis.placements.len());
    }

    #[test]
    fn painter_accepts_size_and_density_matrix() {
        for (diameter, density) in [(75.0, 0.0), (150.0, 0.1), (300.0, 1.0), (450.0, 5.0)] {
            let mut inputs = FabricationInputs::default();
            inputs.wafer.diameter_mm = diameter;
            inputs.wafer.edge_exclusion_mm = (diameter * 0.02).max(1.0);
            inputs.process.defect_density_cm2 = density;
            let analysis = analyze(&inputs).expect("parameter combination should be valid");
            let scene = WaferScene::from_analysis(&analysis);

            egui::__run_test_ui(|ui| {
                let response = paint_wafer(ui, &scene, 320.0);
                assert!(response.rect.is_positive());
            });
        }
    }
}
