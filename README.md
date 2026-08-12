# Yield Studio

Rust wafer die-yield and probe-planning calculator. Native and WebAssembly GUI.

## Features

- Wafer map with edge exclusion, die placement, and visible scribe lanes
- Murphy yield model with deterministic defect visualization
- Standard wafer sizes from 76 mm (3 in) to 450 mm (18 in)
- Die geometry, grid alignment, and probe-array controls
- Responsive native and browser interface
- PNG, SVG, and A4 PDF report export with printing

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
as PNG, SVG, or A4 PDF. Browser builds download the report directly and use the
browser print dialog. Native builds use the system save dialog and PDF viewer.

## Testing

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo check --workspace --target wasm32-unknown-unknown
```

## GitHub Pages

The included workflow builds and deploys the WebAssembly site on pushes to
`main`. Select **GitHub Actions** as the publishing source under
**Settings → Pages** before the first deployment.

## Project Structure

```text
crates/
  die-yield-core/    -- Input model, validation, and yield analysis
  die-yield-render/  -- Wafer scene and egui renderer
  die-yield-gui/     -- Native and WebAssembly application
web/                 -- Browser entry point and static assets
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

Embedded font licenses and notices are preserved in [LICENSES](LICENSES/).
