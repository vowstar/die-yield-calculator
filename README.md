# Yield Studio

Rust wafer die-yield and probe-planning calculator. Native and WebAssembly GUI.

**[Open Yield Studio in your browser](https://vowstar.github.io/die-yield-calculator/)**

![Yield Studio showing Gross Dies per Wafer, estimated yield, expected good dies, responsive calculation setup, wafer map, and an exported analysis report](https://github.com/user-attachments/assets/b87dd002-635a-4f42-aa95-a011917ea916)

## Features

- Gross Dies / Wafer as the primary geometric result, separate from statistical yield
- Wafer map with edge exclusion, die placement, and visible scribe lanes
- Selectable Poisson, Murphy triangular, Seeds, and negative-binomial yield models
- Progressive setup with essential inputs first and manufacturing, alignment, formula,
  unit, exposure, and unrounded-expectation details available on demand
- Standard wafer sizes from 76 mm (3 in) to 450 mm (18 in)
- Die geometry, grid alignment, and probe-array controls
- Responsive native and browser interface
- PNG, SVG, A4 PDF, and reproducible JSON export with printing

## Building

Requires Rust 1.95 or newer.

**Native:**

```sh
cargo build --release --workspace
cargo run --release -p die-yield-gui
```

**WebAssembly:**

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --version 0.21.14 --locked
trunk serve
```

Build the static site for deployment:

```sh
trunk build --release --public-url ./
```

The output is written to `dist/` and requires no backend.

## Reports

Open **Report** to export the current wafer map, results, parameters, and legend
as PNG, SVG, or A4 PDF, or save normalized inputs and exact results as JSON.
Browser builds download the report directly and use the browser print dialog.
Native builds use the system save dialog and PDF viewer.

The yield models treat D₀ as an effective full-process density of random fatal
defects, measured in defects/cm². Per-mask or baseline D₀ values that require a
separate process-complexity factor are outside the supported input semantics.

## Calibration validation

An offline workflow validates anonymous Gross Die datasets against the geometric
baseline using grouped leave-one-project-out comparisons and explicit error gates.
No fitted coefficient is bundled with the application. See the
[calibration data contract](docs/gross-die-calibration-data.md) for required units,
definitions, provenance controls, and release criteria.

## Testing

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo check --workspace --target wasm32-unknown-unknown
python3 -m unittest discover -s scripts -p 'test_*.py' -v
python3 .agents/skills/run-multi-user-acceptance/scripts/evaluate_acceptance.py --self-test
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

Embedded font licenses and notices are preserved in [LICENSES](LICENSES/).
