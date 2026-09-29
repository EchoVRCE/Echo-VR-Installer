//! Embedded images, uploaded to the GPU on first use at the exact size they are drawn
//! (pre-scaled with a Lanczos filter, like Swing's `SCALE_SMOOTH`).

use std::cell::RefCell;
use std::collections::HashMap;

use egui::{ColorImage, Context, TextureHandle, TextureOptions};

const IMAGES: &[(&str, &[u8])] = &[
    (
        "tipbox_top.png",
        include_bytes!("../../assets/img/tipbox_top.png"),
    ),
    ("icon.png", include_bytes!("../../assets/img/icon.png")),
    (
        "main_background.jpg",
        include_bytes!("../../assets/img/main_background.jpg"),
    ),
    (
        "left_sidebar.jpg",
        include_bytes!("../../assets/img/left_sidebar.jpg"),
    ),
    (
        "sidebar_selected.png",
        include_bytes!("../../assets/img/sidebar_selected.png"),
    ),
    (
        "sidebar_hover.png",
        include_bytes!("../../assets/img/sidebar_hover.png"),
    ),
    (
        "logo_echovr.png",
        include_bytes!("../../assets/img/logo_echovr.png"),
    ),
    (
        "play_button.png",
        include_bytes!("../../assets/img/play_button.png"),
    ),
    (
        "play_button_blank.png",
        include_bytes!("../../assets/img/play_button_blank.png"),
    ),
    (
        "update_button.png",
        include_bytes!("../../assets/img/update_button.png"),
    ),
    (
        "update_button_alert.png",
        include_bytes!("../../assets/img/update_button_alert.png"),
    ),
    (
        "hardware_pc.png",
        include_bytes!("../../assets/img/hardware_pc.png"),
    ),
    (
        "hardware_quest.png",
        include_bytes!("../../assets/img/hardware_quest.png"),
    ),
    (
        "news_header.png",
        include_bytes!("../../assets/img/news_header.png"),
    ),
    (
        "news_fallback.jpg",
        include_bytes!("../../assets/img/news_fallback.jpg"),
    ),
    (
        "card_bg.png",
        include_bytes!("../../assets/img/card_bg.png"),
    ),
    (
        "panel_bg.png",
        include_bytes!("../../assets/img/panel_bg.png"),
    ),
    (
        "icon_play.png",
        include_bytes!("../../assets/img/icon_play.png"),
    ),
    (
        "icon_spark.png",
        include_bytes!("../../assets/img/icon_spark.png"),
    ),
    (
        "icon_echovrce.png",
        include_bytes!("../../assets/img/icon_echovrce.png"),
    ),
    (
        "world_map.png",
        include_bytes!("../../assets/img/world_map.png"),
    ),
    (
        "icon_community.png",
        include_bytes!("../../assets/img/icon_community.png"),
    ),
    (
        "button_up.png",
        include_bytes!("../../assets/img/button_up.png"),
    ),
    (
        "button_down.png",
        include_bytes!("../../assets/img/button_down.png"),
    ),
    (
        "button_highlighted.png",
        include_bytes!("../../assets/img/button_highlighted.png"),
    ),
    (
        "button_up_middle.png",
        include_bytes!("../../assets/img/button_up_middle.png"),
    ),
    (
        "button_down_middle.png",
        include_bytes!("../../assets/img/button_down_middle.png"),
    ),
    (
        "button_highlighted_middle.png",
        include_bytes!("../../assets/img/button_highlighted_middle.png"),
    ),
    (
        "button_up_small.png",
        include_bytes!("../../assets/img/button_up_small.png"),
    ),
    (
        "button_down_small.png",
        include_bytes!("../../assets/img/button_down_small.png"),
    ),
    (
        "button_highlighted_small.png",
        include_bytes!("../../assets/img/button_highlighted_small.png"),
    ),
];

const CLIPPY_GIF: &[u8] = include_bytes!("../../assets/img/clippy.gif");

fn decode(name: &str) -> image::RgbaImage {
    let bytes = IMAGES
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("unknown image {name}"))
        .1;
    image::load_from_memory(bytes)
        .expect("embedded image")
        .to_rgba8()
}

fn to_color_image(img: &image::RgbaImage) -> ColorImage {
    ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw())
}

/// Native pixel size of an embedded image.
pub fn native_size(name: &str) -> (u32, u32) {
    thread_local! {
        static SIZES: RefCell<HashMap<String, (u32, u32)>> = RefCell::new(HashMap::new());
    }
    SIZES.with(|s| {
        *s.borrow_mut().entry(name.to_string()).or_insert_with(|| {
            let bytes = IMAGES
                .iter()
                .find(|(n, _)| *n == name)
                .expect("known image")
                .1;
            image::ImageReader::new(std::io::Cursor::new(bytes))
                .with_guessed_format()
                .ok()
                .and_then(|r| r.into_dimensions().ok())
                .unwrap_or((0, 0))
        })
    })
}

/// Window icon data.
pub fn icon() -> egui::IconData {
    let img = decode("icon.png");
    egui::IconData {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    }
}

#[derive(Default)]
pub struct Assets {
    textures: RefCell<HashMap<(String, u32, u32), TextureHandle>>,
    clippy: RefCell<Option<Vec<TextureHandle>>>,
}

impl Assets {
    /// A texture for drawing `name` into a `w`x`h` (logical) rect. Downscales are done
    /// once on the CPU at the physical target size (a GPU minification of a 802px image
    /// into 300px aliases badly); upscales are left to linear filtering.
    pub fn tex(&self, ctx: &Context, name: &str, w: u32, h: u32) -> TextureHandle {
        let ppp = ctx.pixels_per_point();
        let (nw, nh) = native_size(name);
        let (tw, th) = (
            (w as f32 * ppp).round() as u32,
            (h as f32 * ppp).round() as u32,
        );
        let (tw, th) = if tw > 0 && th > 0 && (tw < nw || th < nh) {
            (tw, th)
        } else {
            (nw, nh)
        };
        let key = (name.to_string(), tw, th);
        if let Some(t) = self.textures.borrow().get(&key) {
            return t.clone();
        }
        let mut img = decode(name);
        if (img.width(), img.height()) != (tw, th) {
            img = image::imageops::resize(&img, tw, th, image::imageops::FilterType::Lanczos3);
        }
        let t = ctx.load_texture(
            format!("{name}@{w}x{h}"),
            to_color_image(&img),
            TextureOptions::LINEAR,
        );
        self.textures.borrow_mut().insert(key, t.clone());
        t
    }

    /// The Clippy GIF, one texture per (composited) frame.
    pub fn clippy_frames(&self, ctx: &Context) -> Vec<TextureHandle> {
        if let Some(f) = self.clippy.borrow().as_ref() {
            return f.clone();
        }
        use image::AnimationDecoder;
        let frames: Vec<TextureHandle> =
            image::codecs::gif::GifDecoder::new(std::io::Cursor::new(CLIPPY_GIF))
                .and_then(|d| d.into_frames().collect_frames())
                .map(|frames| {
                    frames
                        .iter()
                        .enumerate()
                        .map(|(i, f)| {
                            ctx.load_texture(
                                format!("clippy{i}"),
                                to_color_image(f.buffer()),
                                TextureOptions::LINEAR,
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
        *self.clippy.borrow_mut() = Some(frames.clone());
        frames
    }
}
