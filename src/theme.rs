use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
pub const BG: Color32 = Color32::from_rgb(16, 18, 23);
pub const PANEL: Color32 = Color32::from_rgb(25, 28, 36);
pub const LINE: Color32 = Color32::from_rgb(43, 48, 62);
pub const TEXT: Color32 = Color32::from_rgb(236, 238, 246);
pub const MUTED: Color32 = Color32::from_rgb(160, 169, 188);
pub const PURPLE: Color32 = Color32::from_rgb(181, 165, 255);
pub const MINT: Color32 = Color32::from_rgb(118, 221, 189);
pub const COLORS: [Color32; 8] = [
    PURPLE,
    MINT,
    Color32::from_rgb(115, 189, 241),
    Color32::from_rgb(238, 185, 120),
    Color32::from_rgb(236, 145, 176),
    Color32::from_rgb(126, 147, 227),
    Color32::from_rgb(201, 202, 120),
    Color32::from_rgb(85, 92, 112),
];
pub fn apply(ctx: &egui::Context) {
    let mut s = (*ctx.style()).clone();
    s.visuals = egui::Visuals::dark();
    s.visuals.panel_fill = BG;
    s.visuals.window_fill = PANEL;
    s.visuals.extreme_bg_color = Color32::from_rgb(18, 21, 28);
    s.visuals.override_text_color = Some(TEXT);
    s.visuals.selection.bg_fill = Color32::from_rgb(43, 38, 63);
    s.visuals.selection.stroke.color = PURPLE;
    s.visuals.widgets.inactive.bg_fill = Color32::from_rgb(30, 34, 44);
    s.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, LINE);
    s.visuals.widgets.hovered.bg_fill = Color32::from_rgb(43, 38, 63);
    s.visuals.widgets.active.bg_fill = Color32::from_rgb(65, 56, 91);
    s.visuals.hyperlink_color = PURPLE;
    s.spacing.item_spacing = Vec2::new(10., 8.);
    s.spacing.button_padding = Vec2::new(12., 8.);
    for (kind, size) in [
        (egui::TextStyle::Body, 13.),
        (egui::TextStyle::Button, 12.),
        (egui::TextStyle::Small, 10.),
        (egui::TextStyle::Heading, 22.),
    ] {
        s.text_styles.insert(kind, FontId::proportional(size));
    }
    ctx.set_style(s);
    let mut fonts = egui::FontDefinitions::default();
    fonts.families.insert(
        egui::FontFamily::Name("bold".into()),
        fonts.families[&egui::FontFamily::Proportional].clone(),
    );
    ctx.set_fonts(fonts);
    #[cfg(windows)]
    if let Some(windir) = std::env::var_os("WINDIR") {
        // Load the installed system font without redistributing it.
        if let Ok(bytes) =
            std::fs::read(std::path::PathBuf::from(&windir).join("Fonts/segoeui.ttf"))
        {
            let mut fonts = egui::FontDefinitions::default();
            fonts.families.insert(
                egui::FontFamily::Name("bold".into()),
                fonts.families[&egui::FontFamily::Proportional].clone(),
            );
            fonts
                .font_data
                .insert("system".into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .get_mut(&egui::FontFamily::Proportional)
                .unwrap()
                .insert(0, "system".into());
            for (name, file) in [("bold", "segoeuib.ttf"), ("symbols", "seguisym.ttf")] {
                if let Ok(bytes) =
                    std::fs::read(std::path::PathBuf::from(&windir).join("Fonts").join(file))
                {
                    fonts
                        .font_data
                        .insert(name.into(), egui::FontData::from_owned(bytes).into());
                    if name == "bold" {
                        let mut family = fonts.families[&egui::FontFamily::Proportional].clone();
                        family.insert(0, name.into());
                        fonts
                            .families
                            .insert(egui::FontFamily::Name("bold".into()), family);
                    } else {
                        fonts
                            .families
                            .get_mut(&egui::FontFamily::Proportional)
                            .unwrap()
                            .push(name.into());
                    }
                }
            }
            ctx.set_fonts(fonts);
        }
    }
}
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(15)
        .inner_margin(20)
}
pub fn eyebrow(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(10.).color(MUTED));
}
pub fn muted(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).size(11.).color(MUTED));
}
pub fn count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
