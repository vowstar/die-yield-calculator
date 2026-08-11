//! Platform-independent inputs and results for wafer-yield analysis.

use serde::{Deserialize, Serialize};

/// Physical dimensions of a rectangular die and its surrounding scribe lanes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DieGeometry {
    /// Active die width in millimetres.
    pub width_mm: f64,
    /// Active die height in millimetres.
    pub height_mm: f64,
    /// Horizontal separation between adjacent die columns in millimetres.
    pub column_lane_mm: f64,
    /// Vertical separation between adjacent die rows in millimetres.
    pub row_lane_mm: f64,
}

/// Wafer dimensions and edge-exclusion policy.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaferGeometry {
    /// Nominal wafer diameter in millimetres.
    pub diameter_mm: f64,
    /// Radial exclusion measured inward from the wafer edge in millimetres.
    pub edge_exclusion_mm: f64,
}

/// Placement phase and process-yield parameters.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProcessSettings {
    /// Random defect density per square centimetre.
    pub defect_density_cm2: f64,
    /// Horizontal grid offset in millimetres.
    pub offset_x_mm: f64,
    /// Vertical grid offset in millimetres.
    pub offset_y_mm: f64,
    /// Whether one die is centered on the wafer origin.
    pub die_at_origin: bool,
}

/// Rectangular probe array dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeArray {
    /// Number of die columns covered by one touchdown.
    pub columns: u32,
    /// Number of die rows covered by one touchdown.
    pub rows: u32,
}

/// Complete user input for one calculation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FabricationInputs {
    /// Die and scribe dimensions.
    pub die: DieGeometry,
    /// Wafer dimensions.
    pub wafer: WaferGeometry,
    /// Process and placement settings.
    pub process: ProcessSettings,
    /// Probe-card array dimensions.
    pub probe: ProbeArray,
}

impl Default for FabricationInputs {
    fn default() -> Self {
        Self {
            die: DieGeometry {
                width_mm: 10.0,
                height_mm: 8.0,
                column_lane_mm: 0.12,
                row_lane_mm: 0.12,
            },
            wafer: WaferGeometry {
                diameter_mm: 300.0,
                edge_exclusion_mm: 3.0,
            },
            process: ProcessSettings {
                defect_density_cm2: 0.1,
                offset_x_mm: 0.0,
                offset_y_mm: 0.0,
                die_at_origin: true,
            },
            probe: ProbeArray {
                columns: 4,
                rows: 4,
            },
        }
    }
}
