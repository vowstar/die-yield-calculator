//! Testable wafer-scene model and egui painter.

use die_yield_core::{DieClass, WaferAnalysis};
use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

/// Preferred minimum scribe-lane thickness in logical display points.
///
/// The painter may reduce this only when a die pitch is too small to preserve
/// both a visible lane and a visible cell.
pub const MIN_VISIBLE_SCRIBE_POINTS: f32 = 1.25;

const MIN_VISIBLE_CELL_POINTS: f32 = 0.35;

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
    /// Column and row scribe-lane widths in millimetres.
    pub scribe_lane_mm: [f64; 2],
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
            scribe_lane_mm: [inputs.die.column_lane_mm, inputs.die.row_lane_mm],
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
    /// Solid separation between adjacent die cells.
    pub scribe: Color32,
    /// Orientation crosshair.
    pub guide: Color32,
}

impl Default for WaferPalette {
    fn default() -> Self {
        Self {
            backdrop: Color32::from_rgb(13, 23, 31),
            wafer: Color32::from_rgb(25, 40, 49),
            wafer_highlight: Color32::from_rgb(31, 51, 60),
            wafer_outline: Color32::from_rgb(91, 113, 127),
            usable_outline: Color32::from_rgb(100, 169, 160),
            productive: Color32::from_rgb(61, 151, 139),
            defective: Color32::from_rgb(190, 79, 83),
            boundary: Color32::from_rgb(187, 132, 47),
            excluded: Color32::from_rgb(74, 93, 110),
            scribe: Color32::from_rgb(8, 17, 23),
            guide: Color32::from_rgba_unmultiplied(135, 157, 169, 45),
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
        Color32::from_rgba_unmultiplied(0, 124, 116, 14),
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

    // Paint the pitch-sized lane layer first so adjacent cells share one solid,
    // continuous separator instead of relying on subpixel gaps between fills.
    for cell in &scene.cells {
        let (pitch_rect, _) = cell_rects(cell, scene.scribe_lane_mm, center, scale);
        painter.rect_filled(pitch_rect, 0, palette.scribe);
    }

    for cell in &scene.cells {
        let (_, cell_rect) = cell_rects(cell, scene.scribe_lane_mm, center, scale);
        let fill = match cell.tone {
            CellTone::Productive => palette.productive,
            CellTone::Defective => palette.defective,
            CellTone::Boundary => palette.boundary,
            CellTone::Excluded => palette.excluded,
        };
        painter.rect_filled(cell_rect, 0, fill);
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

fn cell_rects(
    cell: &SceneCell,
    scribe_lane_mm: [f64; 2],
    center: Pos2,
    scale: f32,
) -> (Rect, Rect) {
    let cell_center = pos2(
        center.x + cell.center_mm[0] as f32 * scale,
        center.y - cell.center_mm[1] as f32 * scale,
    );
    let pitch_size = vec2(
        ((cell.size_mm[0] + scribe_lane_mm[0]) as f32 * scale).max(0.0),
        ((cell.size_mm[1] + scribe_lane_mm[1]) as f32 * scale).max(0.0),
    );
    let visible_lane = vec2(
        visible_scribe_width(scribe_lane_mm[0] as f32 * scale, pitch_size.x),
        visible_scribe_width(scribe_lane_mm[1] as f32 * scale, pitch_size.y),
    );
    let cell_size = vec2(
        (pitch_size.x - visible_lane.x).max(0.0),
        (pitch_size.y - visible_lane.y).max(0.0),
    );

    (
        Rect::from_center_size(cell_center, pitch_size),
        Rect::from_center_size(cell_center, cell_size),
    )
}

fn visible_scribe_width(physical_points: f32, pitch_points: f32) -> f32 {
    let available = (pitch_points - MIN_VISIBLE_CELL_POINTS).max(0.0);
    physical_points
        .max(MIN_VISIBLE_SCRIBE_POINTS)
        .min(available)
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
        assert_eq!(scene.scribe_lane_mm, [0.12, 0.12]);
    }

    #[test]
    fn subpixel_scribe_lanes_keep_a_solid_display_gap() {
        let cell = SceneCell {
            center_mm: [0.0, 0.0],
            size_mm: [10.0, 8.0],
            tone: CellTone::Productive,
        };
        let (pitch_rect, cell_rect) = cell_rects(&cell, [0.001, 0.001], Pos2::ZERO, 10.0);

        assert!((pitch_rect.width() - cell_rect.width()) >= MIN_VISIBLE_SCRIBE_POINTS - 1.0e-4);
        assert!((pitch_rect.height() - cell_rect.height()) >= MIN_VISIBLE_SCRIBE_POINTS - 1.0e-4);
    }

    #[test]
    fn physical_scribe_width_wins_when_it_is_already_visible() {
        let cell = SceneCell {
            center_mm: [0.0, 0.0],
            size_mm: [10.0, 8.0],
            tone: CellTone::Productive,
        };
        let (pitch_rect, cell_rect) = cell_rects(&cell, [0.5, 0.25], Pos2::ZERO, 10.0);

        assert!((pitch_rect.width() - cell_rect.width() - 5.0).abs() < 1.0e-4);
        assert!((pitch_rect.height() - cell_rect.height() - 2.5).abs() < 1.0e-4);
    }

    #[test]
    fn painter_accepts_size_and_density_matrix() {
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
            let analysis = analyze(&inputs).expect("parameter combination should be valid");
            let scene = WaferScene::from_analysis(&analysis);

            egui::__run_test_ui(|ui| {
                let response = paint_wafer(ui, &scene, 320.0);
                assert!(response.rect.is_positive());
            });
        }
    }
}
