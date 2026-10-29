use core::f32;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand_distr::{Distribution, Normal};
use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::pixels::Color;
use sdl3::rect::{Point, Rect};
use sdl3::render::{Canvas, FPoint};
use sdl3::ttf::Font;
use sdl3::video::Window;
use std::time::Duration;
use std::{iter, ops};

#[derive(Clone, Copy, PartialEq)]
struct Meter(f32);

impl ops::Add for Meter {
    type Output = Meter;

    fn add(self, rhs: Self) -> Self::Output {
        Meter(self.0 + rhs.0)
    }
}

impl ops::Sub for Meter {
    type Output = Meter;

    fn sub(self, rhs: Self) -> Self::Output {
        Meter(self.0 - rhs.0)
    }
}

impl ops::Neg for Meter {
    type Output = Meter;

    fn neg(self) -> Self::Output {
        Meter(-self.0)
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Coords {
    x: Meter,
    y: Meter,
}

#[derive(Clone, Copy, PartialEq)]
struct Vector {
    x: Meter,
    y: Meter,
}

impl ops::Sub for Coords {
    type Output = Vector;

    fn sub(self, rhs: Self) -> Self::Output {
        Vector {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl ops::Add<Vector> for Coords {
    type Output = Coords;

    fn add(self, rhs: Vector) -> Self::Output {
        Coords {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Coords {
    fn init(x: f32, y: f32) -> Coords {
        Coords {
            x: Meter(x),
            y: Meter(y),
        }
    }

    fn as_fpoint(&self, camera: &Camera) -> FPoint {
        let px_coords: Vector = (camera.px_per_meter as f32) * (*self - camera.coords);
        FPoint::new(
            (camera.width / 2) as f32 + px_coords.x.0,
            (camera.height / 2) as f32 - px_coords.y.0,
        )
    }

    fn as_point(&self, camera: &Camera) -> Point {
        let px_coords: Vector = (camera.px_per_meter as f32) * (*self - camera.coords);
        Point::new(
            (camera.width / 2) as i32 + (px_coords.x.0).round() as i32,
            (camera.height / 2) as i32 - (px_coords.y.0).round() as i32,
        )
    }

    fn render(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), sdl3::Error> {
        canvas.draw_rect(Rect::from_center(self.as_point(camera), 5, 5))
    }
}

impl Vector {
    fn init(x: f32, y: f32) -> Vector {
        Vector {
            x: Meter(x),
            y: Meter(y),
        }
    }

    fn orthogonal(&self) -> Vector {
        Vector {
            x: -self.y,
            y: self.x,
        }
    }

    fn norm(&self) -> Meter {
        Meter((self.x.0.powi(2) + self.y.0.powi(2)).sqrt())
    }

    fn normalized(&self) -> Vector {
        self.norm().0.recip() * *self
    }

    fn rotated(&self, theta: f32) -> Vector {
        let cos_theta: f32 = theta.cos();
        let sin_theta: f32 = theta.sin();

        let x: f32 = self.x.0;
        let y: f32 = self.y.0;

        Vector {
            x: Meter(x * cos_theta - y * sin_theta),
            y: Meter(x * sin_theta + y * cos_theta),
        }
    }
}

impl ops::Add for Vector {
    type Output = Vector;

    fn add(self, rhs: Self) -> Self::Output {
        Vector {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl ops::Mul<Vector> for f32 {
    type Output = Vector;

    fn mul(self, mut rhs: Vector) -> Self::Output {
        rhs.x.0 *= self;
        rhs.y.0 *= self;
        rhs
    }
}

struct Camera {
    width: u32,
    height: u32,
    coords: Coords,
    px_per_meter: u32,
}

struct Car {
    coords: Coords,
    direction_vec: Vector,
    length: Meter,
    width: Meter,
    acceleration: Vector,
    velocity: Vector,
}

impl Car {
    fn init(coords: Coords, direction: Vector) -> Car {
        Car {
            coords,
            direction_vec: direction.normalized(),
            length: Meter(4.5),
            width: Meter(2.),
            acceleration: Vector::init(0., 0.),
            velocity: Vector::init(0., 0.),
        }
    }

    fn render(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), sdl3::Error> {
        let corners: Vec<FPoint> = [(1., 1.), (-1., 1.), (-1., -1.), (1., -1.), (1., 1.)]
            .into_iter()
            .map(|(d1, d2)| {
                self.coords
                    + (d1 * (self.length.0 / 2.)) * self.direction_vec
                    + (d2 * (self.width.0 / 2.)) * self.direction_vec.orthogonal()
            })
            .map(|coords: Coords| coords.as_fpoint(camera))
            .collect();

        canvas.draw_lines(corners.as_slice())
    }

    fn set_acceleration(&mut self, acc: Vector) {
        self.acceleration = acc
    }

    fn update_coords(&mut self, dt: f32) {
        self.velocity = self.velocity + dt * self.acceleration;
        let velocity_norm: f32 = self.velocity.norm().0;
        if velocity_norm != 0. {
            self.direction_vec = (1. / velocity_norm) * self.velocity;
        }
        self.coords = self.coords + dt * self.velocity;
    }
}

fn render_coord_loop(
    coords_loop: &[Coords],
    camera: &Camera,
    canvas: &mut Canvas<Window>,
) -> Result<(), sdl3::Error> {
    let mut coords_vec: Vec<FPoint> = coords_loop
        .iter()
        .map(|coord: &Coords| coord.as_fpoint(camera))
        .collect();
    coords_vec.push(coords_loop[0].as_fpoint(camera));
    canvas.draw_lines(coords_vec.as_slice())
}

#[derive(Clone)]
struct BezierPathConfig {
    center: Coords,
    n_anchors: i32,
    mean_anchor_radius: f32,
    stdev_anchor_radius: f32,
    stdev_subanchor_angle_dev: f32,
    points_per_bezier_curve: usize,
}

impl Default for BezierPathConfig {
    fn default() -> Self {
        BezierPathConfig {
            center: Coords::init(0., 0.),
            n_anchors: 7,
            mean_anchor_radius: 60.,
            stdev_anchor_radius: 20.,
            stdev_subanchor_angle_dev: 10.,
            points_per_bezier_curve: 30,
        }
    }
}

struct BezierPath {
    coords: Vec<Coords>,
    starting_point: Coords,
    starting_direction: Vector,
    config: BezierPathConfig,          // for debug rendering
    anchor_triplets: Vec<[Coords; 3]>, // for debug rendering
}

impl BezierPathConfig {
    fn generate_path(&self, seed: Option<u64>) -> BezierPath {
        fn sample_anchors(config: &BezierPathConfig, rng: &mut StdRng) -> Vec<Coords> {
            let radius_normal_dist: Normal<f32> =
                Normal::new(config.mean_anchor_radius, config.stdev_anchor_radius).unwrap();
            let angle_dev_dist: Normal<f32> =
                Normal::new(0., (360. / config.n_anchors as f32) * 0.15).unwrap();

            (0..config.n_anchors)
                .map(|n: i32| {
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
                        .max(config.mean_anchor_radius - 2. * config.stdev_anchor_radius)
                        .max(0.);

                    Coords::init(
                        config.center.x.0 + radius * angle.to_radians().cos(),
                        config.center.y.0 + radius * angle.to_radians().sin(),
                    )
                })
                .collect()
        }

        fn sample_anchor_triplets(
            config: &BezierPathConfig,
            mut anchors: Vec<Coords>,
            rng: &mut StdRng,
        ) -> Vec<[Coords; 3]> {
            let mut anchor_triplets: Vec<[Coords; 3]> = Vec::new();

            let angle_dev_dist: Normal<f32> =
                Normal::new(0., config.stdev_subanchor_angle_dev).unwrap();

            let mut cyclic_anchors: Vec<Coords> = vec![*anchors.last().unwrap()];
            anchors.push(anchors[0]);
            cyclic_anchors.extend(anchors);

            // Add first anchor to the end to make it circular

            for (prev_anchor, anchor, next_anchor) in cyclic_anchors
                .windows(3)
                .map(|window: &[Coords]| (window[0], window[1], window[2]))
            {
                let angle: f32 = (anchor - config.center)
                    .y
                    .0
                    .atan2((anchor - config.center).x.0)
                    + f32::consts::FRAC_PI_2;
                let angle_dev: f32 = angle_dev_dist.sample(rng);
                let new_angle: f32 = angle + angle_dev.to_radians();

                let track_dir_vector: Vector = Vector::init(new_angle.cos(), new_angle.sin());

                let subanchor_dist_coef: f32 = 0.4;

                let right_subanchor_dist: f32 = (anchor - prev_anchor).norm().0;
                let right_subanchor: Coords =
                    anchor + subanchor_dist_coef * (-right_subanchor_dist) * track_dir_vector;

                let left_subanchor_dist: f32 = (next_anchor - anchor).norm().0;
                let left_subanchor: Coords =
                    anchor + subanchor_dist_coef * left_subanchor_dist * track_dir_vector;

                anchor_triplets.push([right_subanchor, anchor, left_subanchor]);
            }

            anchor_triplets
        }

        fn anchor_triplets_to_control_points(anchor_triplets: &[[Coords; 3]]) -> Vec<[Coords; 4]> {
            anchor_triplets
                .windows(2)
                .chain(iter::once(
                    [
                        *anchor_triplets.last().unwrap(),
                        *anchor_triplets.first().unwrap(),
                    ]
                    .as_slice(),
                ))
                .map(|anch_pair: &[[Coords; 3]]| {
                    let [_, center1, left1] = anch_pair[0];
                    let [right2, center2, _] = anch_pair[1];

                    [center1, left1, right2, center2]
                })
                .collect()
        }

        fn generate_bezier_curve(control_points: [Coords; 4], n: usize) -> Vec<Coords> {
            // the last point is not included to avoid duplication between consecutive curves
            (0..n)
                .map(|i: usize| {
                    let t: f32 = i as f32 / n as f32;
                    let one_minus_t: f32 = 1. - t;

                    let p0: Coords = control_points[0];
                    let p1: Coords = control_points[1];
                    let p2: Coords = control_points[2];
                    let p3: Coords = control_points[3];

                    let x: f32 = one_minus_t.powi(3) * p0.x.0
                        + 3. * one_minus_t.powi(2) * t * p1.x.0
                        + 3. * one_minus_t * t.powi(2) * p2.x.0
                        + t.powi(3) * p3.x.0;

                    let y: f32 = one_minus_t.powi(3) * p0.y.0
                        + 3. * one_minus_t.powi(2) * t * p1.y.0
                        + 3. * one_minus_t * t.powi(2) * p2.y.0
                        + t.powi(3) * p3.y.0;

                    Coords::init(x, y)
                })
                .collect()
        }

        let mut coords: Vec<Coords> = Vec::new();

        let mut rng: StdRng =
            StdRng::seed_from_u64(seed.unwrap_or(rand::random_range(0..u64::MAX)));

        let anchors: Vec<Coords> = sample_anchors(self, &mut rng);

        let starting_point: Coords = anchors[0];

        let anchor_triplets: Vec<[Coords; 3]> = sample_anchor_triplets(self, anchors, &mut rng);

        let vec_control_points: Vec<[Coords; 4]> =
            anchor_triplets_to_control_points(&anchor_triplets);

        vec_control_points
            .iter()
            .for_each(|control_points: &[Coords; 4]| {
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
    fn render(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), sdl3::Error> {
        render_coord_loop(&self.coords, camera, canvas)
    }

    fn render_debug_anchors(
        &self,
        camera: &Camera,
        canvas: &mut Canvas<Window>,
    ) -> Result<(), sdl3::Error> {
        let curr_color: Color = canvas.draw_color();

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

        canvas.set_draw_color(curr_color);
        Ok(())
    }

    fn generate_path_borders(&self, path_width: f32) -> (Vec<Coords>, Vec<Coords>) {
        let half_width: f32 = path_width / 2.0;
        let mut outer_border: Vec<Coords> = Vec::new();
        let mut inner_border: Vec<Coords> = Vec::new();

        // loop for every path point, and define a unit vector pointing in the direction of the path
        // at that exact point
        // use the point that comes just before and the one jsut after to make the vector

        let cyclic_path_points: Vec<&Coords> = std::iter::once(self.coords.last().unwrap())
            .chain(self.coords.iter())
            .chain(std::iter::once(self.coords.first().unwrap()))
            .collect();
        for (i, (prev, curr, next)) in cyclic_path_points
            .windows(3)
            .map(|w| (w[0], w[1], w[2]))
            .enumerate()
        {
            let direction: Vector = (*next - *prev).normalized();
            let orthogonal: Vector = direction.orthogonal();
            let outer_point: Coords = *curr + (-half_width) * orthogonal;
            let inner_point: Coords = *curr + half_width * orthogonal;

            outer_border.push(outer_point);

            // the inner point is only included if it is not too close to other path points
            let mut include_inner: bool = true;
            let points_per_curve: i32 = self.config.points_per_bezier_curve as i32;
            for j in (i as i32 - points_per_curve)..=(i as i32 + points_per_curve) {
                let idx: usize = j.rem_euclid(self.coords.len() as i32) as usize;
                let point: &Coords = &self.coords[idx];
                if *point != *curr && (*point - inner_point).norm().0 < half_width + 1e-4 {
                    include_inner = false;
                    break;
                }
            }
            include_inner.then(|| inner_border.push(inner_point));
        }

        (outer_border, inner_border)
    }
}

const SCREEN_WIDTH: u32 = 1200;
const SCREEN_HEIGHT: u32 = 900;

const GAME_FPS: f32 = 60.0;
const DT: f32 = GAME_FPS.recip();

pub fn main() {
    // Initialize SDL
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

    // Setup a game tick counter
    let mut game_tick_counter: u32 = 0;

    // Generate the circuit
    let seed: u64 = rand::random_range(0..u64::MAX);
    println!("Seed: {}", seed);
    let debug: bool = false;
    let path_config: BezierPathConfig = BezierPathConfig::default();
    let path: BezierPath = path_config.generate_path(Some(seed));
    let (outer_border, inner_border) = path.generate_path_borders(10.0);
    let starting_point: Coords = path.starting_point;
    let starting_direction: Vector = path.starting_direction;

    // Initialize a car
    let mut car: Car = Car::init(starting_point, starting_direction);

    // Initialize a camera
    let fix_camera: Camera = Camera {
        width: SCREEN_WIDTH,
        height: SCREEN_HEIGHT,
        coords: Coords::init(0., 0.),
        px_per_meter: 4,
    };

    let mut car_camera: Camera = Camera {
        width: SCREEN_WIDTH,
        height: SCREEN_HEIGHT,
        coords: car.coords,
        px_per_meter: (SCREEN_WIDTH as f32 / (car.length.0 / 0.05)).round() as u32,
        // car length is 5% of the screen width
    };

    // Initialize the TTF context for rendering text
    let ttf_context: sdl3::ttf::Sdl3TtfContext = sdl3::ttf::init().unwrap();
    let font: Font = ttf_context
        .load_font("assets/Basic-Regular.ttf", 25.)
        .unwrap();

    // Render text surface and texture for displaying instructions
    let hold_space_surf = font
        .render("HOLD SPACE TO FOLLOW CAR")
        .blended(Color::RGB(255, 255, 255))
        .map_err(|e: sdl3::ttf::FontError| e.to_string())
        .unwrap();
    let texture_creator = canvas.texture_creator();
    let hold_space_texture = texture_creator
        .create_texture_from_surface(&hold_space_surf)
        .map_err(|e| e.to_string())
        .unwrap();
    let hold_space_rect: Rect =
        sdl3::rect::Rect::new(10, 10, hold_space_surf.width(), hold_space_surf.height());

    let mut event_pump: sdl3::EventPump = sdl_context.event_pump().unwrap();

    'running: loop {
        // Read the events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => {}
            }
        }

        let keyboard_state = event_pump.keyboard_state();
        let space_pressed: bool =
            keyboard_state.is_scancode_pressed(sdl3::keyboard::Scancode::Space);

        // Update the car's position and rotation
        let slowed_counter: usize = (game_tick_counter / 5) as usize;
        car.coords = path.coords[slowed_counter % path.coords.len()];
        let direction: Vector = {
            let prev_index: usize =
                (slowed_counter as i32 - 1).rem_euclid(path.coords.len() as i32) as usize;
            let next_index: usize = (slowed_counter + 1) % path.coords.len();
            let prev_point: Coords = path.coords[prev_index];
            let next_point: Coords = path.coords[next_index];
            Vector::init(
                next_point.x.0 - prev_point.x.0,
                next_point.y.0 - prev_point.y.0,
            )
        }
        .normalized();
        car.direction_vec = direction;

        // Update cameras
        car_camera.coords = car.coords;

        // Start rendering
        let mut failed_rendering: bool = false;

        // Determine which camera to use for rendering
        let mut current_camera: &Camera = &fix_camera;
        if space_pressed {
            current_camera = &car_camera;
        }

        // Fill the entire screen with RGB(50, 50, 50)
        canvas.set_draw_color(Color::RGB(50, 50, 50));
        canvas.clear();

        // Draw the circuit
        canvas.set_draw_color(Color::RGB(200, 200, 200));
        if debug {
            failed_rendering |= path.render(current_camera, &mut canvas).is_err();
        }

        // Draw the anchors for debugging
        if debug {
            failed_rendering |= path
                .render_debug_anchors(current_camera, &mut canvas)
                .is_err();
        }

        // Draw the path borders for debugging
        failed_rendering |= render_coord_loop(&inner_border, current_camera, &mut canvas).is_err();
        failed_rendering |= render_coord_loop(&outer_border, current_camera, &mut canvas).is_err();

        // Render the car
        canvas.set_draw_color(Color::RGB(200, 0, 0));
        failed_rendering |= car.render(current_camera, &mut canvas).is_err();

        // Write "HOLD SPACE TO FOLLOW CAR"
        if !space_pressed {
            failed_rendering |= canvas
                .copy(&hold_space_texture, None, hold_space_rect)
                .is_err();
        }

        // Apply the changes on the screen
        canvas.present();

        // Detect failed rendering attempts
        if failed_rendering {
            eprintln!("Warning: Failed rendering one or more elements.");
        }

        // Control the frame rate
        std::thread::sleep(Duration::new(
            0,
            (1_000_000_000f32 / GAME_FPS).round() as u32,
        ));

        game_tick_counter += 1;
    }
}
