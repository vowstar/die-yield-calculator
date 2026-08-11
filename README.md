# Yield Studio

Yield Studio is a responsive wafer die-yield and probe-planning calculator built with Rust and egui. The same workspace runs as a native desktop application and as a static WebAssembly site.

## Run locally

```sh
cargo run --release -p die-yield-gui
```

## Run in a browser

Install the `wasm32-unknown-unknown` Rust target and stable Trunk 0.21.14, then run:

```sh
trunk serve
```

For a deployable static bundle:

```sh
trunk build --release --public-url ./
```

The result is written to `dist/`. No server-side runtime or database is required.

## Test

```sh
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --target wasm32-unknown-unknown
```

Tests cover standard wafer diameters, a defect-density range, die and probe configurations, invalid inputs, deterministic results, renderer invariants, and responsive layouts from 480 to 1440 points.

## GitHub Pages

The included workflow builds and deploys the static site on every push to `main`. In the repository, enable GitHub Actions and select **GitHub Actions** as the Pages source under **Settings → Pages**. GitHub Free supports Pages for public repositories; private-repository availability depends on the account plan.

GitHub Pages has no fixed site expiration. A deployed site remains available while the repository, account, plan, and Pages configuration remain active. Workflow artifact retention is separate from the deployed site.

## License

Project code is available under either the MIT License or Apache License 2.0. egui embeds Ubuntu Light and fallback fonts; their open-source license texts are preserved in [`LICENSES`](LICENSES/).

Results use a Murphy yield estimate and are intended for planning, not production sign-off.
