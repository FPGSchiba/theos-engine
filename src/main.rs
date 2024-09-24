// TODO: Hot reloading of theos engine with UI
// Default UI to start and edit levels
// Lib functionality to support loading of Data objects
// Common definition of components for Bevy's Components system
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
use eframe::egui;
use eframe::egui::IconData;
use state::TheosEngineState;
mod state;

fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let icon: Vec<u8> = include_bytes!("../assets/rust-icon.png").to_vec();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([320.0, 240.0])
            .with_min_inner_size([320.0, 240.0])
            .with_icon(IconData {
                rgba: icon,
                width: 32,
                height: 32,
            }),
        ..Default::default()
    };

    // Our application state:
    let mut state = TheosEngineState {
        age: 40,
        name: "Testing".to_string(),
    };

    eframe::run_simple_native("My egui App", options, move |ctx, _frame| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("My egui Application");
            ui.horizontal(|ui| {
                let name_label = ui.label("Your name: ");
                ui.text_edit_singleline(&mut state.name)
                    .labelled_by(name_label.id);
            });
            ui.add(egui::Slider::new(&mut state.age, 0..=120).text("age"));
            if ui.button("Increment").clicked() {
                state.age += 1;
            }
            ui.label(format!("Hello '{}', age {}", state.name, state.age));
        });
    })
}
