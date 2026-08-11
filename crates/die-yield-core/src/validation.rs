use crate::{
    FabricationInputs, InputField, ValidationError, ValidationErrors, model::ValidatedInputs,
};

const MIN_DIE_MM: f64 = 0.25;
const MAX_WAFER_MM: f64 = 450.0;
const MAX_LANE_MM: f64 = 10.0;
const MAX_DEFECT_DENSITY: f64 = 100.0;
const MAX_PROBE_SPAN: u32 = 128;
const MAX_CANDIDATES: u64 = 1_000_000;

pub(crate) fn validate(inputs: &FabricationInputs) -> Result<ValidatedInputs, ValidationErrors> {
    let mut errors = Vec::new();

    check_range(
        &mut errors,
        InputField::DieWidth,
        inputs.die.width_mm,
        MIN_DIE_MM,
        inputs.wafer.diameter_mm,
        "must be between 0.25 mm and the wafer diameter",
    );
    check_range(
        &mut errors,
        InputField::DieHeight,
        inputs.die.height_mm,
        MIN_DIE_MM,
        inputs.wafer.diameter_mm,
        "must be between 0.25 mm and the wafer diameter",
    );
    check_range(
        &mut errors,
        InputField::ColumnLane,
        inputs.die.column_lane_mm,
        0.0,
        MAX_LANE_MM,
        "must be between 0 mm and 10 mm",
    );
    check_range(
        &mut errors,
        InputField::RowLane,
        inputs.die.row_lane_mm,
        0.0,
        MAX_LANE_MM,
        "must be between 0 mm and 10 mm",
    );
    check_range(
        &mut errors,
        InputField::WaferDiameter,
        inputs.wafer.diameter_mm,
        25.0,
        MAX_WAFER_MM,
        "must be between 25 mm and 450 mm",
    );
    check_edge_exclusion(&mut errors, inputs);
    check_range(
        &mut errors,
        InputField::DefectDensity,
        inputs.process.defect_density_cm2,
        0.0,
        MAX_DEFECT_DENSITY,
        "must be between 0 and 100 defects/cm²",
    );
    check_finite(&mut errors, InputField::OffsetX, inputs.process.offset_x_mm);
    check_finite(&mut errors, InputField::OffsetY, inputs.process.offset_y_mm);
    check_integer_range(&mut errors, InputField::ProbeColumns, inputs.probe.columns);
    check_integer_range(&mut errors, InputField::ProbeRows, inputs.probe.rows);

    if !errors.is_empty() {
        return Err(ValidationErrors { errors });
    }

    let pitch_x_mm = inputs.die.width_mm + inputs.die.column_lane_mm;
    let pitch_y_mm = inputs.die.height_mm + inputs.die.row_lane_mm;
    let estimated_columns =
        ((inputs.wafer.diameter_mm + inputs.die.width_mm) / pitch_x_mm).ceil() as u64 + 3;
    let estimated_rows =
        ((inputs.wafer.diameter_mm + inputs.die.height_mm) / pitch_y_mm).ceil() as u64 + 3;

    if estimated_columns.saturating_mul(estimated_rows) > MAX_CANDIDATES {
        return Err(ValidationErrors {
            errors: vec![ValidationError {
                field: InputField::GridDensity,
                message: "produces more than one million candidate sites",
            }],
        });
    }

    let mut normalized = *inputs;
    normalized.process.offset_x_mm = wrap_phase(inputs.process.offset_x_mm, pitch_x_mm);
    normalized.process.offset_y_mm = wrap_phase(inputs.process.offset_y_mm, pitch_y_mm);

    Ok(ValidatedInputs {
        inputs: normalized,
        pitch_x_mm,
        pitch_y_mm,
        wafer_radius_mm: normalized.wafer.diameter_mm / 2.0,
        usable_radius_mm: normalized.wafer.diameter_mm / 2.0 - normalized.wafer.edge_exclusion_mm,
    })
}

fn check_range(
    errors: &mut Vec<ValidationError>,
    field: InputField,
    value: f64,
    minimum: f64,
    maximum: f64,
    message: &'static str,
) {
    if !value.is_finite() || value < minimum || value > maximum {
        errors.push(ValidationError { field, message });
    }
}

fn check_edge_exclusion(errors: &mut Vec<ValidationError>, inputs: &FabricationInputs) {
    let value = inputs.wafer.edge_exclusion_mm;
    if !value.is_finite() || value < 0.0 || value >= inputs.wafer.diameter_mm / 2.0 {
        errors.push(ValidationError {
            field: InputField::EdgeExclusion,
            message: "must be non-negative and smaller than the wafer radius",
        });
    }
}

fn check_finite(errors: &mut Vec<ValidationError>, field: InputField, value: f64) {
    if !value.is_finite() {
        errors.push(ValidationError {
            field,
            message: "must be a finite number",
        });
    }
}

fn check_integer_range(errors: &mut Vec<ValidationError>, field: InputField, value: u32) {
    if !(1..=MAX_PROBE_SPAN).contains(&value) {
        errors.push(ValidationError {
            field,
            message: "must be between 1 and 128",
        });
    }
}

fn wrap_phase(value: f64, pitch: f64) -> f64 {
    let wrapped = (value + pitch / 2.0).rem_euclid(pitch) - pitch / 2.0;
    if wrapped.abs() <= pitch * 1.0e-12 {
        0.0
    } else {
        wrapped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equivalent_offsets_normalize_to_the_same_phase() {
        let inputs = FabricationInputs::default();
        let pitch = inputs.die.width_mm + inputs.die.column_lane_mm;
        let base = validate(&inputs).expect("default inputs should be valid");

        let mut shifted = inputs;
        shifted.process.offset_x_mm += 3.0 * pitch;
        let shifted = validate(&shifted).expect("shifted inputs should be valid");

        assert!(
            (base.inputs.process.offset_x_mm - shifted.inputs.process.offset_x_mm).abs() < 1e-9
        );
    }

    #[test]
    fn dense_grid_is_rejected_before_allocation() {
        let mut inputs = FabricationInputs::default();
        inputs.wafer.diameter_mm = 450.0;
        inputs.die.width_mm = 0.25;
        inputs.die.height_mm = 0.25;
        inputs.die.column_lane_mm = 0.0;
        inputs.die.row_lane_mm = 0.0;

        let errors = validate(&inputs).expect_err("grid should exceed the safety limit");
        assert_eq!(errors.as_slice()[0].field, InputField::GridDensity);
    }
}
