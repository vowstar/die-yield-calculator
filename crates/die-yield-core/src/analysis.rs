use crate::{
    DieClass, DiePlacement, FabricationInputs, ProbeSummary, ValidationErrors, WaferAnalysis,
    YieldModel, YieldSummary, model::ValidatedInputs, validation::validate,
};
use std::{collections::BTreeSet, ops::RangeInclusive};

/// Validates the inputs and performs geometric, statistical, and probe analysis.
pub fn analyze(inputs: &FabricationInputs) -> Result<WaferAnalysis, ValidationErrors> {
    let validated = validate(inputs)?;
    let mut placements = build_grid(&validated);
    let geometric_usable = count_class(&placements, DieClass::Usable);
    let yield_area_mm2 = validated.inputs.die.width_mm * validated.inputs.die.height_mm;
    let defect_exposure = yield_area_mm2 / 100.0 * validated.inputs.process.defect_density_cm2;
    let yield_fraction = calculate_yield(
        validated.inputs.process.yield_model,
        yield_area_mm2,
        validated.inputs.process.defect_density_cm2,
        validated.inputs.process.clustering_alpha,
    );
    let expected_good_exact = geometric_usable as f64 * yield_fraction;
    let expected_defective_exact = geometric_usable as f64 * (1.0 - yield_fraction);
    let expected_good = expected_good_exact
        .round()
        .clamp(0.0, geometric_usable as f64) as u64;
    let expected_defective = geometric_usable - expected_good;

    mark_expected_defects(
        &mut placements,
        expected_defective as usize,
        analysis_seed(&validated),
    );

    let probe = summarize_probe(&validated, &placements);
    let summary = YieldSummary {
        yield_fraction,
        yield_area_mm2,
        defect_exposure,
        geometric_usable,
        expected_good_exact,
        expected_defective_exact,
        expected_good,
        expected_defective,
        partial: count_class(&placements, DieClass::Partial),
        edge_excluded: count_class(&placements, DieClass::EdgeExclusion),
    };

    Ok(WaferAnalysis {
        normalized_inputs: validated.inputs,
        placements,
        summary,
        probe,
    })
}

/// Calculates random-defect die yield for area in mm² and density in defects/cm².
///
/// `clustering_alpha` is used only by [`YieldModel::NegativeBinomial`] and must be
/// positive and finite for that model. Invalid negative-binomial alpha values
/// return `NaN`; [`analyze`] rejects them during input validation.
#[must_use]
pub fn calculate_yield(
    model: YieldModel,
    die_area_mm2: f64,
    defect_density_cm2: f64,
    clustering_alpha: f64,
) -> f64 {
    let exposure = die_area_mm2 / 100.0 * defect_density_cm2;
    let yield_fraction = match model {
        YieldModel::Poisson => (-exposure).exp(),
        YieldModel::MurphyTriangular => murphy_from_exposure(exposure),
        YieldModel::Seeds => 1.0 / (1.0 + exposure),
        YieldModel::NegativeBinomial => {
            if !clustering_alpha.is_finite() || clustering_alpha <= 0.0 {
                return f64::NAN;
            }
            if exposure == 0.0 {
                return 1.0;
            }

            let ratio = exposure / clustering_alpha;
            let log_base = if ratio.is_infinite() {
                exposure.ln() - clustering_alpha.ln()
            } else {
                ratio.ln_1p()
            };
            (-clustering_alpha * log_base).exp()
        }
    };

    yield_fraction.clamp(0.0, 1.0)
}

/// Returns Murphy-model yield for die area in mm² and density in defects/cm².
#[must_use]
pub fn murphy_yield(die_area_mm2: f64, defect_density_cm2: f64) -> f64 {
    calculate_yield(
        YieldModel::MurphyTriangular,
        die_area_mm2,
        defect_density_cm2,
        1.0,
    )
}

fn murphy_from_exposure(exposure: f64) -> f64 {
    if exposure.abs() < 1.0e-12 {
        return 1.0;
    }

    (-(-exposure).exp_m1() / exposure).powi(2)
}

fn build_grid(validated: &ValidatedInputs) -> Vec<DiePlacement> {
    let inputs = validated.inputs;
    let half_width = inputs.die.width_mm / 2.0;
    let half_height = inputs.die.height_mm / 2.0;
    let origin_x = grid_origin(
        inputs.process.die_at_origin,
        validated.pitch_x_mm,
        inputs.process.offset_x_mm,
    );
    let origin_y = grid_origin(
        inputs.process.die_at_origin,
        validated.pitch_y_mm,
        inputs.process.offset_y_mm,
    );
    let columns = index_span(
        validated.wafer_radius_mm,
        half_width,
        origin_x,
        validated.pitch_x_mm,
    );
    let rows = index_span(
        validated.wafer_radius_mm,
        half_height,
        origin_y,
        validated.pitch_y_mm,
    );
    let mut placements = Vec::new();

    for row in rows {
        let center_y = origin_y + f64::from(row) * validated.pitch_y_mm;
        for column in columns.clone() {
            let center_x = origin_x + f64::from(column) * validated.pitch_x_mm;
            if let Some(class) = classify_rectangle(
                center_x,
                center_y,
                half_width,
                half_height,
                validated.wafer_radius_mm,
                validated.usable_radius_mm,
            ) {
                placements.push(DiePlacement {
                    column,
                    row,
                    center_mm: [center_x, center_y],
                    size_mm: [inputs.die.width_mm, inputs.die.height_mm],
                    class,
                    defective: false,
                });
            }
        }
    }

    placements
}

fn grid_origin(die_at_origin: bool, pitch: f64, offset: f64) -> f64 {
    if die_at_origin {
        offset
    } else {
        pitch / 2.0 + offset
    }
}

fn index_span(radius: f64, half_size: f64, origin: f64, pitch: f64) -> RangeInclusive<i32> {
    let first = ((-radius - half_size - origin) / pitch).floor() as i32;
    let last = ((radius + half_size - origin) / pitch).ceil() as i32;
    first..=last
}

fn classify_rectangle(
    center_x: f64,
    center_y: f64,
    half_width: f64,
    half_height: f64,
    wafer_radius: f64,
    usable_radius: f64,
) -> Option<DieClass> {
    let nearest_x = (center_x.abs() - half_width).max(0.0);
    let nearest_y = (center_y.abs() - half_height).max(0.0);
    let nearest_distance_sq = nearest_x.mul_add(nearest_x, nearest_y * nearest_y);
    let farthest_x = center_x.abs() + half_width;
    let farthest_y = center_y.abs() + half_height;
    let farthest_distance_sq = farthest_x.mul_add(farthest_x, farthest_y * farthest_y);
    let wafer_radius_sq = wafer_radius * wafer_radius;
    let usable_radius_sq = usable_radius * usable_radius;
    let tolerance = wafer_radius_sq.max(1.0) * 1.0e-12;

    if nearest_distance_sq > wafer_radius_sq + tolerance {
        None
    } else if farthest_distance_sq <= usable_radius_sq + tolerance {
        Some(DieClass::Usable)
    } else if nearest_distance_sq <= usable_radius_sq + tolerance {
        Some(DieClass::Partial)
    } else {
        Some(DieClass::EdgeExclusion)
    }
}

fn count_class(placements: &[DiePlacement], class: DieClass) -> u64 {
    placements
        .iter()
        .filter(|placement| placement.class == class)
        .count() as u64
}

fn mark_expected_defects(placements: &mut [DiePlacement], defect_count: usize, seed: u64) {
    let mut usable_indices: Vec<usize> = placements
        .iter()
        .enumerate()
        .filter_map(|(index, placement)| (placement.class == DieClass::Usable).then_some(index))
        .collect();
    let selection_count = defect_count.min(usable_indices.len());
    let mut random = SplitMix64::new(seed);

    for selected in 0..selection_count {
        let remaining = usable_indices.len() - selected;
        let swap_index = selected + random.index(remaining);
        usable_indices.swap(selected, swap_index);
        placements[usable_indices[selected]].defective = true;
    }
}

fn summarize_probe(validated: &ValidatedInputs, placements: &[DiePlacement]) -> ProbeSummary {
    let usable: Vec<&DiePlacement> = placements
        .iter()
        .filter(|placement| placement.class == DieClass::Usable)
        .collect();
    let sites_per_touchdown =
        u64::from(validated.inputs.probe.columns) * u64::from(validated.inputs.probe.rows);

    let (Some(min_column), Some(min_row)) = (
        usable.iter().map(|placement| placement.column).min(),
        usable.iter().map(|placement| placement.row).min(),
    ) else {
        return ProbeSummary {
            sites_per_touchdown,
            touchdown_count: 0,
        };
    };

    let probe_columns = validated.inputs.probe.columns as i32;
    let probe_rows = validated.inputs.probe.rows as i32;
    let occupied_blocks: BTreeSet<(i32, i32)> = usable
        .iter()
        .map(|placement| {
            (
                (placement.column - min_column).div_euclid(probe_columns),
                (placement.row - min_row).div_euclid(probe_rows),
            )
        })
        .collect();

    ProbeSummary {
        sites_per_touchdown,
        touchdown_count: occupied_blocks.len() as u64,
    }
}

fn analysis_seed(validated: &ValidatedInputs) -> u64 {
    let inputs = validated.inputs;
    let values = [
        inputs.die.width_mm.to_bits(),
        inputs.die.height_mm.to_bits(),
        inputs.die.column_lane_mm.to_bits(),
        inputs.die.row_lane_mm.to_bits(),
        inputs.wafer.diameter_mm.to_bits(),
        inputs.wafer.edge_exclusion_mm.to_bits(),
        inputs.process.defect_density_cm2.to_bits(),
        inputs.process.offset_x_mm.to_bits(),
        inputs.process.offset_y_mm.to_bits(),
        u64::from(inputs.process.die_at_origin),
        u64::from(inputs.probe.columns),
        u64::from(inputs.probe.rows),
    ];

    values.into_iter().fold(0x6a09_e667_f3bc_c909, mix)
}

fn mix(state: u64, value: u64) -> u64 {
    let mut mixed = state ^ value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix(0, self.state)
    }

    fn index(&mut self, upper_bound: usize) -> usize {
        (self.next() % upper_bound as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InputField;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected:.16}, got {actual:.16}"
        );
    }

    #[test]
    fn default_analysis_is_self_consistent() {
        let analysis = analyze(&FabricationInputs::default()).expect("defaults should be valid");

        assert_eq!(analysis.summary.geometric_usable, 767);
        assert_eq!(analysis.summary.expected_good, 708);
        assert_eq!(analysis.summary.expected_defective, 59);
        assert_eq!(analysis.summary.partial, 124);
        assert_eq!(analysis.summary.edge_excluded, 40);
        assert_close(
            analysis.summary.yield_fraction,
            0.923_608_780_146_839_3,
            1e-12,
        );
        assert_eq!(analysis.summary.yield_area_mm2, 80.0);
        assert_close(analysis.summary.defect_exposure, 0.08, f64::EPSILON);
        assert_close(
            analysis.summary.expected_good_exact,
            708.407_934_372_625_7,
            1e-12,
        );
        assert_close(
            analysis.summary.expected_defective_exact,
            58.592_065_627_374_225,
            1e-12,
        );
        assert_eq!(
            analysis.summary.expected_good + analysis.summary.expected_defective,
            analysis.summary.geometric_usable
        );
        assert!(analysis.probe.touchdown_count > 0);
        assert!(analysis.probe.touchdown_count <= analysis.summary.geometric_usable);
    }

    #[test]
    fn zero_density_preserves_every_usable_die() {
        let mut inputs = FabricationInputs::default();
        inputs.process.defect_density_cm2 = 0.0;
        let analysis = analyze(&inputs).expect("zero density should be valid");

        assert_eq!(analysis.summary.yield_fraction, 1.0);
        assert_eq!(analysis.summary.expected_defective, 0);
        assert_eq!(
            analysis.summary.expected_good,
            analysis.summary.geometric_usable
        );
        assert!(
            analysis
                .placements
                .iter()
                .all(|placement| !placement.defective)
        );
    }

    #[test]
    fn displayed_good_count_rounds_the_good_expectation_at_a_half_tie() {
        let mut inputs = FabricationInputs::default();
        inputs.wafer.diameter_mm = 25.0;
        inputs.wafer.edge_exclusion_mm = 0.0;
        inputs.die.width_mm = 10.0;
        inputs.die.height_mm = 10.0;
        inputs.die.column_lane_mm = 0.1;
        inputs.die.row_lane_mm = 0.1;
        inputs.process.yield_model = YieldModel::Poisson;
        inputs.process.defect_density_cm2 = std::f64::consts::LN_2;

        let analysis = analyze(&inputs).expect("half-yield fixture should be valid");

        assert_eq!(analysis.summary.geometric_usable, 1);
        assert_close(analysis.summary.expected_good_exact, 0.5, 1.0e-15);
        assert_eq!(analysis.summary.expected_good, 1);
        assert_eq!(analysis.summary.expected_defective, 0);
    }

    #[test]
    fn defective_markers_are_deterministic() {
        let inputs = FabricationInputs::default();
        let first = analyze(&inputs).expect("defaults should be valid");
        let second = analyze(&inputs).expect("defaults should be valid");

        assert_eq!(first, second);
        assert_eq!(
            first
                .placements
                .iter()
                .filter(|placement| placement.defective)
                .count() as u64,
            first.summary.expected_defective
        );
    }

    #[test]
    fn whole_pitch_offsets_produce_equivalent_analysis() {
        let inputs = FabricationInputs::default();
        let mut shifted = inputs;
        shifted.process.offset_x_mm += inputs.die.width_mm + inputs.die.column_lane_mm;
        shifted.process.offset_y_mm -= inputs.die.height_mm + inputs.die.row_lane_mm;

        assert_eq!(
            analyze(&inputs).expect("defaults should be valid"),
            analyze(&shifted).expect("shifted inputs should be valid")
        );
    }

    #[test]
    fn invalid_values_report_all_relevant_fields() {
        let mut inputs = FabricationInputs::default();
        inputs.die.width_mm = f64::NAN;
        inputs.wafer.diameter_mm = 1.0;
        inputs.process.defect_density_cm2 = -1.0;
        inputs.probe.rows = 0;

        let errors = analyze(&inputs).expect_err("invalid inputs should be rejected");
        let fields: Vec<InputField> = errors.as_slice().iter().map(|error| error.field).collect();

        assert!(fields.contains(&InputField::DieWidth));
        assert!(fields.contains(&InputField::WaferDiameter));
        assert!(fields.contains(&InputField::DefectDensity));
        assert!(fields.contains(&InputField::ProbeRows));
    }

    #[test]
    fn murphy_model_is_stable_at_zero_and_decreases_with_exposure() {
        assert_eq!(murphy_yield(80.0, 0.0), 1.0);
        let low = murphy_yield(80.0, 0.01);
        let high = murphy_yield(80.0, 1.0);
        assert!((0.0..1.0).contains(&low));
        assert!(high < low);
    }

    #[test]
    fn yield_models_match_independent_unit_exposure_oracles() {
        // 100 mm² = 1 cm², so D0 = 1 gives the dimensionless exposure AD0 = 1.
        let cases = [
            (YieldModel::Poisson, 1.0, 0.367_879_441_171_442_33),
            (YieldModel::MurphyTriangular, 1.0, 0.399_576_400_893_728_03),
            (YieldModel::Seeds, 1.0, 0.5),
            (YieldModel::NegativeBinomial, 2.0, 0.444_444_444_444_444_4),
        ];

        for (model, alpha, expected) in cases {
            assert_close(calculate_yield(model, 100.0, 1.0, alpha), expected, 1e-15);
        }
    }

    #[test]
    fn selected_model_flows_into_summary_without_changing_geometry() {
        let default = analyze(&FabricationInputs::default()).expect("defaults should be valid");
        let mut poisson_inputs = FabricationInputs::default();
        poisson_inputs.process.yield_model = YieldModel::Poisson;
        poisson_inputs.process.clustering_alpha = f64::NAN;
        let poisson = analyze(&poisson_inputs).expect("Poisson does not require alpha");

        assert_eq!(poisson.summary.geometric_usable, 767);
        assert_eq!(poisson.summary.yield_area_mm2, 80.0);
        assert_close(poisson.summary.defect_exposure, 0.08, f64::EPSILON);
        assert_close(
            poisson.summary.yield_fraction,
            0.923_116_346_386_635_8,
            1e-15,
        );
        assert!(poisson.summary.yield_fraction < default.summary.yield_fraction);
    }

    #[test]
    fn all_yield_models_have_the_correct_zero_and_high_exposure_limits() {
        for model in [
            YieldModel::Poisson,
            YieldModel::MurphyTriangular,
            YieldModel::Seeds,
            YieldModel::NegativeBinomial,
        ] {
            assert_eq!(calculate_yield(model, 80.0, 0.0, 2.0), 1.0);
            let high = calculate_yield(model, 100.0, 100.0, 2.0);
            assert!(high.is_finite());
            assert!((0.0..=1.0).contains(&high));
        }
    }

    #[test]
    fn all_yield_models_are_stable_near_zero_and_monotonic() {
        for model in [
            YieldModel::Poisson,
            YieldModel::MurphyTriangular,
            YieldModel::Seeds,
            YieldModel::NegativeBinomial,
        ] {
            let near_zero = calculate_yield(model, 1.0e-6, 1.0e-6, 2.0);
            assert!(near_zero.is_finite());
            assert!((1.0 - 1.0e-12..=1.0).contains(&near_zero));

            let mut previous = 1.0;
            for density in [0.0, 0.01, 0.1, 1.0, 10.0, 100.0] {
                let current = calculate_yield(model, 100.0, density, 2.0);
                assert!(current <= previous, "{model:?} at D0={density}");
                previous = current;
            }
        }
    }

    #[test]
    fn negative_binomial_contains_seeds_and_converges_to_poisson() {
        let area_mm2 = 80.0;
        let density_cm2 = 1.0;
        let seeds = calculate_yield(YieldModel::Seeds, area_mm2, density_cm2, f64::NAN);
        let alpha_one = calculate_yield(YieldModel::NegativeBinomial, area_mm2, density_cm2, 1.0);
        let poisson = calculate_yield(YieldModel::Poisson, area_mm2, density_cm2, f64::NAN);
        let large_alpha =
            calculate_yield(YieldModel::NegativeBinomial, area_mm2, density_cm2, 1.0e12);

        assert_close(alpha_one, seeds, f64::EPSILON);
        assert_close(large_alpha, poisson, 2.0e-13);
        assert!(
            calculate_yield(YieldModel::NegativeBinomial, area_mm2, density_cm2, 0.0,).is_nan()
        );
    }

    #[test]
    fn murphy_compatibility_wrapper_uses_the_general_model() {
        for (area, density) in [(0.25, 0.0), (22.64, 0.1), (31.63, 0.2), (80.0, 5.0)] {
            assert_eq!(
                murphy_yield(area, density),
                calculate_yield(YieldModel::MurphyTriangular, area, density, f64::NAN)
            );
        }
    }

    #[test]
    fn standard_wafer_sizes_produce_consistent_geometry() {
        for diameter_mm in [50.0, 75.0, 100.0, 150.0, 200.0, 300.0, 450.0] {
            let mut inputs = FabricationInputs::default();
            inputs.wafer.diameter_mm = diameter_mm;
            inputs.wafer.edge_exclusion_mm = (diameter_mm * 0.02).max(1.0);
            inputs.die.width_mm = 4.0;
            inputs.die.height_mm = 3.0;

            let analysis = analyze(&inputs).expect("standard wafer size should be valid");

            assert!(analysis.summary.geometric_usable > 0, "{diameter_mm} mm");
            assert_eq!(
                analysis.summary.expected_good + analysis.summary.expected_defective,
                analysis.summary.geometric_usable,
                "{diameter_mm} mm"
            );
            assert_eq!(
                analysis.summary.geometric_usable,
                count_class(&analysis.placements, DieClass::Usable),
                "{diameter_mm} mm"
            );
        }
    }

    #[test]
    fn defect_density_matrix_is_monotonic() {
        let mut previous_yield = 1.0;
        let mut previous_defective = 0;

        for density in [0.0, 0.01, 0.1, 0.5, 1.0, 5.0] {
            let mut inputs = FabricationInputs::default();
            inputs.process.defect_density_cm2 = density;
            let analysis = analyze(&inputs).expect("density should be valid");

            assert!(analysis.summary.yield_fraction <= previous_yield);
            assert!(analysis.summary.expected_defective >= previous_defective);
            previous_yield = analysis.summary.yield_fraction;
            previous_defective = analysis.summary.expected_defective;
        }
    }

    #[test]
    fn geometry_and_probe_parameter_matrix_preserves_invariants() {
        let mut wide_die = FabricationInputs::default();
        wide_die.die.width_mm = 18.0;
        wide_die.die.height_mm = 4.5;
        wide_die.die.column_lane_mm = 0.2;
        wide_die.die.row_lane_mm = 0.08;
        wide_die.probe.columns = 8;
        wide_die.probe.rows = 2;

        let mut shifted_grid = FabricationInputs::default();
        shifted_grid.process.die_at_origin = false;
        shifted_grid.process.offset_x_mm = 1.7;
        shifted_grid.process.offset_y_mm = -2.3;
        shifted_grid.wafer.edge_exclusion_mm = 8.0;

        let mut square_array = FabricationInputs::default();
        square_array.wafer.diameter_mm = 200.0;
        square_array.die.width_mm = 6.5;
        square_array.die.height_mm = 6.5;
        square_array.probe.columns = 1;
        square_array.probe.rows = 1;

        for inputs in [wide_die, shifted_grid, square_array] {
            let analysis = analyze(&inputs).expect("parameter set should be valid");
            let classified = analysis.summary.geometric_usable
                + analysis.summary.partial
                + analysis.summary.edge_excluded;

            assert_eq!(classified, analysis.placements.len() as u64);
            assert_eq!(
                analysis.summary.expected_good + analysis.summary.expected_defective,
                analysis.summary.geometric_usable
            );
            assert!(analysis.probe.touchdown_count <= analysis.summary.geometric_usable);
        }
    }

    #[test]
    fn rectangle_classification_uses_circle_intersection() {
        assert_eq!(
            classify_rectangle(0.0, 0.0, 1.0, 1.0, 10.0, 8.0),
            Some(DieClass::Usable)
        );
        assert_eq!(
            classify_rectangle(7.5, 0.0, 1.0, 1.0, 10.0, 8.0),
            Some(DieClass::Partial)
        );
        assert_eq!(
            classify_rectangle(9.5, 0.0, 0.2, 0.2, 10.0, 8.0),
            Some(DieClass::EdgeExclusion)
        );
        assert_eq!(classify_rectangle(11.0, 0.0, 0.2, 0.2, 10.0, 8.0), None);
    }

    #[test]
    fn rectangle_classification_includes_exact_circle_tangencies() {
        assert_eq!(
            classify_rectangle(0.0, 0.0, 6.0, 8.0, 12.0, 10.0),
            Some(DieClass::Usable),
            "a farthest corner exactly on the usable circle is included"
        );
        assert_eq!(
            classify_rectangle(9.0, 0.0, 1.0, 1.0, 12.0, 8.0),
            Some(DieClass::Partial),
            "a rectangle tangent to the usable circle intersects its boundary"
        );
        assert_eq!(
            classify_rectangle(10.25, 0.0, 0.25, 0.25, 10.0, 8.0),
            Some(DieClass::EdgeExclusion),
            "a rectangle tangent to the outer circle remains mapped"
        );
    }
}
