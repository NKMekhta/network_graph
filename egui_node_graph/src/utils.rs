use eframe::egui;
#[allow(clippy::module_name_repetitions)]
pub trait ColorUtils {
    /// Multiplies the color rgb values by `factor`, keeping alpha untouched.
    fn lighten(&self, factor: f32) -> Self;
}


impl ColorUtils for egui::Color32 {
    fn lighten(&self, factor: f32) -> Self {
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
    
        egui::Color32::from_rgba_premultiplied(
            (f32::from(self.r()) * factor) as u8,
            (f32::from(self.g()) * factor) as u8,
            (f32::from(self.b()) * factor) as u8,
            self.a(),
        )
    }
}
