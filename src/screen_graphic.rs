use crate::texture_cache::{HashableGraphicProps, Hashf32, TextureCache};
use sdl3::{
    pixels::{Color, PixelFormat},
    render::{self, Canvas, RenderTarget},
    surface::Surface,
    ttf,
};
use std::hash::Hash;

pub trait ScreenImmGraphic {
    fn render_onto<T: RenderTarget>(
        &self,
        canvas: &mut Canvas<T>,
        top_left_pos: (i32, i32),
    ) -> Result<(), sdl3::Error>;
}

pub trait SizedScreenGraphic {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
}

pub trait ScreenTexture {
    fn generate_texture<'tc, U>(
        &self,
        texture_creator: &'tc render::TextureCreator<U>,
    ) -> Result<render::Texture<'tc>, ()>;
}

// TODO : Consider potentially transforming this into a macro
fn derive_texture<'tc, SSG: ScreenImmGraphic + SizedScreenGraphic, U>(
    graphic: &SSG,
    texture_creator: &'tc render::TextureCreator<U>,
) -> Result<render::Texture<'tc>, ()> {
    let mut canvas = Canvas::from_surface(
        Surface::new(graphic.width(), graphic.height(), PixelFormat::RGBA32).map_err(|_| ())?,
    )
    .map_err(|_| ())?;
    graphic.render_onto(&mut canvas, (0, 0)).map_err(|_| ())?;
    let texture = texture_creator
        .create_texture_from_surface(canvas.into_surface())
        .map_err(|_| ())?;
    Ok(texture)
}

pub trait CacheableScreenGraphic<HashType: Hash + Eq>: ScreenTexture {
    fn hash_props(&self) -> HashType;

    fn cache<'tc, U>(
        &self,
        texture_creator: &'tc render::TextureCreator<U>,
        texture_cache: &mut TextureCache<'tc, HashType>,
    ) -> Result<(), ()> {
        let hash = self.hash_props();
        let texture = self.generate_texture(texture_creator)?;
        texture_cache.insert(hash, texture);
        Ok(())
    }

    fn cached_render_onto<'tc, T: RenderTarget, U>(
        &self,
        canvas: &mut Canvas<T>,
        texture_creator: &'tc render::TextureCreator<U>,
        texture_cache: &mut TextureCache<'tc, HashType>,
        position: (i32, i32),
    ) -> Result<(), ()> {
        let hash = self.hash_props();
        if let Some(texture) = texture_cache.get_cached(&hash) {
            let query = texture.query();
            canvas
                .copy(
                    &texture,
                    None,
                    Some(render::FRect::new(
                        position.0 as f32,
                        position.1 as f32,
                        query.width as f32,
                        query.height as f32,
                    )),
                )
                .map_err(|_| ())?;
            return Ok(());
        }

        self.cache(texture_creator, texture_cache)?;
        if let Some(texture) = texture_cache.get_cached(&hash) {
            let query = texture.query();
            canvas
                .copy(
                    &texture,
                    None,
                    Some(render::FRect::new(
                        position.0 as f32,
                        position.1 as f32,
                        query.width as f32,
                        query.height as f32,
                    )),
                )
                .map_err(|_| ())?;
        }
        Ok(())
    }
}

pub struct CircleGUI {
    radius: f32,
    color: Color,
}

impl ScreenImmGraphic for CircleGUI {
    fn render_onto<T: RenderTarget>(
        &self,
        canvas: &mut Canvas<T>,
        top_left_pos: (i32, i32),
    ) -> Result<(), sdl3::Error> {
        let (x, y) = top_left_pos;
        let radius = self.radius;
        for i in 0..36 {
            let theta1: f32 = (i as f32) * 2.0 * std::f32::consts::PI / 36.;
            let theta2: f32 = ((i + 1) as f32) * 2.0 * std::f32::consts::PI / 36.;
            let x1: f32 = radius * theta1.cos() + radius;
            let y1: f32 = radius * theta1.sin() + radius;
            let x2: f32 = radius * theta2.cos() + radius;
            let y2: f32 = radius * theta2.sin() + radius;
            canvas.draw_line(
                (x + x1 as i32, y + y1 as i32),
                (x + x2 as i32, y + y2 as i32),
            )?;
        }
        Ok(())
    }
}

impl ScreenTexture for CircleGUI {
    fn generate_texture<'tc, U>(
        &self,
        texture_creator: &'tc render::TextureCreator<U>,
    ) -> Result<render::Texture<'tc>, ()> {
        derive_texture(self, texture_creator)
    }
}

impl SizedScreenGraphic for CircleGUI {
    fn width(&self) -> u32 {
        self.radius.ceil() as u32 * 2
    }

    fn height(&self) -> u32 {
        self.radius.ceil() as u32 * 2
    }
}

impl CacheableScreenGraphic<HashableGraphicProps> for CircleGUI {
    fn hash_props(&self) -> HashableGraphicProps {
        HashableGraphicProps::Circle {
            radius: Hashf32::new(self.radius),
            color: self.color,
        }
    }
}

pub struct DiscGUI {
    radius: f32,
    color: Color,
}

impl ScreenImmGraphic for DiscGUI {
    fn render_onto<T: RenderTarget>(
        &self,
        canvas: &mut Canvas<T>,
        top_left_pos: (i32, i32),
    ) -> Result<(), sdl3::Error> {
        let (x, y) = top_left_pos;
        let radius = self.radius;
        for i in -(radius as i16)..=(radius as i16) {
            let y_offset = (radius * radius - (i as f32) * (i as f32)).sqrt() as i16;
            for j in -y_offset..=y_offset {
                canvas.draw_point((x + radius as i32 + i as i32, y + radius as i32 + j as i32))?;
            }
        }
        Ok(())
    }
}

impl SizedScreenGraphic for DiscGUI {
    fn width(&self) -> u32 {
        self.radius.ceil() as u32 * 2
    }

    fn height(&self) -> u32 {
        self.radius.ceil() as u32 * 2
    }
}

impl ScreenTexture for DiscGUI {
    fn generate_texture<'tc, U>(
        &self,
        texture_creator: &'tc render::TextureCreator<U>,
    ) -> Result<render::Texture<'tc>, ()> {
        derive_texture(self, texture_creator)
    }
}

impl CacheableScreenGraphic<HashableGraphicProps> for DiscGUI {
    fn hash_props(&self) -> HashableGraphicProps {
        HashableGraphicProps::Disc {
            radius: Hashf32::new(self.radius),
            color: self.color,
        }
    }
}

pub struct TextFont<'tc> {
    font: ttf::Font<'tc>,
    ttf_name: String,
}

impl<'tc> TextFont<'tc> {
    pub fn new(font: ttf::Font<'tc>, font_name: &str) -> Self {
        Self {
            font,
            ttf_name: font_name.to_string(),
        }
    }
}

impl PartialEq for TextFont<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.ttf_name == other.ttf_name
    }
}

impl Eq for TextFont<'_> {}

impl std::hash::Hash for TextFont<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ttf_name.hash(state);
    }
}

pub struct TextGUI<'ft, 'tcontext> {
    text: String,
    font: &'ft TextFont<'tcontext>,
    color: Color,
}

impl<'ft, 'tcontext> TextGUI<'ft, 'tcontext> {
    pub fn new(text: &str, font: &'ft TextFont<'tcontext>, color: Color) -> Self {
        Self {
            text: text.to_string(),
            font,
            color,
        }
    }
}

impl<'ft, 'tcontext> ScreenTexture for TextGUI<'ft, 'tcontext> {
    fn generate_texture<'tc, U>(
        &self,
        texture_creator: &'tc render::TextureCreator<U>,
    ) -> Result<render::Texture<'tc>, ()> {
        let surface = self
            .font
            .font
            .render(&self.text)
            .blended(self.color)
            .map_err(|_| ())?;
        let texture = texture_creator
            .create_texture_from_surface(&surface)
            .map_err(|_| ())?;
        Ok(texture)
    }
}

impl<'ft, 'tc> CacheableScreenGraphic<HashableGraphicProps> for TextGUI<'ft, 'tc> {
    fn hash_props(&self) -> HashableGraphicProps {
        HashableGraphicProps::Text {
            text: self.text.clone(),
            font_name: self.font.ttf_name.clone(),
            color: self.color,
        }
    }
}

// implement sized for practicality
impl<'ft, 'tcontext> SizedScreenGraphic for TextGUI<'ft, 'tcontext> {
    fn width(&self) -> u32 {
        let surface = self
            .font
            .font
            .render(&self.text)
            .blended(self.color)
            .unwrap();
        surface.width()
    }

    fn height(&self) -> u32 {
        let surface = self
            .font
            .font
            .render(&self.text)
            .blended(self.color)
            .unwrap();
        surface.height()
    }
}
