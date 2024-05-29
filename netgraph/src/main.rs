mod app;

fn main() {
    use eframe::egui::Visuals;

    eframe::run_native(
        "NetGraph",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(Visuals::dark());
            Box::<app::App>::default()
        }),
    )
    .expect("Failed to run native example");
}
