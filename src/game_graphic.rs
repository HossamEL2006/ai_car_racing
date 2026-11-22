use crate::game_window_config::MOUVEMENT_DEBUG;
use crate::{
    game::{Car, Game, Path},
    screen_graphic::{CacheableScreenGraphic, TextFont, TextGUI},
    texture_cache::{HashableGraphicProps, TextureCache},
    world_graphic::{CacheableWorldGraphic, WorldGraphic},
    world_objects::{Camera, Meter, WorldCoords, WorldPolygon},
};
use sdl3::{
    pixels::Color,
    render::{self, Canvas, RenderTarget},
};

// TODO: render a speedometer for the car

impl Path {
    fn render<'tc, T: RenderTarget, U>(
        &self,
        camera: &Camera,
        canvas: &mut Canvas<T>,
        texture_creator: &'tc render::TextureCreator<U>,
        texture_cache: &mut TextureCache<'tc, HashableGraphicProps>,
    ) -> Result<(), ()> {
        WorldPolygon::new(&self.coords)
            .as_graphic(false, Color::RGB(255, 255, 255))
            .cached_render_onto(camera, canvas, texture_creator, texture_cache)
    }

    fn render_debug_anchors(
        &self,
        _camera: &Camera,
        _canvas: &mut Canvas<impl RenderTarget>,
    ) -> Result<(), sdl3::Error> {
        todo!() // TODO: Reimplement circuit generation debugging
        // for [right, center, left] in self.anchor_triplets.iter() {
        //     canvas.set_draw_color(Color::RGB(0, 255, 0));
        //     canvas
        //         .draw_line(center.as_point(camera), left.as_point(camera))
        //         .unwrap();
        //     left.render(camera, canvas).unwrap();

        //     canvas.set_draw_color(Color::RGB(255, 255, 255));
        //     center.render(camera, canvas).unwrap();

        //     canvas.set_draw_color(Color::RGB(0, 0, 255));
        //     canvas
        //         .draw_line(center.as_point(camera), right.as_point(camera))
        //         .unwrap();
        //     right.render(camera, canvas).unwrap();
        // }

        // // draw lines from the center of the track to main anchor points
        // for [_, center, _] in self.anchor_triplets.iter() {
        //     canvas.set_draw_color(Color::RGB(255, 255, 0));
        //     canvas
        //         .draw_line(center.as_point(camera), self.config.center.as_point(camera))
        //         .unwrap();
        // }
        // Ok(())
    }
}

impl Car {
    fn render_rect(
        &self,
        camera: &Camera,
        canvas: &mut Canvas<impl RenderTarget>,
    ) -> Result<(), sdl3::Error> {
        let polygon = self.as_rect_polygon();
        polygon
            .as_graphic(false, Color::RGB(255, 0, 0))
            .render_onto(camera, canvas)
    }

    pub(crate) fn pretty_render(
        &self,
        camera: &Camera,
        canvas: &mut Canvas<impl RenderTarget>,
    ) -> Result<(), sdl3::Error> {
        // TODO: Implement CacheableGraphic for car with flexible hash values to allow fast
        //       rendering without memory bloat.

        for (mut polygon, color) in self.car_polys.clone() {
            for vertex in polygon.vertices_mut() {
                // rotate around origin to match car's direction vector
                let angle_to_origin = self.direction_vec.angle();
                let angle_to_vertical_pos = angle_to_origin - std::f32::consts::FRAC_PI_2;
                let cos_angle = angle_to_vertical_pos.cos();
                let sin_angle = angle_to_vertical_pos.sin();
                let rotated_x = sin_angle * vertex.y().value() - cos_angle * vertex.x().value();
                let rotated_y = sin_angle * vertex.x().value() + cos_angle * vertex.y().value();
                *vertex = WorldCoords::new(Meter::new(rotated_x), Meter::new(-rotated_y));
                *vertex = self.coords + (*vertex - WorldCoords::origin());
            }
            canvas.set_draw_color(color);
            polygon
                .as_graphic(true, color)
                .render_onto(camera, canvas)?;
        }
        Ok(())
    }

    fn render_debug_vectors<'tc, U>(
        &self,
        camera: &Camera,
        canvas: &mut Canvas<impl RenderTarget>,
        font: &TextFont,
        texture_creator: &'tc render::TextureCreator<U>,
        texture_cache: &mut TextureCache<'tc, HashableGraphicProps>,
    ) -> Result<(), ()> {
        // fun fact :
        // Implementing a text caching system made this function around 40 times faster
        let debug_scalar: f32 = 0.3;
        let base_point: WorldCoords = self.coords;
        let friction_acc = -self.curr_friction_factor * self.velocity_vector;
        let vectors = [
            (
                (debug_scalar * self.velocity_vector),
                Color::RGB(255, 0, 0),
                "v",
            ),
            (
                (debug_scalar.powi(2) * self.engine_acc_vector),
                Color::RGB(0, 0, 255),
                "p",
            ),
            (
                (debug_scalar.powi(2) * friction_acc),
                Color::RGB(0, 255, 0),
                "f",
            ),
            (
                (debug_scalar.powi(2) * (self.engine_acc_vector + friction_acc)),
                Color::RGB(255, 255, 0),
                "a",
            ),
        ];

        for (vector, color, label) in vectors {
            let gvector = vector.as_graphic(base_point, color);
            let tip_point = (base_point + vector).as_point(camera);
            let tip_x = tip_point.x;
            let tip_y = tip_point.y;
            let vector_length_px = vector.norm().as_px_dist(camera);
            if vector_length_px > 10. {
                TextGUI::new(label, font, Color::RGB(255, 255, 255)).cached_render_onto(
                    canvas,
                    texture_creator,
                    texture_cache,
                    (tip_x, tip_y - 20),
                )?;
            }
            if vector_length_px > 2. {
                gvector.render_onto(camera, canvas).map_err(|_| ())?;
            }
        }
        Ok(())
    }
}

impl Game {
    pub(crate) fn render_onto<'tc, U>(
        &self,
        camera: &Camera,
        canvas: &mut Canvas<impl RenderTarget>,
        texture_creator: &'tc render::TextureCreator<U>,
        texture_cache: &mut TextureCache<'tc, HashableGraphicProps>,
        text_font: &TextFont,
    ) -> bool {
        let mut failed_rendering = false;

        canvas.set_draw_color(Color::RGB(25, 50, 25));
        canvas.clear();

        // Render the track's inner border
        failed_rendering |= self
            .track()
            .inner_border()
            .as_graphic(true, Color::RGB(10, 70, 10))
            .cached_render_onto(camera, canvas, texture_creator, texture_cache)
            .map_err(|e| eprintln!("Failed to render the enclosed area: {:?}", e))
            .is_err();

        // Render the circuit polygon
        failed_rendering |= self
            .track()
            .circuit_polygon()
            .as_graphic(true, Color::RGB(80, 80, 80))
            .cached_render_onto(camera, canvas, texture_creator, texture_cache)
            .map_err(|e| eprintln!("Failed to render the circuit polygon: {:?}", e))
            .is_err();

        // if CIRCUIT_DEBUG {
        //     failed_rendering |= self
        //         .track()
        //         .bezier_path()
        //         .render(camera, canvas, texture_creator, texture_cache)
        //         .map_err(|e| eprintln!("Failed to render the path: {:?}", e))
        //         .is_err();
        //     failed_rendering |= self
        //         .track()
        //         .bezier_path()
        //         .render_debug_anchors(camera, canvas)
        //         .map_err(|e| eprintln!("Failed to render the path debug anchors: {:?}", e))
        //         .is_err();
        // }

        for player in self.players() {
            failed_rendering |= player
                .car()
                .pretty_render(camera, canvas)
                .map_err(|e| eprintln!("Failed to render the car: {:?}", e))
                .is_err();
        }

        if MOUVEMENT_DEBUG {
            for player in self.players() {
                failed_rendering |= player
                    .car()
                    .render_debug_vectors(camera, canvas, text_font, texture_creator, texture_cache)
                    .map_err(|e| eprintln!("Failed to render the car debug info: {:?}", e))
                    .is_err();
            }
        }

        failed_rendering
    }
}
