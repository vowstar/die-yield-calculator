use serde::{Deserialize, Serialize};
use std::fmt;

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
    /// Random-defect yield model.
    #[serde(default)]
    pub yield_model: YieldModel,
    /// Defect-clustering parameter used by the negative-binomial model.
    #[serde(default = "default_clustering_alpha")]
    pub clustering_alpha: f64,
    /// Horizontal grid offset in millimetres.
    pub offset_x_mm: f64,
    /// Vertical grid offset in millimetres.
    pub offset_y_mm: f64,
    /// Whether one die is centered on the wafer origin.
    pub die_at_origin: bool,
}

const fn default_clustering_alpha() -> f64 {
    1.0
}

/// Statistical model used to estimate random-defect die yield.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YieldModel {
    /// Independent, uniformly distributed random defects.
    Poisson,
    /// Murphy's triangular defect-density distribution.
    #[default]
    MurphyTriangular,
    /// Seeds' clustered-defect approximation.
    Seeds,
    /// Gamma-Poisson mixture controlled by a positive clustering parameter.
    NegativeBinomial,
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
                yield_model: YieldModel::default(),
                clustering_alpha: default_clustering_alpha(),
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

/// User input associated with a validation error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputField {
    /// Die width.
    DieWidth,
    /// Die height.
    DieHeight,
    /// Horizontal scribe lane.
    ColumnLane,
    /// Vertical scribe lane.
    RowLane,
    /// Wafer diameter.
    WaferDiameter,
    /// Edge exclusion.
    EdgeExclusion,
    /// Defect density.
    DefectDensity,
    /// Negative-binomial clustering parameter.
    ClusteringAlpha,
    /// Horizontal grid offset.
    OffsetX,
    /// Vertical grid offset.
    OffsetY,
    /// Probe columns.
    ProbeColumns,
    /// Probe rows.
    ProbeRows,
    /// Candidate-grid size.
    GridDensity,
}

impl InputField {
    /// Stable human-readable field label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::DieWidth => "die width",
            Self::DieHeight => "die height",
            Self::ColumnLane => "column lane",
            Self::RowLane => "row lane",
            Self::WaferDiameter => "wafer diameter",
            Self::EdgeExclusion => "edge exclusion",
            Self::DefectDensity => "defect density",
            Self::ClusteringAlpha => "clustering alpha",
            Self::OffsetX => "horizontal offset",
            Self::OffsetY => "vertical offset",
            Self::ProbeColumns => "probe columns",
            Self::ProbeRows => "probe rows",
            Self::GridDensity => "grid density",
        }
    }
}

/// One validation problem associated with a user input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationError {
    /// Input that failed validation.
    pub field: InputField,
    /// Corrective message suitable for the interface.
    pub message: &'static str,
}

/// Collection of validation problems returned by an analysis request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationErrors {
    pub(crate) errors: Vec<ValidationError>,
}

impl ValidationErrors {
    /// Returns the individual validation errors.
    #[must_use]
    pub fn as_slice(&self) -> &[ValidationError] {
        &self.errors
    }

    /// Returns whether no validation problems are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} invalid input(s)", self.errors.len())
    }
}

impl std::error::Error for ValidationErrors {}

/// Geometric relationship between a die candidate and the usable wafer area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DieClass {
    /// The complete die rectangle is within the usable wafer radius.
    Usable,
    /// The die intersects the usable-radius boundary.
    Partial,
    /// The die intersects the wafer but lies wholly in the exclusion band.
    EdgeExclusion,
}

/// One die-sized rectangle intersecting the wafer.
#[derive(Clone, Debug, PartialEq)]
pub struct DiePlacement {
    /// Stable column index in the infinite placement grid.
    pub column: i32,
    /// Stable row index in the infinite placement grid.
    pub row: i32,
    /// Rectangle centre relative to the wafer centre in millimetres.
    pub center_mm: [f64; 2],
    /// Active die size in millimetres.
    pub size_mm: [f64; 2],
    /// Geometric classification.
    pub class: DieClass,
    /// Whether this usable die was selected as an expected defect marker.
    pub defective: bool,
}

/// Aggregate geometric and statistical die counts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct YieldSummary {
    /// Selected-model yield in the inclusive range zero to one.
    pub yield_fraction: f64,
    /// Active die area used by the random-defect model in square millimetres.
    pub yield_area_mm2: f64,
    /// Dimensionless defect exposure, active area in cm² multiplied by D0.
    pub defect_exposure: f64,
    /// Complete dies inside the usable radius before statistical defects.
    pub geometric_usable: u64,
    /// Unrounded expected good-die count.
    pub expected_good_exact: f64,
    /// Unrounded expected defective-die count.
    pub expected_defective_exact: f64,
    /// Expected good dies after statistical defects.
    pub expected_good: u64,
    /// Expected defective dies.
    pub expected_defective: u64,
    /// Dies intersecting the usable-radius boundary.
    pub partial: u64,
    /// Candidates wholly in the edge-exclusion band.
    pub edge_excluded: u64,
}

/// Probe-array coverage summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeSummary {
    /// Maximum number of sites in one rectangular touchdown.
    pub sites_per_touchdown: u64,
    /// Number of occupied probe-array blocks covering all usable sites.
    pub touchdown_count: u64,
}

/// Complete normalized result of a wafer analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct WaferAnalysis {
    /// Inputs after grid-offset normalization.
    pub normalized_inputs: FabricationInputs,
    /// Candidate die rectangles that intersect the wafer.
    pub placements: Vec<DiePlacement>,
    /// Yield and geometry totals.
    pub summary: YieldSummary,
    /// Probe-array coverage totals.
    pub probe: ProbeSummary,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ValidatedInputs {
    pub inputs: FabricationInputs,
    pub pitch_x_mm: f64,
    pub pitch_y_mm: f64,
    pub wafer_radius_mm: f64,
    pub usable_radius_mm: f64,
}
