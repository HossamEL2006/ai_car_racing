use crate::default_game_config::*;
use crate::{
    utils::parse_color,
    world_objects::{Meter, WorldCoords, WorldPolygon, WorldVector},
};
use rand::{SeedableRng, rngs::StdRng};
use rand_distr::{Distribution, Normal};
use sdl3::pixels::Color;
use std::{f32, iter};
use svg::{node::element::tag, parser};

pub(crate) struct Path {
    pub coords: Vec<WorldCoords>,
    pub starting_point: WorldCoords,
    pub starting_direction: WorldVector,
    // pub(crate) config: BezierPathConfig, // for debugging
    // pub(crate) anchor_triplets: Vec<[WorldCoords; 3]>, // for debugging
}

impl Path {
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
            let points_per_curve: i32 = self.coords.len() as i32 / 10; // 10 is arbitrary
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

pub struct Track {
    outer_border: WorldPolygon,
    inner_border: WorldPolygon,
    circuit_polygon: WorldPolygon,
    path: Path,
    starting_point: WorldCoords,
    starting_direction: WorldVector,
}

impl Track {
    fn from_path(path: Path, path_width: f32) -> Self {
        let (outer_border, inner_border) = path.generate_path_borders(path_width);
        let starting_point: WorldCoords = path.starting_point;
        let starting_direction: WorldVector = path.starting_direction;
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
            path,
            starting_point,
            starting_direction,
        }
    }

    pub fn inner_border(&self) -> &WorldPolygon {
        &self.inner_border
    }

    pub fn outer_border(&self) -> &WorldPolygon {
        &self.outer_border
    }

    pub fn circuit_polygon(&self) -> &WorldPolygon {
        &self.circuit_polygon
    }

    pub fn bezier_path(&self) -> &Path {
        &self.path
    }
}

pub(crate) struct BezierPathConfig {
    pub center: WorldCoords,
    pub n_anchors: usize,
    pub mean_anchor_radius: f32,
    pub stdev_anchor_radius: f32,
    pub stdev_subanchor_angle_dev: f32,
    pub points_per_bezier_curve: usize,
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

impl BezierPathConfig {
    fn into_path(self, seed: u64) -> Path {
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

        let mut rng: StdRng = StdRng::seed_from_u64(seed);

        let anchors: Vec<WorldCoords> = sample_anchors(&self, &mut rng);

        let starting_point: WorldCoords = anchors[0];

        let anchor_triplets: Vec<[WorldCoords; 3]> =
            sample_anchor_triplets(&self, anchors, &mut rng);

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

        Path {
            coords,
            starting_point,
            starting_direction,
            // config: self.clone(),
            // anchor_triplets,
        }
    }
}

pub struct CarConfig {
    pub length: Meter,
    pub width: Meter,
    pub friction_factor: f32,
    pub engine_acc_magnitude: f32,
}

impl Default for CarConfig {
    fn default() -> Self {
        CarConfig {
            length: Meter::new(CAR_LENGTH),
            width: Meter::new(CAR_WIDTH),
            engine_acc_magnitude: ENGINE_ACCELERATION,
            friction_factor: FRICTION_FACTOR,
        }
    }
}

pub struct Car {
    pub(crate) coords: WorldCoords,
    pub(crate) direction_vec: WorldVector,
    pub(crate) length: Meter,
    pub(crate) width: Meter,
    pub(crate) engine_acc_vector: WorldVector,
    pub(crate) velocity_vector: WorldVector,
    pub(crate) engine_acc_magnitude: f32,
    pub(crate) base_friction_factor: f32,
    pub(crate) curr_friction_factor: f32,
    pub(crate) car_polys: Vec<(WorldPolygon, Color)>,
}

impl Car {
    pub fn init(car_config: &CarConfig, coords: WorldCoords, direction: WorldVector) -> Car {
        let mut car: Car = Car {
            coords,
            direction_vec: direction.normalized(),
            length: car_config.length, // 5
            width: car_config.width,   // 2
            engine_acc_vector: WorldVector::zero(),
            velocity_vector: WorldVector::zero(),
            engine_acc_magnitude: car_config.engine_acc_magnitude,
            base_friction_factor: car_config.friction_factor,
            curr_friction_factor: car_config.friction_factor,
            car_polys: Vec::new(),
        };
        car.car_polys = car.load_svg();
        car
    }

    pub fn corner_coords(&self) -> [WorldCoords; 4] {
        [(1., 1.), (-1., 1.), (-1., -1.), (1., -1.)].map(|(d1, d2)| {
            self.coords
                + (d1 * (self.length.value() / 2.)) * self.direction_vec
                + (d2 * (self.width.value() / 2.)) * self.direction_vec.orthogonal()
        })
    }

    pub fn as_rect_polygon(&self) -> WorldPolygon {
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

    fn set_engine_acc(&mut self, acc: WorldVector) {
        self.engine_acc_vector = acc
    }

    fn set_friction_factor(&mut self, friction_factor: f32) {
        self.curr_friction_factor = friction_factor;
    }

    fn update_coords(&mut self, dt: f32) {
        let friction_acc = -self.curr_friction_factor * self.velocity_vector;
        self.velocity_vector =
            self.velocity_vector + dt * self.engine_acc_vector + dt * friction_acc;
        let velocity_norm: f32 = self.velocity_vector.norm().value();
        if velocity_norm > 1e-4 {
            self.direction_vec = (1. / velocity_norm) * self.velocity_vector;
        }
        self.coords = self.coords + dt * self.velocity_vector;
    }

    fn update(
        &mut self,
        acceleration_pressed: bool,
        joystick_direction: WorldVector,
        track: &Track,
        dt: f32,
    ) {
        if acceleration_pressed {
            let engine_acc_vec = if joystick_direction.norm().value() < 0.2 {
                WorldVector::zero()
            } else {
                joystick_direction.normalized()
            };
            self.set_engine_acc(self.engine_acc_magnitude * engine_acc_vec);
        } else {
            self.set_engine_acc(WorldVector::zero());
        }

        // Check if any of the 4 corners of the car rect is outside the circuit polygon
        let mut friction_multiplier: f32 = 2.0;
        let car_corners: [WorldCoords; 4] = self.corner_coords();
        let mut is_car_inside: bool = true;
        for corner in &car_corners {
            if !track.circuit_polygon.contains_point(*corner) {
                is_car_inside = false;
                friction_multiplier += 1.0;
            }
        }
        if is_car_inside {
            self.set_friction_factor(self.base_friction_factor);
        } else {
            self.set_friction_factor(friction_multiplier * self.base_friction_factor);
        }

        // Update the car's coordinates based on its current acceleration and velocity
        self.update_coords(dt);
    }
}

pub struct PlayerDecision {
    pub direction: WorldVector,
    pub accelerate: bool,
}

pub struct Player {
    car: Car,
    decision: PlayerDecision,
}

impl Player {
    pub fn car(&self) -> &Car {
        &self.car
    }

    pub fn mut_car(&mut self) -> &mut Car {
        &mut self.car
    }

    pub fn set_decision(&mut self, decision: PlayerDecision) {
        self.decision = decision;
    }
}

pub enum PathConfig {
    Random {
        path_config: BezierPathConfig,
        seed: Option<u64>,
    },
    Custom {
        path: Path,
    },
}

impl Default for PathConfig {
    fn default() -> Self {
        PathConfig::Random {
            path_config: BezierPathConfig::default(),
            seed: None,
        }
    }
}

pub struct GameConfig {
    pub path_config: PathConfig,
    pub car_config: CarConfig,
    pub track_width: f32,
    pub dt: f32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            path_config: PathConfig::default(),
            car_config: CarConfig::default(),
            track_width: PATH_WIDTH,
            dt: DT,
        }
    }
}

pub struct Game {
    players: Vec<Player>,
    track: Track,
    seed: Option<u64>,
    game_tick: u32,
    dt: f32,
}

impl Game {
    pub fn init_game(n_players: usize, game_config: GameConfig) -> Self {
        let mut game_seed = None;

        let path = match game_config.path_config {
            PathConfig::Random { path_config, seed } => {
                let x = seed.unwrap_or_else(|| rand::random_range(0..u64::MAX));
                game_seed = Some(x);
                path_config.into_path(x)
            }
            PathConfig::Custom { path } => path,
        };

        let track: Track = Track::from_path(path, game_config.track_width);

        let players = (0..n_players)
            .map(|_| Player {
                car: Car::init(
                    &game_config.car_config,
                    track.starting_point,
                    track.starting_direction,
                ),
                decision: PlayerDecision {
                    direction: WorldVector::zero(),
                    accelerate: false,
                },
            })
            .collect();

        Self {
            players,
            track,
            seed: game_seed,
            game_tick: 0,
            dt: game_config.dt,
        }
    }

    pub fn update(&mut self) {
        // Apply player input to the car's acceleration
        for player in &mut self.players {
            let decision = &player.decision;
            let joystick_direction = decision.direction;
            let acceleration_pressed = decision.accelerate;
            let car = &mut player.car;

            car.update(
                acceleration_pressed,
                joystick_direction,
                &self.track,
                self.dt,
            );
        }
        self.game_tick += 1;
    }

    pub fn seed(&self) -> Option<u64> {
        self.seed
    }

    pub fn game_tick(&self) -> u32 {
        self.game_tick
    }

    pub fn players(&self) -> &Vec<Player> {
        &self.players
    }

    pub fn mut_players(&mut self) -> &mut Vec<Player> {
        &mut self.players
    }

    pub fn track(&self) -> &Track {
        &self.track
    }
}
