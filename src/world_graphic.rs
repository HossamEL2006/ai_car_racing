use std::hash::Hash;

use crate::texture_cache::{HashableGraphicProps, Hashf32, TextureCache};
use crate::world_objects::{Camera, Meter, WorldCoords, WorldPolygon, WorldVector};
use sdl3::{
    pixels::Color,
    rect,
    render::{Canvas, RenderTarget, Texture, TextureCreator},
    surface::Surface,
};

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

        let camera_pos = self.anchor()
            + 0.5
                * (WorldCoords::new(self.right_width(), self.top_height())
                    - WorldCoords::new(self.width(), self.height()));

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
            .create_texture_from_surface(canvas.surface())
            .map_err(|_| ())?;

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
        if let Some(texture) = texture_cache.get_cached(&hash) {
            // TODO : Copy texture into the correct position based on the camera
            canvas.copy(&texture, None, None).map_err(|_| ())?;
        } else {
            self.cache(camera.px_per_meter(), texture_creator, texture_cache)?;
            let texture = texture_cache.get_cached(&hash).ok_or(())?;
            canvas.copy(&texture, None, None).map_err(|_| ())?;
        }
        Ok(())
    }
}

// TODO: Design a way to merge between the cacheable and quick version of different graphics
//       Consider seperating the Graphic trait and the Cacheable trait
//       Consider automating anchor definition (some system that will infer the anchor's position)
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
        // TODO : Implement polygon rendering
        Ok(())
    }
}

impl SizedWorldGraphic for WorldPolygonGraphic {
    fn left_width(&self) -> Meter {
        Meter::new(self.polygon.min_x().value().abs())
    }
    fn right_width(&self) -> Meter {
        Meter::new(self.polygon.max_x().value().abs())
    }
    fn top_height(&self) -> Meter {
        Meter::new(self.polygon.max_y().value().abs())
    }
    fn bottom_height(&self) -> Meter {
        Meter::new(self.polygon.min_y().value().abs())
    }
    fn anchor(&self) -> WorldCoords {
        WorldCoords::origin()
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

// impl WorldGraphic for WorldGPolygon {
//     fn render_onto(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), sdl3::Error> {
//         if !self.filled {
//             let mut fpoints: Vec<render::FPoint> = self
//                 .polygon
//                 .vertices
//                 .iter()
//                 .map(|coord: &WorldCoords| coord.as_fpoint(camera))
//                 .collect();
//             fpoints.push(self.polygon.vertices[0].as_fpoint(camera));
//             canvas.draw_lines(fpoints.as_slice())
//         } else {
//             canvas.set_draw_color(self.color);

//             let fpoints: Vec<render::FPoint> = self
//                 .polygon
//                 .vertices
//                 .iter()
//                 .map(|coord| coord.as_fpoint(camera))
//                 .collect();

//             if fpoints.len() < 3 {
//                 return Ok(());
//             }

//             // Get canvas size for clipping bounds
//             let (canvas_w, canvas_h) = canvas.output_size()?;

//             // Find vertical bounds clamped to screen dimensions
//             let y_min = fpoints
//                 .iter()
//                 .map(|p| p.y)
//                 .fold(f32::MAX, f32::min)
//                 .max(0.0) as i32;
//             let y_max = fpoints
//                 .iter()
//                 .map(|p| p.y)
//                 .fold(f32::MIN, f32::max)
//                 .min(canvas_h as f32 - 1.0) as i32;

//             if y_min > y_max {
//                 return Ok(());
//             }

//             let n = fpoints.len();

//             // Process row by row (scanline)
//             for y in y_min..=y_max {
//                 let y_f = y as f32;
//                 let mut intersections = Vec::new();

//                 // Find all edge intersections with the current scanline y
//                 let mut j = n - 1;
//                 for i in 0..n {
//                     let p1 = fpoints[j];
//                     let p2 = fpoints[i];

//                     // Check if edge crosses the scanline y (half-open interval rule avoids duplication)
//                     if (p1.y <= y_f && p2.y > y_f) || (p2.y <= y_f && p1.y > y_f) {
//                         let x = p1.x + (y_f - p1.y) * (p2.x - p1.x) / (p2.y - p1.y);
//                         intersections.push(x);
//                     }
//                     j = i;
//                 }

//                 // Sort intersections from left to right
//                 intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

//                 // Draw horizontal spans between pairs of intersections (even-odd rule)
//                 let mut i = 0;
//                 while i + 1 < intersections.len() {
//                     let x_start = intersections[i].max(0.0) as i32;
//                     let x_end = intersections[i + 1].min(canvas_w as f32 - 1.0) as i32;

//                     if x_start <= x_end {
//                         canvas.draw_line(
//                             render::FPoint::new(x_start as f32, y_f),
//                             render::FPoint::new(x_end as f32, y_f),
//                         )?;
//                     }
//                     i += 2;
//                 }
//             }

//             Ok(())
//         }
//     }
// }
