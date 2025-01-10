fn load_png_as_texture(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    // Load the image using the `image` crate
    let img = image::open(path).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(
        [width as usize, height as usize],
        img.as_raw(),
    );

    // Upload the image as a texture in `egui`
    Some(ctx.load_texture("png_texture", color_image, egui::TextureOptions::LINEAR))
}

const ERR_MSG:&str = "Error loading a texture";
//pub const ICON_PLAY:egui::TextureHandle = load_png_as_texture(ctx, "assets/icons/icon_play.png").expect(ERR_MSG);
pub struct Icon {
    pub play: egui::TextureHandle,
    pub pause: egui::TextureHandle,
}
pub fn load_icons(ctx: &egui::Context) -> Icon {
    Icon {
        play: load_png_as_texture(ctx, "assets/icons/icon_play.png").expect(ERR_MSG),
        pause: load_png_as_texture(ctx, "assets/icons/icon_pause.png").expect(ERR_MSG),
    }
}