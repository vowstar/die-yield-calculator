//! Wafer-scene types and egui painting support.

use die_yield_core::FabricationInputs;

/// Presentation-independent bounds used by the wafer-map painter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneBounds {
    /// Full wafer diameter represented by the scene.
    pub diameter_mm: f64,
    /// Diameter remaining after radial edge exclusion.
    pub usable_diameter_mm: f64,
}

impl SceneBounds {
    /// Creates scene bounds from normalized fabrication inputs.
    #[must_use]
    pub fn from_inputs(inputs: &FabricationInputs) -> Self {
        Self {
            diameter_mm: inputs.wafer.diameter_mm,
            usable_diameter_mm: inputs.wafer.diameter_mm - 2.0 * inputs.wafer.edge_exclusion_mm,
        }
    }
}
