use die_yield_core::FabricationInputs;
use die_yield_render::SceneBounds;
use eframe::egui;
use serde::{Deserialize, Serialize};

/// Interactive die-yield workbench shared by native and browser builds.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YieldWorkbench {
    inputs: FabricationInputs,
}

impl YieldWorkbench {
    /// Creates the application and restores persisted settings when available.
    #[must_use]
    pub fn new(context: &eframe::CreationContext<'_>) -> Self {
        context.egui_ctx.set_visuals(egui::Visuals::dark());
        context
            .storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default()
    }
}

impl eframe::App for YieldWorkbench {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            let scene = SceneBounds::from_inputs(&self.inputs);
            ui.heading("Wafer yield workbench");
            ui.label("Native and WebAssembly workspace initialized.");
            ui.separator();
            ui.monospace(format!(
                "Wafer: {:.0} mm  |  Usable: {:.0} mm",
                scene.diameter_mm, scene.usable_diameter_mm
            ));
        });
    }
}
