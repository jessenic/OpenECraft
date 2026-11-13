//! Logos of the catalogs of the parts server ([`Catalog::logo`]): loaded once per
//! logo file, decoded (PNG, JPEG, SVG) into textures and drawn beside the catalogs on
//! a white plate, which the logos are made for (also in the dark theme).

use crate::client::{Reply, ServerClient};
use egui::{Color32, ColorImage};
use openecraft_parts::portal::Catalog;
use resvg::{tiny_skia, usvg};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

/// Height of the decoded logos in pixels (sharp up to about 48 points on 2x screens).
pub const PIXELS: u32 = 96;
/// Widest logo, as a multiple of its height.
const MAX_ASPECT: f32 = 5.0;

/// The image of a logo file (by its name: SVG, PNG, JPEG, GIF or WebP); SVG logos are
/// drawn `PIXELS` high, very wide ones narrower to fit `MAX_ASPECT`. `None` if it
/// cannot be read.
pub fn decode(name: &str, data: &[u8]) -> Option<ColorImage> {
    if name.to_lowercase().ends_with(".svg") {
        decode_svg(data)
    } else {
        let img = image::load_from_memory(data).ok()?.to_rgba8();
        let (w, h) = (img.width(), img.height());
        if w == 0 || h == 0 {
            return None;
        }
        Some(ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw()))
    }
}

/// Fonts for the texts of SVG logos: egui's own (no system fonts, the same on every
/// platform and in the browser).
fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            for data in egui::FontDefinitions::default().font_data.values() {
                db.load_font_data((*data.font).as_ref().to_vec());
            }
            let family = db.faces().flat_map(|f| f.families.iter()).map(|(name, _)| name.clone()).find(|n| n.starts_with("Ubuntu"));
            if let Some(family) = family {
                db.set_sans_serif_family(family.clone());
                db.set_serif_family(family);
            }
            Arc::new(db)
        })
        .clone()
}

fn decode_svg(data: &[u8]) -> Option<ColorImage> {
    let fonts = fonts();
    let family = fonts.family_name(&usvg::fontdb::Family::SansSerif).to_string();
    let options = usvg::Options { fontdb: fonts, font_family: family, ..Default::default() };
    let tree = usvg::Tree::from_data(data, &options).ok()?;
    let size = tree.size();
    if size.width() <= 0.0 || size.height() <= 0.0 {
        return None;
    }
    // The whole logo, also a very wide one.
    let scale = (PIXELS as f32 / size.height()).min(PIXELS as f32 * MAX_ASPECT / size.width());
    let (width, height) = ((size.width() * scale).ceil().max(1.0) as u32, (size.height() * scale).ceil().max(1.0) as u32);
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Some(ColorImage::from_rgba_premultiplied([width as usize, height as usize], pixmap.data()))
}

enum State {
    Requested,
    /// Downloaded, decoded when it is drawn the first time.
    Data(String, Vec<u8>),
    Ready(egui::TextureHandle),
    Failed,
}

/// The logos loaded so far, by the id of their file (a changed logo is a new file).
#[derive(Default)]
pub struct Logos {
    states: HashMap<String, State>,
}

impl Logos {
    /// Keeps a logo the server sent ([`Reply::Logo`]).
    pub fn receive(&mut self, reply: &Reply) {
        if let Reply::Logo(file, name, result) = reply {
            let state = match result {
                Ok(data) => State::Data(name.clone(), data.clone()),
                Err(_) => State::Failed,
            };
            self.states.insert(file.clone(), state);
        }
    }

    /// Forgets all logos (another server).
    pub fn clear(&mut self) {
        self.states.clear();
    }

    /// True if the logo of this file was received (or failed).
    pub fn loaded(&self, file: &str) -> bool {
        matches!(self.states.get(file), Some(State::Data(..) | State::Ready(_) | State::Failed))
    }

    /// The texture of the catalog's logo; loads the logo when it is first needed.
    fn texture(&mut self, ctx: &egui::Context, client: &mut ServerClient, catalog: &Catalog) -> Option<egui::TextureHandle> {
        let logo = catalog.logo.as_ref()?;
        let state = self.states.entry(logo.id.clone()).or_insert_with(|| {
            client.catalog_logo(ctx, &catalog.id, &logo.id, &logo.name);
            State::Requested
        });
        if let State::Data(name, data) = state {
            *state = match decode(name, data) {
                Some(image) => State::Ready(ctx.load_texture(format!("catalog-logo-{}", logo.id), image, egui::TextureOptions::LINEAR)),
                None => State::Failed,
            };
        }
        match state {
            State::Ready(texture) => Some(texture.clone()),
            _ => None,
        }
    }

    /// Draws the logo of the catalog at most `height` points high (and `MAX_ASPECT`
    /// times as wide) on its plate. Nothing is drawn while it loads or if the catalog
    /// has none.
    pub fn show(&mut self, ui: &mut egui::Ui, client: &mut ServerClient, catalog: &Catalog, height: f32) -> Option<egui::Response> {
        let texture = self.texture(ui.ctx(), client, catalog)?;
        let size = fit(texture.size(), egui::vec2(height * MAX_ASPECT, height));
        let frame = egui::Frame::new().fill(Color32::WHITE).corner_radius(3.0).inner_margin(2.0);
        let response = frame.show(ui, |ui| ui.add(egui::Image::new((texture.id(), size)))).response;
        Some(response.on_hover_text(&catalog.name))
    }

    /// Like [`Logos::show`] in a column of logos: always takes `height` × `MAX_ASPECT`
    /// (also without logo), so that what follows lines up.
    pub fn show_in_column(&mut self, ui: &mut egui::Ui, client: &mut ServerClient, catalog: &Catalog, height: f32) {
        let column = egui::vec2(height * MAX_ASPECT + 4.0, height + 4.0);
        let (rect, _) = ui.allocate_exact_size(column, egui::Sense::hover());
        let mut cell = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
        self.show(&mut cell, client, catalog, height);
    }
}

/// Size of an image of `pixels` scaled to fit into `room`, keeping its proportions.
pub fn fit(pixels: [usize; 2], room: egui::Vec2) -> egui::Vec2 {
    let (w, h) = (pixels[0].max(1) as f32, pixels[1].max(1) as f32);
    let scale = (room.x / w).min(room.y / h);
    egui::vec2(w * scale, h * scale)
}

#[cfg(test)]
mod tests;
