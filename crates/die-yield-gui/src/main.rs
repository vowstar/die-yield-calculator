#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();

    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_title("Die Yield Calculator")
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([760.0, 560.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Die Yield Calculator",
        native_options,
        Box::new(|context| Ok(Box::new(die_yield_gui::YieldWorkbench::new(context)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    eframe::WebLogger::init(log::LevelFilter::Info).ok();

    wasm_bindgen_futures::spawn_local(async {
        let window = web_sys::window().expect("browser window is unavailable");
        let document = window.document().expect("browser document is unavailable");
        let canvas = document
            .get_element_by_id("yield_canvas")
            .expect("yield_canvas element is missing")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("yield_canvas is not a canvas element");

        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|context| Ok(Box::new(die_yield_gui::YieldWorkbench::new(context)))),
            )
            .await;

        if let Some(status) = document.get_element_by_id("startup_status") {
            match result {
                Ok(()) => status.remove(),
                Err(error) => {
                    status.set_text_content(Some("Unable to start the application."));
                    panic!("failed to start web application: {error:?}");
                }
            }
        }
    });
}
