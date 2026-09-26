#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use clap::Parser;
use eframe::egui;

use app::GuiApp;

#[derive(Parser)]
#[command(
    name = "pantones-gui",
    version,
    about = "Pantone warehouse stock tracker (GUI)"
)]
struct Cli {
    /// Path to the CSV file
    #[arg(default_value = "pantones.csv")]
    file: String,
}

fn main() -> eframe::Result<()> {
    let cli = Cli::parse();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Pantones — склад пантонов")
            .with_inner_size([1000.0, 640.0])
            .with_min_inner_size([640.0, 400.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "Pantones",
        native_options,
        Box::new(move |cc| Ok(Box::new(GuiApp::new(cc, cli.file)))),
    )
}
