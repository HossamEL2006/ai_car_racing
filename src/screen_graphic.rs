use crate::texture_cache::{HashableGraphicProps, Hashf32, TextureCache};
use sdl3::{
    pixels::{Color, PixelFormat},
    render::{self, Canvas, FPoint, RenderTarget},
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
        top_left_pos: (i32, i32),
    ) -> Result<(), ()> {
        let hash = self.hash_props();
        if let Some(texture) = texture_cache.get_cached(&hash) {
            let query = texture.query();
            canvas
                .copy(
                    &texture,
                    None,
                    Some(render::FRect::new(
                        top_left_pos.0 as f32,
                        top_left_pos.1 as f32,
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
                        top_left_pos.0 as f32,
                        top_left_pos.1 as f32,
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

impl CircleGUI {
    pub fn new(radius: f32, color: Color) -> Self {
        Self { radius, color }
    }
}

impl ScreenImmGraphic for CircleGUI {
    fn render_onto<T: RenderTarget>(
        &self,
        canvas: &mut Canvas<T>,
        top_left_pos: (i32, i32),
    ) -> Result<(), sdl3::Error> {
        canvas.set_draw_color(self.color);
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

impl DiscGUI {
    pub fn new(radius: f32, color: Color) -> Self {
        Self { radius, color }
    }
}

impl ScreenImmGraphic for DiscGUI {
    fn render_onto<T: RenderTarget>(
        &self,
        canvas: &mut Canvas<T>,
        top_left_pos: (i32, i32),
    ) -> Result<(), sdl3::Error> {
        canvas.set_draw_color(self.color);
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

pub struct PolygonGUI {
    vertices: Vec<FPoint>,
    color: Color,
    filled: bool,
}

impl PolygonGUI {
    pub fn new(vertices: Vec<FPoint>, color: Color, filled: bool) -> Self {
        Self {
            vertices,
            color,
            filled,
        }
    }
}

impl ScreenImmGraphic for PolygonGUI {
    fn render_onto<T: RenderTarget>(
        &self,
        canvas: &mut Canvas<T>,
        top_left_pos: (i32, i32),
    ) -> Result<(), sdl3::Error> {
        if !self.filled {
            let mut cyclic_shifted_fpoints = self
                .vertices
                .iter()
                .map(|v| render::FPoint {
                    x: v.x + top_left_pos.0 as f32,
                    y: v.y + top_left_pos.1 as f32,
                })
                .collect::<Vec<_>>();
            cyclic_shifted_fpoints.push(render::FPoint {
                x: self.vertices[0].x + top_left_pos.0 as f32,
                y: self.vertices[0].y + top_left_pos.1 as f32,
            });

            canvas.draw_lines(cyclic_shifted_fpoints.as_slice())
        } else {
            canvas.set_draw_color(self.color);

            let shifted_fpoints: Vec<render::FPoint> = self
                .vertices
                .iter()
                .map(|v| render::FPoint {
                    x: v.x + top_left_pos.0 as f32,
                    y: v.y + top_left_pos.1 as f32,
                })
                .collect::<Vec<_>>();

            if shifted_fpoints.len() < 3 {
                return Ok(());
            }

            // Get canvas size for clipping bounds
            let (canvas_w, canvas_h) = canvas.output_size()?;

            // Find vertical bounds clamped to screen dimensions
            let y_min = shifted_fpoints
                .iter()
                .map(|p| p.y)
                .fold(f32::MAX, f32::min)
                .max(0.0) as i32;
            let y_max = shifted_fpoints
                .iter()
                .map(|p| p.y)
                .fold(f32::MIN, f32::max)
                .min(canvas_h as f32 - 1.0) as i32;

            if y_min > y_max {
                return Ok(());
            }

            let n = shifted_fpoints.len();

            // Process row by row (scanline)
            for y in y_min..=y_max {
                let y_f = y as f32;
                let mut intersections = Vec::new();

                // Find all edge intersections with the current scanline y
                let mut j = n - 1;
                for i in 0..n {
                    let p1 = shifted_fpoints[j];
                    let p2 = shifted_fpoints[i];

                    // Check if edge crosses the scanline y (half-open interval rule avoids duplication)
                    if (p1.y <= y_f && p2.y > y_f) || (p2.y <= y_f && p1.y > y_f) {
                        let x = p1.x + (y_f - p1.y) * (p2.x - p1.x) / (p2.y - p1.y);
                        intersections.push(x);
                    }
                    j = i;
                }

                // Sort intersections from left to right
                intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

                // Draw horizontal spans between pairs of intersections (even-odd rule)
                let mut i = 0;
                while i + 1 < intersections.len() {
                    let x_start = intersections[i].max(0.0) as i32;
                    let x_end = intersections[i + 1].min(canvas_w as f32 - 1.0) as i32;

                    if x_start <= x_end {
                        canvas.draw_line(
                            render::FPoint::new(x_start as f32, y_f),
                            render::FPoint::new(x_end as f32, y_f),
                        )?;
                    }
                    i += 2;
                }
            }

            Ok(())
        }
    }
}

impl SizedScreenGraphic for PolygonGUI {
    fn width(&self) -> u32 {
        self.vertices.iter().map(|v| v.x).fold(f32::MIN, f32::max) as u32
    }

    fn height(&self) -> u32 {
        self.vertices.iter().map(|v| v.y).fold(f32::MIN, f32::max) as u32
    }
}

impl ScreenTexture for PolygonGUI {
    fn generate_texture<'tc, U>(
        &self,
        texture_creator: &'tc render::TextureCreator<U>,
    ) -> Result<render::Texture<'tc>, ()> {
        derive_texture(self, texture_creator)
    }
}

impl CacheableScreenGraphic<String> for PolygonGUI {
    fn hash_props(&self) -> String {
        format!("{:?}", self.vertices)
    }
}
