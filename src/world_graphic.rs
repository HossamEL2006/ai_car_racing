use crate::screen_graphic::{PolygonGUI, ScreenImmGraphic};
use crate::texture_cache::{HashableGraphicProps, Hashf32, TextureCache};
use crate::world_objects::{Camera, Meter, WorldCoords, WorldPolygon, WorldVector};
use sdl3::render::{FPoint, FRect};
use sdl3::{
    pixels::Color,
    rect,
    render::{Canvas, RenderTarget, Texture, TextureCreator},
    surface::Surface,
};
use std::hash::Hash;

pub trait WorldGraphic {
    // this trait is for world objects (depending on the camera and unit distance `Meter`) that
    // should be rendered quickly without creating a seperate texture
    // (since they won't be cached anyways)
    fn render_onto<T: RenderTarget>(
        &self,
        camera: &Camera,
        canvas: &mut Canvas<T>,
    ) -> Result<(), sdl3::Error>;
}

pub trait SizedWorldGraphic: WorldGraphic {
    fn left_width(&self) -> Meter;
    fn right_width(&self) -> Meter;
    fn top_height(&self) -> Meter;
    fn bottom_height(&self) -> Meter;
    fn anchor(&self) -> WorldCoords;

    fn width(&self) -> Meter {
        self.left_width() + self.right_width()
    }

    fn height(&self) -> Meter {
        self.top_height() + self.bottom_height()
    }
}

pub trait CacheableWorldGraphic<HashType: Hash + Eq>: SizedWorldGraphic {
    // TODO: Consider potentially giving the entier camera object to the hash_props function to
    //       account for all potential camera-dependent properties
    fn hash_props(&self, px_per_meter: u32) -> HashType;

    fn generate_texture<'tc, T>(
        &self,
        camera_px_per_meter: u32,
        texture_creator: &'tc TextureCreator<T>,
    ) -> Result<Texture<'tc>, ()> {
        let mut canvas = Canvas::from_surface(
            Surface::new(
                (camera_px_per_meter as f32 * self.width().value()).round() as u32,
                (camera_px_per_meter as f32 * self.height().value()).round() as u32,
                sdl3::pixels::PixelFormat::RGBA32,
            )
            .map_err(|_| ())?,
        )
        .map_err(|_| ())?;

        // center the camera on the canvas
        let camera_pos = self.anchor()
            + WorldVector::new(
                0.5 * self.width() - self.left_width(),
                -(0.5 * self.height() - self.top_height()),
            );

        self.render_onto(
            &Camera::new(
                (camera_px_per_meter as f32 * self.width().value()).round() as u32,
                (camera_px_per_meter as f32 * self.height().value()).round() as u32,
                camera_pos,
                camera_px_per_meter,
            ),
            &mut canvas,
        )
        .map_err(|_| ())?;

        let texture = texture_creator
            .create_texture_from_surface(canvas.into_surface())
            .map_err(|_| ())?;
        // 3 hours of debugging to figure out that it should have been `into_surface()` instead
        // of `surface()`
        // TODO : Figure out the difference between `into_surface()` and `surface()`

        Ok(texture)
    }

    fn cache<'tc, U>(
        &self,
        camera_px_per_meter: u32,
        texture_creator: &'tc TextureCreator<U>,
        texture_cache: &mut TextureCache<'tc, HashType>,
    ) -> Result<(), ()> {
        let texture = self.generate_texture(camera_px_per_meter, texture_creator)?;
        texture_cache.insert(self.hash_props(camera_px_per_meter), texture);
        Ok(())
    }

    fn cached_render_onto<'tc, T: RenderTarget, U>(
        &self,
        camera: &Camera,
        canvas: &mut Canvas<T>,
        texture_creator: &'tc TextureCreator<U>,
        texture_cache: &mut TextureCache<'tc, HashType>,
    ) -> Result<(), ()> {
        let hash = self.hash_props(camera.px_per_meter());
        let top_left_fpoint = (WorldCoords::new(
            self.anchor().x() - self.left_width(),
            self.anchor().y() - self.top_height(),
        ))
        .as_fpoint(camera);
        if let Some(texture) = texture_cache.get_cached(&hash) {
            let query = texture.query();
            canvas
                .copy(
                    &texture,
                    None,
                    Some(FRect::new(
                        top_left_fpoint.x,
                        top_left_fpoint.y,
                        query.width as f32,
                        query.height as f32,
                    )),
                )
                .map_err(|_| ())?;
        } else {
            self.cache(camera.px_per_meter(), texture_creator, texture_cache)?;
            let texture = texture_cache.get_cached(&hash).ok_or(())?;
            let query = texture.query();
            canvas
                .copy(
                    &texture,
                    None,
                    Some(FRect::new(
                        top_left_fpoint.x,
                        top_left_fpoint.y,
                        query.width as f32,
                        query.height as f32,
                    )),
                )
                .map_err(|_| ())?;
        }
        Ok(())
    }
}

// TODO: Design a way to merge between the cacheable and quick version of different graphics
//       Make the anchor system for world graphics more intuitive
pub struct WorldVectorGraphic {
    pub(crate) base: WorldCoords,
    pub(crate) vector: WorldVector,
    pub(crate) color: Color,
}

impl WorldGraphic for WorldVectorGraphic {
    fn render_onto<T: RenderTarget>(
        &self,
        camera: &Camera,
        canvas: &mut Canvas<T>,
    ) -> Result<(), sdl3::Error> {
        canvas.set_draw_color(self.color);
        let start: rect::Point = (self.base).as_point(camera);
        let end: rect::Point = (self.base + self.vector).as_point(camera);
        canvas.draw_line(start, end)?;

        let arrow_tip: rect::Point =
            (self.base + self.vector + 0.2 * self.vector.rotated(150.0f32.to_radians()))
                .as_point(camera);
        canvas.draw_line(end, arrow_tip)?;
        let arrow_tip2: rect::Point =
            (self.base + self.vector + 0.2 * self.vector.rotated(210.0f32.to_radians()))
                .as_point(camera);
        canvas.draw_line(end, arrow_tip2)?;

        Ok(())
    }
}

impl SizedWorldGraphic for WorldVectorGraphic {
    fn left_width(&self) -> Meter {
        self.vector.norm()
    }
    fn right_width(&self) -> Meter {
        self.vector.norm()
    }
    fn top_height(&self) -> Meter {
        self.vector.norm()
    }
    fn bottom_height(&self) -> Meter {
        self.vector.norm()
    }
    fn anchor(&self) -> WorldCoords {
        self.base
    }
}

impl CacheableWorldGraphic<HashableGraphicProps> for WorldVectorGraphic {
    fn hash_props(&self, px_per_meter: u32) -> HashableGraphicProps {
        HashableGraphicProps::WorldVector {
            x: Hashf32::new(self.vector.x().value()),
            y: Hashf32::new(self.vector.y().value()),
            px_per_meter,
            color: Some(self.color),
            label: None,
        }
    }
}

pub struct WorldPolygonGraphic {
    pub(crate) polygon: WorldPolygon,
    pub(crate) filled: bool,
    pub(crate) color: Color,
}

impl WorldGraphic for WorldPolygonGraphic {
    fn render_onto<T: RenderTarget>(
        &self,
        camera: &Camera,
        canvas: &mut Canvas<T>,
    ) -> Result<(), sdl3::Error> {
        canvas.set_draw_color(self.color);

        let top_left_point =
            WorldCoords::new(self.polygon.min_x(), self.polygon.max_y()).as_fpoint(camera);
        let fpoints = self
            .polygon
            .vertices()
            .iter()
            .map(|coord| coord.as_fpoint(camera))
            .map(|fpoint| FPoint::new(fpoint.x - top_left_point.x, fpoint.y - top_left_point.y))
            .collect::<Vec<_>>();

        let polygon_gui = PolygonGUI::new(fpoints, self.color, self.filled);
        polygon_gui.render_onto(canvas, (top_left_point.x as i32, top_left_point.y as i32))?;

        Ok(())
    }
}

impl SizedWorldGraphic for WorldPolygonGraphic {
    fn left_width(&self) -> Meter {
        Meter::new(0.)
    }

    fn right_width(&self) -> Meter {
        self.polygon.width()
    }

    fn top_height(&self) -> Meter {
        Meter::new(0.)
    }

    fn bottom_height(&self) -> Meter {
        self.polygon.height()
    }

    fn anchor(&self) -> WorldCoords {
        WorldCoords::new(self.polygon.min_x(), self.polygon.max_y())
    }
}

impl CacheableWorldGraphic<HashableGraphicProps> for WorldPolygonGraphic {
    fn hash_props(&self, px_per_meter: u32) -> HashableGraphicProps {
        HashableGraphicProps::WorldPolygon {
            polygon: self.polygon.clone(),
            // TODO : Consider hashing the vertices instead of cloning the whole polygon
            color: self.color,
            px_per_meter,
            filled: self.filled,
        }
    }
}
