mod screen_graphic;
mod texture_cache;
mod world_graphic;
mod world_objects;

use core::f32;
use rand::{SeedableRng, rngs::StdRng};
use rand_distr::{Distribution, Normal};
use sdl3::{
    event::Event,
    joystick::Joystick,
    keyboard::{KeyboardState, Keycode, Scancode},
    pixels::Color,
    render::{self, Canvas, RenderTarget},
    ttf::{self, Text},
    video::{Window, WindowContext},
};
use std::{
    collections::VecDeque,
    iter,
    time::{self, Duration},
};
use svg::{node::element::tag, parser};
use world_objects::*;

use crate::{
    screen_graphic::{CacheableScreenGraphic, SizedScreenGraphic, TextFont, TextGUI},
    texture_cache::{HashableGraphicProps, TextureCache},
    world_graphic::{CacheableWorldGraphic, WorldGraphic},
};

struct Car {
    coords: WorldCoords,
    direction_vec: WorldVector,
    length: Meter,
    width: Meter,
    engine_acc: WorldVector,
    velocity: WorldVector,
    friction_factor: f32,
    car_polys: Vec<(WorldPolygon, Color)>,
}

impl Car {
    fn init(coords: WorldCoords, direction: WorldVector) -> Car {
        let mut car: Car = Car {
            coords,
            direction_vec: direction.normalized(),
            length: Meter::new(5.),
            width: Meter::new(2.),
            engine_acc: WorldVector::zero(),
            velocity: WorldVector::zero(),
            friction_factor: FRICTION_FACTOR,
            car_polys: Vec::new(),
        };
        car.car_polys = car.load_svg();
        car
    }

    fn corner_coords(&self) -> [WorldCoords; 4] {
        [(1., 1.), (-1., 1.), (-1., -1.), (1., -1.)].map(|(d1, d2)| {
            self.coords
                + (d1 * (self.length.value() / 2.)) * self.direction_vec
                + (d2 * (self.width.value() / 2.)) * self.direction_vec.orthogonal()
        })
    }

    fn as_rect_polygon(&self) -> WorldPolygon {
        WorldPolygon::new(self.corner_coords().as_slice())
    }

    fn load_svg(&self) -> Vec<(WorldPolygon, Color)> {
        let svg_path: &str = "assets/f1_car.svg";
        println!("Loading F1 car SVG from {}", svg_path);

        let mut polygons: Vec<(WorldPolygon, Color)> = Vec::new();

        let mut content = String::new();
        let svg_iterator = svg::open(svg_path, &mut content).unwrap();
        for event in svg_iterator {
            if let parser::Event::Tag(tag::Polygon, _, attributes) = event {
                let points = attributes.get("points").unwrap();
                // parse into an sdl3 RGB
                let color: Color = attributes
                    .get("fill")
                    .map(|s| parse_color(s).unwrap_or(Color::BLACK))
                    .unwrap_or(Color::BLACK);
                let polygon: WorldPolygon = WorldPolygon::new(
                    &points
                        .split_whitespace()
                        .map(|pair| {
                            let mut iter = pair.split(',');
                            (iter.next().unwrap(), iter.next().unwrap())
                        })
                        .filter_map(|(s1, s2)| {
                            let x = s1.parse::<f32>().ok()?;
                            let y = s2.parse::<f32>().ok()?;
                            Some(WorldCoords::new(
                                Meter::new(self.width.value() * x / 200. - self.width.value() / 2.),
                                Meter::new(
                                    self.length.value() * y / 500. - self.length.value() / 2.,
                                ),
                            ))
                        })
                        .collect::<Vec<WorldCoords>>(),
                );
                polygons.push((polygon, color));
            }
        }

        polygons
    }

    fn render_rect(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), sdl3::Error> {
        let polygon = self.as_rect_polygon();
        polygon
            .as_graphic(false, Color::RGB(255, 0, 0))
            .render_onto(camera, canvas)
    }

    fn pretty_render(
        &self,
        camera: &Camera,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), sdl3::Error> {
        // TODO: Implement CacheableGraphic for car with flexible hash values to allow fast
        //       rendering without memory bloat.

        for (mut polygon, color) in self.car_polys.clone() {
            for vertex in polygon.vertices_mut() {
                // rotate around origin to match car's direction vector
                let angle = self.direction_vec.angle();
                // TODO : Investigate of this stil works correctly since x.atan2(y) was replace
                //        with y.atan2(x)
                let cos_angle = angle.cos();
                let sin_angle = angle.sin();
                let rotated_x = cos_angle * vertex.x().value() - sin_angle * vertex.y().value();
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

    fn render_debug_vectors(
        &self,
        camera: &Camera,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), sdl3::Error> {
        // fun fact :
        // Implementing a text caching system made this function around 40 times faster

        let debug_scalar: f32 = 0.3;
        let base_point: WorldCoords = self.coords;
        let friction_acc = -self.friction_factor * self.velocity;
        let gvectors = [
            (
                (debug_scalar * self.velocity).into_graphic(base_point, Color::RGB(255, 0, 0)),
                "v",
            ),
            (
                (debug_scalar.powi(2) * self.engine_acc)
                    .into_graphic(base_point, Color::RGB(0, 0, 255)),
                "p",
            ),
            (
                (debug_scalar.powi(2) * friction_acc)
                    .into_graphic(base_point, Color::RGB(0, 255, 0)),
                "f",
            ),
            (
                (debug_scalar.powi(2) * (self.engine_acc + friction_acc))
                    .into_graphic(base_point, Color::RGB(255, 255, 0)),
                "a",
            ),
        ];

        for (gvector, label) in gvectors {
            // TODO: Implement label rendering
            gvector.render_onto(camera, canvas)?;
        }
        Ok(())
    }

    fn set_engine_acc(&mut self, acc: WorldVector) {
        self.engine_acc = acc
    }

    fn set_friction_factor(&mut self, friction_factor: f32) {
        self.friction_factor = friction_factor;
    }

    fn update_coords(&mut self, dt: f32) {
        let friction_acc = -self.friction_factor * self.velocity;
        self.velocity = self.velocity + dt * self.engine_acc + dt * friction_acc;
        let velocity_norm: f32 = self.velocity.norm().value();
        if velocity_norm > 1e-4 {
            self.direction_vec = (1. / velocity_norm) * self.velocity;
        }
        self.coords = self.coords + dt * self.velocity;
    }
}

#[derive(Clone)]
struct BezierPathConfig {
    center: WorldCoords,
    n_anchors: usize,
    mean_anchor_radius: f32,
    stdev_anchor_radius: f32,
    stdev_subanchor_angle_dev: f32,
    points_per_bezier_curve: usize,
}

impl BezierPathConfig {
    fn init_circular() -> Self {
        BezierPathConfig {
            center: WorldCoords::origin(),
            n_anchors: N_ANCHORS,
            mean_anchor_radius: MEAN_ANCHOR_RADIUS,
            stdev_anchor_radius: 0.,
            stdev_subanchor_angle_dev: 0.,
            points_per_bezier_curve: POINTS_PER_BEZIER_CURVE,
        }
    }
}

impl Default for BezierPathConfig {
    fn default() -> Self {
        BezierPathConfig {
            center: WorldCoords::origin(),
            n_anchors: N_ANCHORS,
            mean_anchor_radius: MEAN_ANCHOR_RADIUS,
            stdev_anchor_radius: STDEV_ANCHOR_RADIUS,
            stdev_subanchor_angle_dev: STDEV_SUBANCHOR_ANGLE_DEV,
            points_per_bezier_curve: POINTS_PER_BEZIER_CURVE,
        }
    }
}

struct BezierPath {
    coords: Vec<WorldCoords>,
    starting_point: WorldCoords,
    starting_direction: WorldVector,
    config: BezierPathConfig,               // for debugging
    anchor_triplets: Vec<[WorldCoords; 3]>, // for debugging
}

impl BezierPathConfig {
    fn generate_path(&self, seed: Option<u64>) -> BezierPath {
        fn sample_anchors(config: &BezierPathConfig, rng: &mut StdRng) -> Vec<WorldCoords> {
            let radius_normal_dist: Normal<f32> =
                Normal::new(config.mean_anchor_radius, config.stdev_anchor_radius).unwrap();
            let angle_dev_dist: Normal<f32> =
                Normal::new(0., (360. / config.n_anchors as f32) * 0.15).unwrap();

            (0..config.n_anchors)
                .map(|n: usize| {
                    let angle_dev: f32 = angle_dev_dist.sample(rng);
                    n as f32 * (360. / config.n_anchors as f32) + angle_dev - 90.
                    // -90 to start the first anchor at the bottom of the screen
                })
                .collect::<Vec<f32>>()
                // To avoid borrowing issues `&mut rng`, we collect into a Vec first
                .into_iter()
                .map(|angle: f32| {
                    let radius: f32 = radius_normal_dist
                        .sample(rng)
                        .max(config.mean_anchor_radius / 4.);
                    WorldCoords::new(
                        Meter::new(config.center.x().value() + radius * angle.to_radians().cos()),
                        Meter::new(config.center.y().value() + radius * angle.to_radians().sin()),
                    )
                })
                .collect()
        }

        fn sample_anchor_triplets(
            config: &BezierPathConfig,
            mut anchors: Vec<WorldCoords>,
            rng: &mut StdRng,
        ) -> Vec<[WorldCoords; 3]> {
            let mut anchor_triplets: Vec<[WorldCoords; 3]> = Vec::new();

            let angle_dev_dist: Normal<f32> =
                Normal::new(0., config.stdev_subanchor_angle_dev).unwrap();

            let mut cyclic_anchors: Vec<WorldCoords> = vec![*anchors.last().unwrap()];
            anchors.push(anchors[0]);
            cyclic_anchors.extend(anchors);

            for (prev_anchor, anchor, next_anchor) in cyclic_anchors
                .windows(3)
                .map(|window: &[WorldCoords]| (window[0], window[1], window[2]))
            {
                let angle: f32 = (anchor - config.center).angle() + f32::consts::FRAC_PI_2;
                let angle_dev: f32 = angle_dev_dist.sample(rng);
                let new_angle: f32 = angle + angle_dev.to_radians();

                let track_dir_vector: WorldVector =
                    WorldVector::new(Meter::new(new_angle.cos()), Meter::new(new_angle.sin()));

                let subanchor_dist_coef: f32 = 0.4;

                let right_subanchor_dist: f32 = (anchor - prev_anchor).norm().value();
                let right_subanchor: WorldCoords =
                    anchor + subanchor_dist_coef * (-right_subanchor_dist) * track_dir_vector;

                let left_subanchor_dist: f32 = (next_anchor - anchor).norm().value();
                let left_subanchor: WorldCoords =
                    anchor + subanchor_dist_coef * left_subanchor_dist * track_dir_vector;

                anchor_triplets.push([right_subanchor, anchor, left_subanchor]);
            }

            anchor_triplets
        }

        fn anchor_triplets_to_control_points(
            anchor_triplets: &[[WorldCoords; 3]],
        ) -> Vec<[WorldCoords; 4]> {
            anchor_triplets
                .windows(2)
                .chain(iter::once(
                    [
                        *anchor_triplets.last().unwrap(),
                        *anchor_triplets.first().unwrap(),
                    ]
                    .as_slice(),
                ))
                .map(|anch_pair: &[[WorldCoords; 3]]| {
                    let [_, center1, left1] = anch_pair[0];
                    let [right2, center2, _] = anch_pair[1];

                    [center1, left1, right2, center2]
                })
                .collect()
        }

        fn generate_bezier_curve(control_points: [WorldCoords; 4], n: usize) -> Vec<WorldCoords> {
            // the last point is not included to avoid duplication between consecutive curves
            (0..n)
                .map(|i: usize| {
                    let t: f32 = i as f32 / n as f32;
                    let one_minus_t: f32 = 1. - t;

                    let p0: WorldCoords = control_points[0];
                    let p1: WorldCoords = control_points[1];
                    let p2: WorldCoords = control_points[2];
                    let p3: WorldCoords = control_points[3];

                    let x: f32 = one_minus_t.powi(3) * p0.x().value()
                        + 3. * one_minus_t.powi(2) * t * p1.x().value()
                        + 3. * one_minus_t * t.powi(2) * p2.x().value()
                        + t.powi(3) * p3.x().value();

                    let y: f32 = one_minus_t.powi(3) * p0.y().value()
                        + 3. * one_minus_t.powi(2) * t * p1.y().value()
                        + 3. * one_minus_t * t.powi(2) * p2.y().value()
                        + t.powi(3) * p3.y().value();

                    WorldCoords::new(Meter::new(x), Meter::new(y))
                })
                .collect()
        }

        let mut coords: Vec<WorldCoords> = Vec::new();

        let mut rng: StdRng =
            StdRng::seed_from_u64(seed.unwrap_or(rand::random_range(0..u64::MAX)));

        let anchors: Vec<WorldCoords> = sample_anchors(self, &mut rng);

        let starting_point: WorldCoords = anchors[0];

        let anchor_triplets: Vec<[WorldCoords; 3]> =
            sample_anchor_triplets(self, anchors, &mut rng);

        let vec_control_points: Vec<[WorldCoords; 4]> =
            anchor_triplets_to_control_points(&anchor_triplets);

        vec_control_points
            .iter()
            .for_each(|control_points: &[WorldCoords; 4]| {
                coords.extend(generate_bezier_curve(
                    *control_points,
                    self.points_per_bezier_curve,
                ))
            });

        let starting_direction = anchor_triplets[0][2] - anchor_triplets[0][0];

        BezierPath {
            coords,
            starting_point,
            starting_direction,
            config: self.clone(),
            anchor_triplets,
        }
    }
}

impl BezierPath {
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
        camera: &Camera,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), sdl3::Error> {
        for [right, center, left] in self.anchor_triplets.iter() {
            canvas.set_draw_color(Color::RGB(0, 255, 0));
            canvas
                .draw_line(center.as_point(camera), left.as_point(camera))
                .unwrap();
            left.render(camera, canvas).unwrap();

            canvas.set_draw_color(Color::RGB(255, 255, 255));
            center.render(camera, canvas).unwrap();

            canvas.set_draw_color(Color::RGB(0, 0, 255));
            canvas
                .draw_line(center.as_point(camera), right.as_point(camera))
                .unwrap();
            right.render(camera, canvas).unwrap();
        }

        // draw lines from the center of the track to main anchor points
        for [_, center, _] in self.anchor_triplets.iter() {
            canvas.set_draw_color(Color::RGB(255, 255, 0));
            canvas
                .draw_line(center.as_point(camera), self.config.center.as_point(camera))
                .unwrap();
        }
        Ok(())
    }

    fn generate_path_borders(&self, path_width: f32) -> (Vec<WorldCoords>, Vec<WorldCoords>) {
        let half_width: f32 = path_width / 2.0;
        let mut outer_border: Vec<WorldCoords> = Vec::new();
        let mut inner_border: Vec<Option<WorldCoords>> = Vec::new();

        let cyclic_path_points: Vec<&WorldCoords> = std::iter::once(self.coords.last().unwrap())
            .chain(self.coords.iter())
            .chain(std::iter::once(self.coords.first().unwrap()))
            .collect();
        for (i, (prev, curr, next)) in cyclic_path_points
            .windows(3)
            .map(|w| (w[0], w[1], w[2]))
            .enumerate()
        {
            let direction: WorldVector = (*next - *prev).normalized();
            let orthogonal: WorldVector = direction.orthogonal();
            let outer_point: WorldCoords = *curr + (-half_width) * orthogonal;
            let mut inner_point: Option<WorldCoords> = Some(*curr + half_width * orthogonal);

            outer_border.push(outer_point);

            // the inner point is only included if it is not too close to other path points
            let points_per_curve: i32 = self.config.points_per_bezier_curve as i32;
            for j in (i as i32 - points_per_curve)..=(i as i32 + points_per_curve) {
                let idx: usize = j.rem_euclid(self.coords.len() as i32) as usize;
                let point: &WorldCoords = &self.coords[idx];
                if *point != *curr
                    && (*point - inner_point.unwrap()).norm().value() < half_width * 1.001
                {
                    inner_point = None;
                    break;
                }
            }
            inner_border.push(inner_point);
        }

        // replace nones with closest next point
        for i in 0..inner_border.len() {
            if inner_border[i].is_none() {
                let mut offset: usize = 1;
                while offset < inner_border.len() {
                    let next_idx: usize = (i + offset) % inner_border.len();
                    if let Some(point) = inner_border[next_idx] {
                        inner_border[i] = Some(point);
                        break;
                    }
                    offset += 1;
                }
            }
        }

        (
            outer_border,
            inner_border.iter().map(|p| p.unwrap()).collect(),
        )
    }
}

fn parse_color(color_str: &str) -> Option<Color> {
    if color_str.starts_with('#') && color_str.len() == 7 {
        let r: u8 = u8::from_str_radix(&color_str[1..3], 16).ok()?;
        let g: u8 = u8::from_str_radix(&color_str[3..5], 16).ok()?;
        let b: u8 = u8::from_str_radix(&color_str[5..7], 16).ok()?;
        Some(Color::RGB(r, g, b))
    } else {
        None
    }
}

enum DrawPosition<'cm> {
    ScreenCoords { x: i32, y: i32 },
    RealCoords { camera: &'cm Camera },
}

struct SideJoystickGUI {
    position: (i16, i16),
    is_clicked: bool,
    direction: WorldVector,
    acceleration_key: Scancode,
    acceleration_pressed: bool,
    inner_radius: f32,
    outer_radius: f32,
}

impl SideJoystickGUI {
    fn new(
        position: (i16, i16),
        acceleration_key: Scancode,
        inner_radius: f32,
        outer_radius: f32,
    ) -> Self {
        Self {
            position,
            is_clicked: false,
            direction: WorldVector::zero(),
            acceleration_key,
            acceleration_pressed: false,
            inner_radius,
            outer_radius,
        }
    }

    fn update(&mut self, mouse_position: (f32, f32), mouse_down: bool, acceleration_pressed: bool) {
        self.acceleration_pressed = acceleration_pressed;

        if mouse_down {
            let dist_to_joystick: f32 = ((mouse_position.0 - self.position.0 as f32).powi(2)
                + (mouse_position.1 - self.position.1 as f32).powi(2))
            .sqrt();

            if dist_to_joystick < self.inner_radius {
                self.is_clicked = true;
            }
        } else {
            self.is_clicked = false;
        }

        if self.is_clicked {
            let mouse_x: f32 = mouse_position.0;
            let mouse_y: f32 = mouse_position.1;
            let dx: f32 = mouse_x - self.position.0 as f32;
            let dy: f32 = mouse_y - self.position.1 as f32;
            let max_reach: f32 = self.outer_radius - 0.8 * self.inner_radius;
            self.direction = (1. / max_reach) * WorldVector::new(Meter::new(dx), -Meter::new(dy));
            if self.direction.norm().value() > 1.0 {
                self.direction = self.direction.normalized();
            }
        } else {
            self.direction = WorldVector::zero();
        }
    }

    fn render<'tc, T: RenderTarget, U>(
        &self,
        canvas: &mut Canvas<T>,
        texture_creator: &'tc render::TextureCreator<U>,
        texture_cache: &mut TextureCache<'tc, HashableGraphicProps>,
    ) -> Result<(), ()> {
        todo!();
        // let (center_x, center_y) = self.position;
        // GUICircle {
        //     radius: self.outer_radius,
        //     color: Color::RGB(200, 200, 200),
        // }
        // .draw_onto(
        //     canvas,
        //     texture_creator,
        //     texture_cache,
        //     DrawPosition::ScreenCoords {
        //         x: center_x as i32,
        //         y: center_y as i32,
        //     },
        // )?;

        // let max_reach = self.outer_radius - 0.8 * self.inner_radius;
        // let pos_x: f32 = center_x as f32 + self.direction.x.0 * max_reach;
        // let pos_y: f32 = center_y as f32 - self.direction.y.0 * max_reach;

        // GUIDisc {
        //     radius: self.inner_radius,
        //     color: if self.is_clicked {
        //         Color::RGB(125, 125, 125)
        //     } else {
        //         Color::RGB(175, 175, 175)
        //     },
        // }
        // .draw_onto(
        //     canvas,
        //     texture_creator,
        //     texture_cache,
        //     DrawPosition::ScreenCoords {
        //         x: pos_x as i32,
        //         y: pos_y as i32,
        //     },
        // )?;
        // Ok(())
    }
}

enum PlayerControl {
    Gamepad(Joystick),
    SideJoystick(SideJoystickGUI),
    CenteredJoystick { button: Scancode },
    RustFunction(fn(&Car, &Track) -> WorldVector),
    ExternalFunction,
    None,
}

struct PlayerProfile {
    name: String,
    control: PlayerControl,
}

struct Player {
    profile: PlayerProfile,
    car: Car,
}

struct Track {
    outer_border: WorldPolygon,
    inner_border: WorldPolygon,
    circuit_polygon: WorldPolygon,
    bezier_path: BezierPath,
    starting_point: WorldCoords,
    starting_direction: WorldVector,
}

impl Track {
    fn from_bezier(bezier_path: BezierPath, path_width: f32) -> Self {
        let (outer_border, inner_border) = bezier_path.generate_path_borders(path_width);
        let starting_point: WorldCoords = bezier_path.starting_point;
        let starting_direction: WorldVector = bezier_path.starting_direction;
        let inner_border = WorldPolygon::new(
            inner_border
                .iter()
                .chain(iter::once(inner_border.first().unwrap()))
                .copied()
                .collect::<Vec<WorldCoords>>()
                .as_slice(),
        );
        let outer_border = WorldPolygon::new(
            outer_border
                .iter()
                .chain(iter::once(outer_border.first().unwrap()))
                .copied()
                .collect::<Vec<WorldCoords>>()
                .as_slice(),
        );
        let circuit_polygon: WorldPolygon = WorldPolygon::new(
            inner_border
                .vertices()
                .iter()
                .chain(outer_border.vertices().iter().rev())
                .copied()
                .collect::<Vec<_>>()
                .as_slice(),
        );
        Self {
            outer_border,
            inner_border,
            circuit_polygon,
            bezier_path,
            starting_point,
            starting_direction,
        }
    }
}

struct Game {
    players: Vec<Player>,
    track: Track,
    seed: u64,
    game_tick: u32,
    dt: f32,
}

impl Game {
    fn init_game(
        profiles: Vec<PlayerProfile>,
        seed: Option<u64>,
        path_width: f32,
        dt: f32,
    ) -> Self {
        let seed: u64 = seed.unwrap_or_else(|| rand::random_range(0..u64::MAX));
        let path_config: BezierPathConfig = BezierPathConfig::default();
        let path: BezierPath = path_config.generate_path(Some(seed));
        let track: Track = Track::from_bezier(path, path_width);

        let players = profiles
            .into_iter()
            .map(|profile| Player {
                profile,
                car: Car::init(track.starting_point, track.starting_direction),
            })
            .collect();

        Self {
            players,
            track,
            seed,
            game_tick: 0,
            dt,
        }
    }
}

impl Game {
    fn get_player_decision(player_control: &PlayerControl) -> PlayerDecision {
        match player_control {
            PlayerControl::Gamepad(joystick) => {
                let axis_0: f32 = joystick.axis(0).unwrap_or(0) as f32 / 32768.0;
                let axis_1: f32 = joystick.axis(1).unwrap_or(0) as f32 / 32768.0;
                let joystick_direction = WorldVector::new(Meter::new(axis_0), -Meter::new(axis_1));
                let acceleration_pressed = joystick.button(0).unwrap_or(false);
                PlayerDecision {
                    direction: joystick_direction,
                    accelerate: acceleration_pressed,
                }
            }
            PlayerControl::SideJoystick(joystick) => PlayerDecision {
                direction: joystick.direction.normalized(),
                accelerate: joystick.acceleration_pressed,
            },
            PlayerControl::CenteredJoystick { .. } => todo!(),
            PlayerControl::RustFunction(_) => todo!(),
            PlayerControl::ExternalFunction => todo!(),
            PlayerControl::None => PlayerDecision {
                direction: WorldVector::zero(),
                accelerate: false,
            },
        }
    }

    fn update(&mut self) {
        // Apply player input to the car's acceleration
        for player in &mut self.players {
            let decision = Self::get_player_decision(&player.profile.control);
            let joystick_direction = decision.direction;
            let acceleration_pressed = decision.accelerate;
            let car = &mut player.car;
            if acceleration_pressed {
                let engine_acc_vec = if joystick_direction.norm().value() < 0.2 {
                    WorldVector::zero()
                } else {
                    joystick_direction.normalized()
                };
                car.set_engine_acc(ENGINE_ACCELERATION * engine_acc_vec);
            } else {
                car.set_engine_acc(WorldVector::zero());
            }

            // Check if any of the 4 corners of the car rect is outside the circuit polygon
            let mut friction_multiplier: f32 = 2.0;
            let car_corners: [WorldCoords; 4] = car.corner_coords();
            let mut is_car_inside: bool = true;
            for corner in &car_corners {
                if !self.track.circuit_polygon.contains_point(*corner) {
                    is_car_inside = false;
                    friction_multiplier += 1.0;
                }
            }
            if is_car_inside {
                car.set_friction_factor(FRICTION_FACTOR);
            } else {
                car.set_friction_factor(friction_multiplier * FRICTION_FACTOR);
            }

            // Update the car's coordinates based on its current acceleration and velocity
            car.update_coords(self.dt);
        }
        self.game_tick += 1;
    }
}

struct PlayerDecision {
    direction: WorldVector,
    accelerate: bool,
}

const CAMERA_WIDTH: u32 = 1200;
const CAMERA_HEIGHT: u32 = 900;
const SIDE_PANEL_WIDTH: u32 = 400;

const SCREEN_WIDTH: u32 = CAMERA_WIDTH + SIDE_PANEL_WIDTH;
const SCREEN_HEIGHT: u32 = CAMERA_HEIGHT;

const JOYSTICK_POSITION: (i16, i16) = (
    CAMERA_WIDTH as i16 + SIDE_PANEL_WIDTH as i16 / 2,
    CAMERA_HEIGHT as i16 / 3,
);
const JOYSTICK_INNER_RADIUS: f32 = 50.;
const JOYSTICK_OUTER_RADIUS: f32 = 100.;

const LN_100: f32 = 4.60517;
const TIME_TO_99_PERC_SPEED: f32 = 5.; // in s
const THEORETICAL_MAX_SPEED: f32 = 100.; // in m/s
const FRICTION_FACTOR: f32 = LN_100 / TIME_TO_99_PERC_SPEED;
const ENGINE_ACCELERATION: f32 = THEORETICAL_MAX_SPEED * FRICTION_FACTOR;

const N_ANCHORS: usize = 10;
const MEAN_ANCHOR_RADIUS: f32 = 120.;
const STDEV_ANCHOR_RADIUS: f32 = 55.;
const STDEV_SUBANCHOR_ANGLE_DEV: f32 = (360. / N_ANCHORS as f32) / 3.0;
const POINTS_PER_BEZIER_CURVE: usize = 15;
const PATH_WIDTH: f32 = 15.0;

const CIRCUIT_DEBUG: bool = false;
const MOUVEMENT_DEBUG: bool = true;
const DEBUG_MAX_FPS: bool = false;
const REGULAR_LOG_FPS: bool = true;
const REGULAR_LOG_PERF: bool = true;
const REGULARITY_INTERVAL: u32 = 60 * 3;
const LOG_PERF_ON_LAG: bool = true;

const VSYNC: bool = true && !DEBUG_MAX_FPS;

const DEBUG_PERFORMANCE_LABELS: [&str; 11] = [
    "             Input Handeling",
    "                   Car Logic",
    "             Screen Clearing",
    "     Enclosed Area Rendering",
    "      Circuit Path Rendering",
    "Circuit Generation Debugging",
    "               Car Rendering",
    "      Car Movement Debugging",
    "              Text Rendering",
    "        Side Panel Rendering",
    "               Screen Update",
];

const GAME_FPS: f32 = 60.0;
const DT: f32 = GAME_FPS.recip();

pub fn main() {
    // Initialize SDL
    if VSYNC {
        sdl3::hint::set("SDL_RENDER_VSYNC", "1");
    }
    let sdl_context: sdl3::Sdl = sdl3::init().unwrap();
    let video_subsystem: sdl3::VideoSubsystem = sdl_context.video().unwrap();

    // Create a window
    let window: Window = video_subsystem
        .window("Car Racing Game", SCREEN_WIDTH, SCREEN_HEIGHT)
        .position_centered()
        .build()
        .unwrap();

    // Transform the window into a canvas
    let mut canvas: Canvas<Window> = window.into_canvas();

    // Initialize a texture creator
    let texture_creator: render::TextureCreator<WindowContext> = canvas.texture_creator();

    // Initalize connection with a controller joystick
    let mut joystick: Option<Joystick> = None;
    if let Ok(joystick_subsystem) = sdl_context.joystick() {
        println!("Joystick subsystem initialized");
        joystick_subsystem.set_joystick_events_enabled(true);
        if let Some(fst_joystick_id) = joystick_subsystem
            .joysticks()
            .ok()
            .and_then(|joysticks| joysticks.first().cloned())
        {
            joystick = joystick_subsystem
                .open(fst_joystick_id)
                .ok()
                .filter(|js: &Joystick| js.num_axes() >= 2);
        }
    }

    // Initialize the Game
    let control = match joystick {
        Some(js) => PlayerControl::Gamepad(js),
        None => PlayerControl::SideJoystick(SideJoystickGUI::new(
            JOYSTICK_POSITION,
            Scancode::Space,
            JOYSTICK_INNER_RADIUS,
            JOYSTICK_OUTER_RADIUS,
        )),
    };

    let mut game: Game = Game::init_game(
        vec![PlayerProfile {
            name: "Human1".to_string(),
            control,
        }],
        None,
        PATH_WIDTH,
        DT,
    );

    // Initialize the cameras
    let mut car_camera: Camera = Camera::new(
        CAMERA_WIDTH,
        CAMERA_HEIGHT,
        game.players[0].car.coords,
        (CAMERA_WIDTH as f32 / (game.players[0].car.length.value() / 0.04)).round() as u32,
        // car length is 4% of the camera width
    );

    let followed_car_index: usize = 0;

    let fix_camera: Camera = Camera::new(CAMERA_WIDTH, CAMERA_HEIGHT, WorldCoords::origin(), 2);

    let mut current_camera: usize = 0; // 0 for car_camera, 1 for fix_camera

    // Initialize Font
    let ttf_context: ttf::Sdl3TtfContext = ttf::init().unwrap();
    let text_font: TextFont = TextFont::new(
        ttf_context
            .load_font("assets/Basic-Regular.ttf", 25.)
            .unwrap(),
        "Basic-Regular",
    );

    // Initialize the texture caching system
    // TODO: Create a rendering context with texture_cache as a potential struct field
    //       this could include the texture creator, can be passed as one object to rendering
    //       functions rether than always including both the texture creator and the cache
    //       separately. And it could also remove the need to alway have a visible variable of type
    //       Option<TextureCache> circulating in the main function.
    let mut texture_cache: TextureCache<HashableGraphicProps> = TextureCache::init();

    // Render text textures
    let hold_space_text = TextGUI::new(
        "PRESS ENTER TO TOGGLE CAMERAS",
        &text_font,
        Color::RGB(255, 255, 255),
    );

    let js_name_str = if let PlayerControl::Gamepad(js) = &game.players[0].profile.control {
        format!("Controller: {}", js.name())
    } else {
        "No controller detected at startup".to_string()
    };

    let js_name_text = TextGUI::new(&js_name_str, &text_font, Color::RGB(255, 255, 255));

    let seed_text = TextGUI::new(
        &format!("SEED: {}", game.seed),
        &text_font,
        Color::RGB(255, 255, 255),
    );

    // Initialize an FPS sample queue to monitor average FPS across 3 seconds
    let mut fps_sample: VecDeque<f32> = VecDeque::with_capacity(3 * GAME_FPS as usize);
    for _ in 0..(3 * GAME_FPS as usize) {
        fps_sample.push_back(GAME_FPS);
    }

    // Initialize the event pump for handling SDL events
    let mut event_pump: sdl3::EventPump = sdl_context.event_pump().unwrap();

    'running: loop {
        let time: time::Instant = time::Instant::now();
        let mut timestamps: Vec<u128> = Vec::new();

        // Read the events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::Return),
                    ..
                } => {
                    current_camera = 1 - current_camera;
                }
                _ => (),
            }
        }

        let keyboard_state: KeyboardState<'_> = event_pump.keyboard_state();

        let mouse_down = event_pump.mouse_state().left();
        let mouse_pos = (event_pump.mouse_state().x(), event_pump.mouse_state().y());

        for player in &mut game.players {
            let player_control = &mut player.profile.control;
            if let PlayerControl::SideJoystick(joystick) = player_control {
                joystick.update(
                    mouse_pos,
                    mouse_down,
                    keyboard_state.is_scancode_pressed(joystick.acceleration_key),
                );
            }
        }

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        game.update();

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        // Update cameras
        car_camera.set_coords(game.players[followed_car_index].car.coords);

        // Determine which camera to use for rendering
        let current_camera: &Camera = if current_camera == 0 {
            &car_camera
        } else {
            &fix_camera
        };

        // Start rendering
        let mut failed_rendering: bool = false;

        // Fill the entire screen with dark green
        canvas.set_draw_color(Color::RGB(25, 50, 25));
        canvas.clear();

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        // Draw the circuit
        failed_rendering |= game
            .track
            .inner_border
            .as_graphic(true, Color::RGB(10, 70, 10))
            .cached_render_onto(
                current_camera,
                &mut canvas,
                &texture_creator,
                &mut texture_cache,
            )
            .map_err(|e| eprintln!("Failed to render the enclosed area: {:?}", e))
            .is_err();

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        failed_rendering |= game
            .track
            .circuit_polygon
            .as_graphic(true, Color::RGB(10, 70, 10))
            .cached_render_onto(
                current_camera,
                &mut canvas,
                &texture_creator,
                &mut texture_cache,
            )
            .map_err(|e| eprintln!("Failed to render the circuit polygon: {:?}", e))
            .is_err();

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        if CIRCUIT_DEBUG {
            failed_rendering |= game
                .track
                .bezier_path
                .render(
                    current_camera,
                    &mut canvas,
                    &texture_creator,
                    &mut texture_cache,
                )
                .map_err(|e| eprintln!("Failed to render the path: {:?}", e))
                .is_err();
            failed_rendering |= game
                .track
                .bezier_path
                .render_debug_anchors(current_camera, &mut canvas)
                .map_err(|e| eprintln!("Failed to render the path debug anchors: {:?}", e))
                .is_err();
        }

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        // Render the car
        for player in &game.players {
            failed_rendering |= &player
                .car
                .pretty_render(current_camera, &mut canvas)
                .map_err(|e| eprintln!("Failed to render the car: {:?}", e))
                .is_err();
        }

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        // Debug car vectors
        if MOUVEMENT_DEBUG {
            for player in &game.players {
                failed_rendering |= &player
                    .car
                    .render_debug_vectors(current_camera, &mut canvas)
                    .map_err(|e| eprintln!("Failed to render the car debug vectors: {:?}", e))
                    .is_err();
            }
        }

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        failed_rendering |= hold_space_text
            .cached_render_onto(
                &mut canvas,
                &texture_creator,
                &mut texture_cache,
                (
                    10,
                    SCREEN_HEIGHT as i32 - hold_space_text.height() as i32 - 10,
                ),
            )
            .map_err(|e| eprintln!("Failed to render the hold space text: {:?}", e))
            .is_err();

        failed_rendering |= js_name_text
            .cached_render_onto(
                &mut canvas,
                &texture_creator,
                &mut texture_cache,
                (SCREEN_WIDTH as i32 - js_name_text.width() as i32 - 10, 10),
            )
            .map_err(|e| eprintln!("Failed to render the joystick name text: {:?}", e))
            .is_err();

        failed_rendering |= seed_text
            .cached_render_onto(&mut canvas, &texture_creator, &mut texture_cache, (10, 10))
            .map_err(|e| eprintln!("Failed to render the seed text: {:?}", e))
            .is_err();

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        // Draw a seperating line between the game area and the side panel
        canvas.set_draw_color(Color::RGB(100, 100, 100));
        let (x1, y1) = (CAMERA_WIDTH, 0u32);
        let (x2, y2) = (CAMERA_WIDTH, CAMERA_HEIGHT);
        failed_rendering |= canvas
            .draw_line((x1 as f32, y1 as f32), (x2 as f32, y2 as f32))
            .map_err(|e| eprintln!("Failed to draw the separating line: {:?}", e))
            .is_err();

        // Render the joystick
        for player in &game.players {
            if let PlayerControl::SideJoystick(js) = &player.profile.control {
                failed_rendering |= js
                    .render(&mut canvas, &texture_creator, &mut texture_cache)
                    .map_err(|e| eprintln!("Failed to render the joystick: {:?}", e))
                    .is_err();
            }
        }

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        // Apply the changes on the screen
        canvas.present();

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        // Detect failed rendering attempts
        if failed_rendering {
            eprintln!(
                "Warning: Failed rendering one or more elements on frame {}.",
                game.game_tick
            );
        }

        // Sleep to control the FPS
        let mut slept: bool = false;
        if !DEBUG_MAX_FPS {
            let frame_duration: Duration =
                Duration::new(0, (1_000_000_000f32 / GAME_FPS).round() as u32);
            if let Some(remaining_time) = frame_duration.checked_sub(time.elapsed()) {
                std::thread::sleep(remaining_time);
                slept = true;
            }
        }

        // Compute the average FPS
        fps_sample.push_back(1.0 / time.elapsed().as_secs_f32());
        fps_sample.pop_front();
        let average_fps: f32 = fps_sample.iter().sum::<f32>() / fps_sample.len() as f32;
        let mut log_fps: bool =
            REGULAR_LOG_FPS && game.game_tick.is_multiple_of(REGULARITY_INTERVAL);
        let mut log_perf: bool =
            REGULAR_LOG_PERF && game.game_tick.is_multiple_of(REGULARITY_INTERVAL);

        // If average frame rate drops below 80% of the frame target, log a warning
        if average_fps < 0.8 * GAME_FPS {
            eprintln!(
                "Warning: Average FPS dropped below 80% of the target on frame {}.",
                game.game_tick
            );
            log_fps = true;
            LOG_PERF_ON_LAG.then(|| log_perf = true);
        }

        if log_fps || log_perf {
            println!("Frame {}:", game.game_tick);
        }

        if log_fps {
            println!("Average FPS: {}", average_fps);
        }

        if log_perf {
            if timestamps.len() != DEBUG_PERFORMANCE_LABELS.len() {
                eprintln!("Warning: Mismatch between timestamps and debug performance labels.");
            }

            let frame_duration: u128 = *timestamps.last().unwrap();
            println!(
                "Frame duration (sleep excluded): {} µs -> {:.2}% of frame target",
                frame_duration,
                100.0 * frame_duration as f32 / (1_000_000.0 / GAME_FPS)
            );
            if slept {
                println!("The game slept this frame.");
            }

            println!("Detailed performance breakdown:");

            let mut last_timestamp: u128 = 0;
            for (i, timestamp) in timestamps.iter().enumerate() {
                println!(
                    "{}: {} µs -> {:.1}% of frame target",
                    DEBUG_PERFORMANCE_LABELS[i],
                    timestamp - last_timestamp,
                    100.0 * (timestamp - last_timestamp) as f32 / (1_000_000.0 / GAME_FPS)
                );
                last_timestamp = *timestamp;
            }
        }

        if log_fps || log_perf {
            println!();
        }

        // println!("{}", texture_cache.as_ref().unwrap().texture_cache.len());
    }
}
