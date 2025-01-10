use egui::{Color32, Pos2, Context, Key, TextStyle, FontId, Painter, Vec2, Rect, TextureOptions, vec2};
use image::{self, Rgba, RgbaImage};
use tinyfiledialogs::{MessageBoxIcon, OkCancel};
use std::fs::{File, read};
use std::io::{Write, Read};
use std::path::Path;
use egui::load::SizedTexture;

mod icons;
use icons::*;

type ColorMatrix = Vec<Vec<Option<Color32>>>;
type RefMatrix = Vec<Vec<Option<(usize, usize)>>>;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    //$ Save
    file_path: Option<String>,

    //$ Not save
    #[serde(skip)]
    color_matrix: ColorMatrix,
    #[serde(skip)]
    ref_matrix: Vec<RefMatrix>,

    //$ Helper data
    /*//# Icons
    #[serde(skip)]
    icons: Icon,*/
    //# Drag from canvas to ref mechanic
    #[serde(skip)]
    start_drag: Option<Pos2>,
    #[serde(skip)]
    end_drag: Option<Pos2>,
    #[serde(skip)]
    is_dragging: bool,
    #[serde(skip)]
    drag_color: Option<Color32>,
    #[serde(skip)]
    drag_ref: Option<(usize, usize)>,
    #[serde(skip)]
    drag_where: u8,
    //# Frame and animations mechanism
    #[serde(skip)]
    current_frame: usize,
    #[serde(skip)]
    is_animating: bool,
    #[serde(skip)]
    last_update: std::time::Instant,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct PxRefFile {
    ref_png: String,
    ref_matrix: Vec<RefMatrix>,
}

impl PxRefFile {
    fn save(&self) {
        if let Some(mut render_path) = tinyfiledialogs::save_file_dialog("Save as", "") {
            if !render_path.ends_with(".pxref") { render_path = format!("{}.pxref", render_path); }
            if let Ok(mut file) = File::create(render_path) {
                if let Ok(json) = serde_json::to_string_pretty(self) {
                    file.write_all(json.as_bytes()).unwrap_or_else(|e| {
                        tinyfiledialogs::message_box_ok("Failed to Save Ref", e.to_string().as_str(), MessageBoxIcon::Error);
                    });
                }
            } else {
                tinyfiledialogs::message_box_ok("Failed to Save Ref", "", MessageBoxIcon::Error);
            }
        }
    }
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            file_path: None,
            color_matrix: vec![vec![None; 16]; 16],
            ref_matrix: vec![vec![vec![None; 16]; 16]],
            start_drag: None,
            end_drag: None,
            is_dragging: false,
            drag_color: None,
            drag_ref: None,
            current_frame: 0,
            drag_where: 2, // 2 is none
            is_animating: false,
            last_update: std::time::Instant::now(),
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            let mut stored_state: TemplateApp = eframe::get_value(storage, eframe::APP_KEY).unwrap_or(Default::default());
            if let Some(file_path) = &stored_state.file_path {
                stored_state.color_matrix = parse_png_to_matrix(&file_path);
            }
            return stored_state;
        }
        Default::default()
    }
}

impl eframe::App for TemplateApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui
        #[allow(non_snake_case)]
        let ICON:Icon = load_icons(ctx);

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::menu::bar(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                ui.menu_button("File", |ui| {
                    if ui.button("Load PNG").clicked() {
                        match tinyfiledialogs::open_file_dialog("Open", "", None) {
                            Some(file) => {
                                if file.ends_with(".png") {
                                    self.file_path = Some(file.clone());
                                    self.color_matrix = parse_png_to_matrix(&file);
                                } else {
                                    tinyfiledialogs::message_box_ok(
                                        "Invalid File", "Please pick a .png file",
                                        MessageBoxIcon::Error);
                                    self.file_path = None;
                                }
                            },
                            None => self.file_path = None,
                        }
                    }
                    if ui.button("Load Ref").clicked() {
                        match tinyfiledialogs::open_file_dialog("Open", "", None) {
                            Some(path) => {
                                if path.ends_with(".pxref") {
                                    if let Ok(mut file) = File::open(path) {
                                        let mut json_str = String::new();
                                        if let Err(E) = file.read_to_string(&mut json_str) {
                                            tinyfiledialogs::message_box_ok(
                                                "Unable to open Ref", &E.to_string(),
                                                MessageBoxIcon::Error);
                                        };
                                        if let Ok(parsed_data) = serde_json::from_str::<PxRefFile>(&json_str) {
                                            self.file_path = Some(parsed_data.ref_png.clone());
                                            self.color_matrix = parse_png_to_matrix(&parsed_data.ref_png);
                                            self.ref_matrix = parsed_data.ref_matrix;
                                            self.current_frame = 0;
                                        } else {
                                            tinyfiledialogs::message_box_ok(
                                                "Unable to open Ref", "",
                                                MessageBoxIcon::Error);
                                        };
                                    } else {
                                        tinyfiledialogs::message_box_ok(
                                            "Unable to open Ref", "",
                                            MessageBoxIcon::Error);
                                    };
                                } else {
                                    tinyfiledialogs::message_box_ok(
                                        "Unable to open Ref", "Please pick a .pxref file",
                                        MessageBoxIcon::Error);
                                }

                            }
                            _ => {}
                        }
                    }
                    if ui.button("Save Image").clicked() {
                        if let Some(mut render_path) = tinyfiledialogs::save_file_dialog("Render as", "") {
                            let mut img = RgbaImage::new((16 * self.ref_matrix.len()) as u32, 16);
                            // Renders all frames next to one another
                            for (k, frame) in self.ref_matrix.iter().enumerate() {
                                for (i, row) in frame.iter().enumerate() {
                                    for (j, col_opt) in row.iter().enumerate() {
                                        if let Some(pos) = col_opt {
                                            let color = self.color_matrix[pos.0][pos.1];
                                            if let Some(color) = color {
                                                img.put_pixel(i as u32 + 16*k as u32, j as u32, Rgba([color[0], color[1], color[2], color[3]]));
                                            } else {
                                                img.put_pixel(i as u32 + 16*k as u32, j as u32, Rgba([0, 0, 0, 0]));
                                            }
                                        }
                                    }
                                }
                            }
                            if !render_path.ends_with(".png") {
                                render_path = format!("{}.png", render_path);
                            }
                            img.save(render_path).unwrap_or_else(|e| {
                                tinyfiledialogs::message_box_ok("Failed to Render Image", e.to_string().as_str(), MessageBoxIcon::Error);
                            });
                        }
                    }
                    if ui.button("Save Ref").clicked() {
                        let data = PxRefFile {
                            ref_png: self.file_path.clone().unwrap(),
                            ref_matrix: self.ref_matrix.clone(),
                        };
                        data.save();
                    }
                    if !is_web {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Clear Canvas").clicked() {
                        match tinyfiledialogs::message_box_ok_cancel("Clear Canvas", "Are you sure?", MessageBoxIcon::Error, OkCancel::Cancel) {
                            OkCancel::Ok => {
                                self.ref_matrix[self.current_frame] = vec![vec![None; 16]; 16];
                            }
                            OkCancel::Cancel => {}
                        }
                    }
                });
                ui.add_space(16.0);

                //egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let square_size = 16; // Width and height
            let start = egui::pos2(16.0, 16.0); // Top-left corner

            let painter = ui.painter();

            //% left panel
            for (x, row) in self.ref_matrix[self.current_frame].iter().enumerate() {
                for (y, data) in row.iter().enumerate() {
                    let pos = start + egui::vec2(0. + (x*square_size) as f32, 16. + (y*square_size) as f32);
                    let color:Color32;
                    let mut ref_num:Option<String> = None;
                    if let Some(coords) = data {
                        if let Some(color_) = self.color_matrix[coords.0][coords.1] {
                            color = color_;
                            ref_num = Some((coords.1*16 + coords.0 + 1).to_string());
                        } else {
                            color = get_checkerboard(x, y);
                        }
                    } else {
                        color = get_checkerboard(x, y);
                    }
                    painter.rect_filled(
                        egui::Rect::from_min_size(pos, egui::vec2(square_size as f32, square_size as f32)),
                        0.0,    // Corner rounding (0 for a square)
                        color,
                    );
                    if let Some(ref_num) = ref_num {
                        painter.text(pos, egui::Align2::LEFT_TOP, ref_num,
                                     egui::FontId::new(6.0, egui::FontFamily::Proportional),
                                     if color == Color32::WHITE { Color32::GRAY } else { Color32::WHITE });
                    }
                }
            }

            //% Right panel
            for (x, row) in self.color_matrix.iter_mut().enumerate() {
                for (y, col) in row.iter_mut().enumerate() {
                    let pos = start + egui::vec2(272. + (x*square_size) as f32, 16. + (y*square_size) as f32);
                    let color:Color32;
                    if let Some(color_) = col {
                        color = *color_;
                    } else {
                        color = get_checkerboard(x, y);
                    }
                    painter.rect_filled(
                        egui::Rect::from_min_size(pos, egui::vec2(square_size as f32, square_size as f32)),
                        0.0,    // Corner rounding (0 for a square)
                        color,
                    );
                }
            }

            //$ Mouse Drag Logic
            if ctx.input(|i| i.pointer.is_decidedly_dragging()) {
                if let Some(start) = ctx.input(|i| i.pointer.press_origin()) {
                    self.is_dragging = true;
                    self.start_drag = Some(start);
                    self.end_drag = ctx.input(|i| i.pointer.latest_pos())
                } else {
                    self.is_dragging = false;
                    self.drag_where = 2;
                }
            } else {
                self.is_dragging = false;
                self.drag_where = 2;
            }
            //println!("{:?}, {:?} -- {}", self.start_drag, self.end_drag, self.is_dragging);
            if let Some(start_drag) = self.start_drag {
                //# Dragging on the right
                if start_drag.x > 32. + 16.*square_size as f32{
                    self.drag_where = 1;
                    let mut start_pos:Pos2 = start_drag;
                    start_pos.x -= 32. + 16.*square_size as f32;
                    start_pos.y -= 32.;
                    let mut start_ints:(usize, usize) = (start_pos.x as usize, start_pos.y as usize);
                    start_ints.0 = start_ints.0/16;
                    start_ints.1 = start_ints.1/16;
                    self.drag_ref = Some((start_ints.0, start_ints.1));
                    if start_ints.0 < 16 && start_ints.1 < 16 {
                        self.drag_color = self.color_matrix[start_ints.0][start_ints.1];
                    }
                } else { //# Dragging on the left
                    //self.drag_color = None;
                    self.drag_where = 0;
                    let mut start_pos:Pos2 = start_drag;
                    start_pos.x -= 16.;
                    start_pos.y -= 32.;
                    let mut start_ints:(usize, usize) = (start_pos.x as usize, start_pos.y as usize);
                    start_ints.0 = start_ints.0/16;
                    start_ints.1 = start_ints.1/16;
                    self.drag_ref = Some((start_ints.0, start_ints.1));
                    if start_ints.0 < 16 && start_ints.1 < 16 {
                        if let Some(ref_indices) = self.ref_matrix[self.current_frame][start_ints.0][start_ints.1] {
                            self.drag_color = self.color_matrix[ref_indices.0][ref_indices.1];
                        }
                    }
                }
            } else { self.drag_color = None; self.drag_where = 2; }

            if self.is_dragging == true && self.drag_where == 1{
                if let Some(color) = self.drag_color {
                    painter.rect_filled(
                        egui::Rect::from_min_size(self.end_drag.unwrap() - egui::vec2(8., 8.), egui::vec2(square_size as f32, square_size as f32)),
                        0.0,
                        color,
                    );
                }
            } else if self.drag_where == 1 && self.is_dragging == false {
                if let Some(end_drag) = self.end_drag {
                    if let Some(drag_ref) = self.drag_ref {
                        let mut start_pos:Pos2 = end_drag;
                        start_pos.x -= 16.;
                        start_pos.y -= 32.;
                        let mut start_ints:(usize, usize) = (start_pos.x as usize, start_pos.y as usize);
                        start_ints.0 = start_ints.0/16;
                        start_ints.1 = start_ints.1/16;
                        if start_ints.0 < 16 && start_ints.1 < 16 {
                            self.ref_matrix[self.current_frame][start_ints.0][start_ints.1] = Some(drag_ref);
                        }
                        self.start_drag = None;
                        self.end_drag = None;
                        self.drag_color = None;
                        self.drag_ref = None;
                        self.drag_where = 2;
                    }
                }
            }
            if self.is_dragging == true && self.drag_where == 0 {
                if let Some(mut latest) = self.end_drag {
                    latest.x -= 16.;
                    latest.y -= 32.;
                    let (xc, yc) = (latest.x as usize / 16, latest.y as usize / 16);
                    if xc < 16 && yc < 16 {
                        if ui.input(|i| i.modifiers.shift) { // Mass delete
                            self.ref_matrix[self.current_frame][xc][yc] = None;
                            self.drag_where = 2;
                        } else if ui.input(|i| i.modifiers.ctrl)
                            || ui.input(|i| i.modifiers.mac_cmd) { // Reorder
                            if let Some(start) = self.start_drag {
                                let (xc, yc) = ((start.x - 16.) as usize /16, (start.y - 32.) as usize / 16);
                                //self.ref_matrix[self.current_frame][xc][yc] = None;
                                // cover up so it looks like it is actually being dragged, not copied
                                // this has a little visual bug but for the most part it's fine
                                painter.rect_filled(
                                    Rect::from_min_size(Pos2::new((xc + 1) as f32 * 16., (yc + 2) as f32 * 16.), vec2(square_size as f32, square_size as f32)),
                                    0.0,
                                    get_checkerboard(xc, yc),
                                );
                            }
                            if let Some(color) = self.drag_color {
                                painter.rect_filled(
                                    egui::Rect::from_min_size(self.end_drag.unwrap() - egui::vec2(8., 8.), egui::vec2(square_size as f32, square_size as f32)),
                                    0.0,
                                    color,
                                );
                            }
                            self.drag_where = 0;
                        } else { // Copy from right
                            //? Turned off this feature temporarily, as I feel it can lead to user-errors
                            //? will turn back on when I implement ctrl-z
                            //self.ref_matrix[self.current_frame][xc][yc] = Some((xc, yc));
                            //self.drag_where = 2;
                        }
                    }
                }
            } else if self.drag_where == 0 && self.is_dragging == false {
                if ui.input(|i| i.modifiers.ctrl)
                    || ui.input(|i| i.modifiers.mac_cmd) {
                    if let Some(end_drag) = self.end_drag {
                        if let Some(drag_ref) = self.drag_ref {
                            let (xc, yc) = ((end_drag.x - 16.) as usize / 16, (end_drag.y - 32.) as usize / 16);
                            if xc < 16 && yc < 16 {
                                self.ref_matrix[self.current_frame][xc][yc] = self.ref_matrix[self.current_frame][drag_ref.0][drag_ref.1];
                            }
                            if let Some(start_drag) = self.start_drag {
                                let (xs, ys) = ((start_drag.x - 16.) as usize / 16, (start_drag.y - 32.) as usize / 16);
                                if xc < 16 && yc < 16 {
                                    self.ref_matrix[self.current_frame][xs][ys] = None;
                                }
                            }
                        }
                    }
                }
                self.start_drag = None;
                self.end_drag = None;
                self.drag_color = None;
                self.drag_ref = None;
                self.drag_where = 2;
            }
            // Eraser Left
            if ctx.input(|i| i.pointer.any_pressed() && i.modifiers.shift) {
                if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
                    if pos.x < (16 + 16*16) as f32 {
                        let coords = ((pos.x  - 16.) as usize / 16, (pos.y - 32.) as usize / 16);
                        if coords.0 < 16 && coords.1 < 16 {
                            self.ref_matrix[self.current_frame][coords.0][coords.1] = None;
                        }
                    }
                }
            }

            //$ Frames
            let frames_len = self.ref_matrix.len();
            for j in 0..frames_len {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(16.0 + (32*j + 16*j) as f32 , 304.0), // Top-left corner of the rectangle
                    egui::vec2(32.0, 32.0),  // Width and height of the rectangle
                );
                let response = ui.interact(rect, ui.id().with(j), egui::Sense::click());
                let color = if ui.input(|i| i.modifiers.shift) && response.hovered() {
                    Color32::RED
                } else if self.current_frame == j {
                    Color32::DARK_GRAY
                } else if response.hovered() {
                    Color32::GRAY
                } else {
                    Color32::LIGHT_GRAY
                };
                ui.painter().rect_filled(rect, 0.0, color);
                ui.painter().text(
                    rect.center() - egui::vec2(8., 12.),
                    egui::Align2::LEFT_TOP,
                    &format!("{}", j+1),
                    FontId::proportional(20.0),
                    Color32::WHITE,
                );
                if response.clicked() {
                    self.current_frame = j;
                    if ui.input(|i| i.modifiers.shift) {
                        if frames_len != 1 {
                            match tinyfiledialogs::message_box_ok_cancel
                                ("Do you want to remove the frame", "This action can not be undone",
                                 MessageBoxIcon::Warning, OkCancel::Cancel) {
                                OkCancel::Ok => {
                                    self.ref_matrix.remove(self.current_frame);
                                    tinyfiledialogs::message_box_ok("Frame Removed", &format!("Removed frame {}", self.current_frame + 1), MessageBoxIcon::Info);
                                    self.current_frame -= 1;
                                }
                                OkCancel::Cancel => {}
                            }
                        } else {
                            tinyfiledialogs::message_box_ok("Invalid action", "Can not remove the only frame", MessageBoxIcon::Info);
                        }
                    }
                }
            }

            let rect = egui::Rect::from_min_size(
                egui::pos2(16.0 + ((32 + 16)*frames_len) as f32 , 304.0), // Top-left corner of the rectangle
                egui::vec2(32.0, 32.0),  // Width and height of the rectangle
            );
            let response = ui.interact(rect, ui.id().with("Add1"), egui::Sense::click());
            let color = if response.hovered() { Color32::GRAY } else { Color32::LIGHT_GRAY };
            ui.painter().rect_filled(rect, 0.0, color);
            ui.painter().text(
                rect.center() - egui::vec2(8., 12.),
                egui::Align2::LEFT_TOP,
                "+",
                FontId::proportional(20.0),
                Color32::WHITE,
            );
            if response.clicked() {
                self.ref_matrix.push(vec![vec![None; 16]; 16]);
                self.current_frame = frames_len;
            }

            //$ Play animation
                //? Fix button size (optional)
            let button_size = Vec2::new(32.0, 32.0); // Button size (specified by icon size, not independent)
            const ICON_BUTTON_SIZE:Vec2 = Vec2::new(24.0, 24.0); // Image size

            if ui_with_image_button(ui, if !self.is_animating {&ICON.play} else {&ICON.pause}, Vec2::new(560., 32.), button_size, ICON_BUTTON_SIZE) {
                if self.is_animating {
                    self.is_animating = false
                } else {self.is_animating = true} // switch
                println!("Play/Pause button pressed");
            }

        });
    }

    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}

// Transparent png checkerboard using LIGHTGRAY and GRAY
fn get_checkerboard(x:usize, y:usize) -> Color32 {
    if (x+y)%2 == 0 {
        Color32::LIGHT_GRAY
    } else {
        Color32::GRAY
    }
}

fn parse_png_to_matrix(file_path: &str) -> Vec<Vec<Option<Color32>>> {
    // Load the image from file
    let img = image::ImageReader::open(file_path)
        .expect("Failed to open image file")
        .decode()
        .expect("Failed to decode image");

    // Convert image to RGBA8 format
    let img = img.to_rgba8();
    let (width, height) = img.dimensions();

    // Create the pixel matrix
    let mut pixel_matrix = Vec::with_capacity(height as usize);

    for y in 0..height {
        let mut row = Vec::with_capacity(width as usize);
        for x in 0..width {
            // Extract RGBA values for each pixel
            let pixel_colors = img.get_pixel(x, y).0;
            let pixel = Color32::from_rgba_unmultiplied(pixel_colors[0], pixel_colors[1], pixel_colors[2], pixel_colors[3]);
            if pixel_colors[3] == 0 {
                row.push(None)
            } else {
                row.push(Some(pixel));
            }
        }
        pixel_matrix.push(row);
    }

    transpose(pixel_matrix)
}

fn transpose<T: Clone>(matrix: Vec<Vec<T>>) -> Vec<Vec<T>> {
    if matrix.is_empty() || matrix[0].is_empty() {
        return vec![];
    }

    let row_len = matrix.len();
    let col_len = matrix[0].len();

    (0..col_len)
        .map(|col| {
            (0..row_len)
                .map(|row| matrix[row][col].clone())
                .collect()
        })
        .collect()
}

fn ui_with_image_button(
    ui: &mut egui::Ui,
    texture: &egui::TextureHandle,
    position: Vec2,
    button_size: Vec2,
    image_size: Vec2,
) -> bool {
    // Define the button's rectangle
    let rect = Rect::from_min_size(position.to_pos2(), button_size);
    //ui.painter().rect_filled(rect, egui::Rounding::ZERO, Color32::WHITE);

    // Create an ImageButton with the image texture
    let mut sized_texture = SizedTexture::from_handle(texture);
    sized_texture.size = image_size;
    let image_button = egui::ImageButton::new(sized_texture);

    // Render the button at the specified position and return if it was clicked
    ui.put(rect, image_button).clicked()
}