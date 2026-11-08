use core::f32;
use rand::{SeedableRng, rngs::StdRng};
use rand_distr::{Distribution, Normal};
use sdl3::{
    event::Event,
    joystick::Joystick,
    keyboard::{KeyboardState, Keycode, Scancode},
    pixels::{self, Color},
    rect,
    render::{self, Canvas, RenderTarget},
    surface::Surface,
    ttf,
    video::{Window, WindowContext},
};
use std::{
    collections::{HashMap, VecDeque, hash_map},
    hash::Hash,
    iter, ops,
    rc::Rc,
    time::{self, Duration},
};
use svg::{node::element::tag, parser};

#[derive(Clone, Copy, PartialOrd)]
struct Meter(f32);

impl PartialEq for Meter {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for Meter {}

impl Hash for Meter {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

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

impl Meter {
    fn as_px_dist(&self, camera: &Camera) -> f32 {
        self.0 * camera.px_per_meter as f32
    }
}

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
struct Coords {
    x: Meter,
    y: Meter,
}

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
struct Hashf32(u32);

impl Hashf32 {
    fn new(value: f32) -> Self {
        Self(value.to_bits())
    }

    fn value(&self) -> f32 {
        f32::from_bits(self.0)
    }
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

    fn as_fpoint(&self, camera: &Camera) -> render::FPoint {
        let px_coords: Vector = Vector {
            x: Meter((camera.px_per_meter as f32) * (self.x - camera.coords.x).0),
            y: Meter((camera.px_per_meter as f32) * (self.y - camera.coords.y).0),
        };
        render::FPoint::new(
            (camera.width / 2) as f32 + px_coords.x.0,
            (camera.height / 2) as f32 - px_coords.y.0,
        )
    }

    fn as_point(&self, camera: &Camera) -> rect::Point {
        let px_coords: Vector = (camera.px_per_meter as f32) * (*self - camera.coords);
        rect::Point::new(
            (camera.width / 2) as i32 + (px_coords.x.0).round() as i32,
            (camera.height / 2) as i32 - (px_coords.y.0).round() as i32,
        )
    }

    fn render(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), sdl3::Error> {
        canvas.draw_rect(rect::Rect::from_center(self.as_point(camera), 5, 5))
    }
}

impl Vector {
    fn new(x: f32, y: f32) -> Vector {
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

// TODO: Design a way to merge between the cacheable and quick version of different graphics
//       Consider seperating the Graphic trait and the Cacheable trait
//       Consider automating anchor definition (some system that will infer the anchor's position)
struct QuickGVector {
    base: Coords,
    vector: Vector,
    color: Color,
}

impl QuickGVector {
    fn new(base: Coords, vector: Vector, color: Color) -> Self {
        Self {
            base,
            vector,
            color,
        }
    }
}

impl QuickWorldRender for QuickGVector {
    fn render_onto(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), sdl3::Error> {
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

struct Camera {
    width: u32,
    height: u32,
    coords: Coords,
    px_per_meter: u32,
}

trait QuickWorldRender {
    // this trait is for world objects (depending on the camera and unit distance `Meter`) that
    // should be rendered quickly without creating a seperate texture
    // (since they won't be cached anyways)
    fn render_onto(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), sdl3::Error>;
}

#[derive(Clone, Hash, Eq, PartialEq)]
struct Polygon {
    vertices: Vec<Coords>,
}

impl Polygon {
    fn new(vertices: &[Coords]) -> Self {
        Self {
            vertices: vertices.to_vec(),
        }
    }

    fn width(&self) -> Meter {
        Meter(
            self.vertices.iter().map(|v| v.x.0).fold(f32::MIN, f32::max)
                - self.vertices.iter().map(|v| v.x.0).fold(f32::MAX, f32::min),
        )
    }

    fn height(&self) -> Meter {
        Meter(
            self.vertices.iter().map(|v| v.y.0).fold(f32::MIN, f32::max)
                - self.vertices.iter().map(|v| v.y.0).fold(f32::MAX, f32::min),
        )
    }

    fn contains(&self, point: Coords) -> bool {
        let mut inside: bool = false;
        let n: usize = self.vertices.len();
        let mut j: usize = n - 1;
        for i in 0..n {
            let pi: Coords = self.vertices[i];
            let pj: Coords = self.vertices[j];
            if ((pi.y.0 > point.y.0) != (pj.y.0 > point.y.0))
                && (point.x.0
                    < (pj.x.0 - pi.x.0) * (point.y.0 - pi.y.0) / (pj.y.0 - pi.y.0) + pi.x.0)
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }
}

struct QuickGPolygon {
    polygon: Polygon,
    filled: bool,
    color: Color,
}

impl QuickGPolygon {
    fn new(polygon: &Polygon, color: Color, filled: bool, _camera: &Camera) -> Self {
        Self {
            polygon: polygon.clone(),
            filled,
            color,
        }
    }
}

impl QuickWorldRender for QuickGPolygon {
    fn render_onto(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), sdl3::Error> {
        if !self.filled {
            let mut fpoints: Vec<render::FPoint> = self
                .polygon
                .vertices
                .iter()
                .map(|coord: &Coords| coord.as_fpoint(camera))
                .collect();
            fpoints.push(self.polygon.vertices[0].as_fpoint(camera));
            canvas.draw_lines(fpoints.as_slice())
        } else {
            canvas.set_draw_color(self.color);

            let fpoints: Vec<render::FPoint> = self
                .polygon
                .vertices
                .iter()
                .map(|coord| coord.as_fpoint(camera))
                .collect();

            if fpoints.len() < 3 {
                return Ok(());
            }

            // Get canvas size for clipping bounds
            let (canvas_w, canvas_h) = canvas.output_size()?;

            // Find vertical bounds clamped to screen dimensions
            let y_min = fpoints
                .iter()
                .map(|p| p.y)
                .fold(f32::MAX, f32::min)
                .max(0.0) as i32;
            let y_max = fpoints
                .iter()
                .map(|p| p.y)
                .fold(f32::MIN, f32::max)
                .min(canvas_h as f32 - 1.0) as i32;

            if y_min > y_max {
                return Ok(());
            }

            let n = fpoints.len();

            // Process row by row (scanline)
            for y in y_min..=y_max {
                let y_f = y as f32;
                let mut intersections = Vec::new();

                // Find all edge intersections with the current scanline y
                let mut j = n - 1;
                for i in 0..n {
                    let p1 = fpoints[j];
                    let p2 = fpoints[i];

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

struct Car {
    coords: Coords,
    direction_vec: Vector,
    length: Meter,
    width: Meter,
    engine_acc: Vector,
    velocity: Vector,
    friction_factor: f32,
    car_polys: Vec<(Polygon, Color)>,
}

impl Car {
    fn init(coords: Coords, direction: Vector) -> Car {
        let mut car: Car = Car {
            coords,
            direction_vec: direction.normalized(),
            length: Meter(5.),
            width: Meter(2.),
            engine_acc: Vector::new(0., 0.),
            velocity: Vector::new(0., 0.),
            friction_factor: FRICTION_FACTOR,
            car_polys: Vec::new(),
        };
        car.car_polys = car.load_svg();
        car
    }

    fn corner_coords(&self) -> [Coords; 4] {
        [(1., 1.), (-1., 1.), (-1., -1.), (1., -1.)].map(|(d1, d2)| {
            self.coords
                + (d1 * (self.length.0 / 2.)) * self.direction_vec
                + (d2 * (self.width.0 / 2.)) * self.direction_vec.orthogonal()
        })
    }

    fn as_rect_polygon(&self) -> Polygon {
        Polygon::new(self.corner_coords().as_slice())
    }

    fn load_svg(&self) -> Vec<(Polygon, Color)> {
        let svg_path: &str = "assets/f1_car.svg";
        println!("Loading F1 car SVG from {}", svg_path);

        let mut polygons: Vec<(Polygon, Color)> = Vec::new();

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
                let polygon: Polygon = Polygon::new(
                    &points
                        .split_whitespace()
                        .map(|pair| {
                            let mut iter = pair.split(',');
                            (iter.next().unwrap(), iter.next().unwrap())
                        })
                        .filter_map(|(s1, s2)| {
                            let x = s1.parse::<f32>().ok()?;
                            let y = s2.parse::<f32>().ok()?;
                            Some(Coords {
                                x: Meter(self.width.0 * x / 200. - self.width.0 / 2.),
                                y: Meter(self.length.0 * y / 500. - self.length.0 / 2.),
                            })
                        })
                        .collect::<Vec<Coords>>(),
                );
                polygons.push((polygon, color));
            }
        }

        polygons
    }

    fn render_rect(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), sdl3::Error> {
        let polygon = self.as_rect_polygon();
        QuickGPolygon::new(&polygon, Color::RGB(255, 0, 0), false, camera)
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
            for vertex in &mut polygon.vertices {
                // rotate around origin to match car's direction vector
                let angle = self.direction_vec.x.0.atan2(self.direction_vec.y.0);
                let cos_angle = angle.cos();
                let sin_angle = angle.sin();
                let rotated_x = cos_angle * vertex.x.0 - sin_angle * vertex.y.0;
                let rotated_y = sin_angle * vertex.x.0 + cos_angle * vertex.y.0;
                *vertex = Coords {
                    x: Meter(rotated_x),
                    y: Meter(-rotated_y),
                };
                *vertex = self.coords + (*vertex - Coords::init(0., 0.));
            }
            canvas.set_draw_color(color);
            QuickGPolygon::new(&polygon, color, true, camera).render_onto(camera, canvas)?;
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
        let base_point: Coords = self.coords;
        let friction_acc = -self.friction_factor * self.velocity;
        let gvectors = [
            (
                QuickGVector::new(
                    base_point,
                    debug_scalar * self.velocity,
                    Color::RGB(255, 0, 0),
                ),
                "v",
            ),
            (
                QuickGVector::new(
                    base_point,
                    debug_scalar.powi(2) * self.engine_acc,
                    Color::RGB(0, 0, 255),
                ),
                "p",
            ),
            (
                QuickGVector::new(
                    base_point,
                    debug_scalar.powi(2) * friction_acc,
                    Color::RGB(0, 255, 0),
                ),
                "f",
            ),
            (
                QuickGVector::new(
                    base_point,
                    debug_scalar.powi(2) * (self.engine_acc + friction_acc),
                    Color::RGB(255, 255, 0),
                ),
                "a",
            ),
        ];

        for (gvector, label) in gvectors {
            // TODO: Implement label rendering
            gvector.render_onto(camera, canvas)?;
        }
        Ok(())
    }

    fn set_engine_acc(&mut self, acc: Vector) {
        self.engine_acc = acc
    }

    fn set_friction_factor(&mut self, friction_factor: f32) {
        self.friction_factor = friction_factor;
    }

    fn update_coords(&mut self, dt: f32) {
        let friction_acc = -self.friction_factor * self.velocity;
        self.velocity = self.velocity + dt * self.engine_acc + dt * friction_acc;
        let velocity_norm: f32 = self.velocity.norm().0;
        if velocity_norm > 1e-4 {
            self.direction_vec = (1. / velocity_norm) * self.velocity;
        }
        self.coords = self.coords + dt * self.velocity;
    }
}

#[derive(Clone)]
struct BezierPathConfig {
    center: Coords,
    n_anchors: usize,
    mean_anchor_radius: f32,
    stdev_anchor_radius: f32,
    stdev_subanchor_angle_dev: f32,
    points_per_bezier_curve: usize,
}

impl BezierPathConfig {
    fn init_circular() -> Self {
        BezierPathConfig {
            center: Coords::init(0., 0.),
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
            center: Coords::init(0., 0.),
            n_anchors: N_ANCHORS,
            mean_anchor_radius: MEAN_ANCHOR_RADIUS,
            stdev_anchor_radius: STDEV_ANCHOR_RADIUS,
            stdev_subanchor_angle_dev: STDEV_SUBANCHOR_ANGLE_DEV,
            points_per_bezier_curve: POINTS_PER_BEZIER_CURVE,
        }
    }
}

struct BezierPath {
    coords: Vec<Coords>,
    starting_point: Coords,
    starting_direction: Vector,
    config: BezierPathConfig,          // for debugging
    anchor_triplets: Vec<[Coords; 3]>, // for debugging
}

impl BezierPathConfig {
    fn generate_path(&self, seed: Option<u64>) -> BezierPath {
        fn sample_anchors(config: &BezierPathConfig, rng: &mut StdRng) -> Vec<Coords> {
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

                let track_dir_vector: Vector = Vector::new(new_angle.cos(), new_angle.sin());

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
    fn render<'tc, T: RenderTarget, U>(
        &self,
        camera: &Camera,
        canvas: &mut Canvas<T>,
        texture_creator: &'tc render::TextureCreator<U>,
        texture_cache: &mut Option<TextureCache<'tc>>,
    ) -> Result<(), CacheableGraphicError> {
        CacheableGPolygon::new(
            &Polygon::new(&self.coords),
            Color::RGB(255, 255, 255),
            false,
            camera,
        )
        .draw_onto(
            canvas,
            texture_creator,
            texture_cache,
            DrawPosition::RealCoords { camera },
        )
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

    fn generate_path_borders(&self, path_width: f32) -> (Vec<Coords>, Vec<Coords>) {
        let half_width: f32 = path_width / 2.0;
        let mut outer_border: Vec<Coords> = Vec::new();
        let mut inner_border: Vec<Option<Coords>> = Vec::new();

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
            let mut inner_point: Option<Coords> = Some(*curr + half_width * orthogonal);

            outer_border.push(outer_point);

            // the inner point is only included if it is not too close to other path points
            let points_per_curve: i32 = self.config.points_per_bezier_curve as i32;
            for j in (i as i32 - points_per_curve)..=(i as i32 + points_per_curve) {
                let idx: usize = j.rem_euclid(self.coords.len() as i32) as usize;
                let point: &Coords = &self.coords[idx];
                if *point != *curr && (*point - inner_point.unwrap()).norm().0 < half_width * 1.001
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

#[derive(Clone, Hash, Eq, PartialEq)]
enum HashableGraphicProps {
    Circle {
        radius: Hashf32,
        color: Color,
    },
    Disc {
        radius: Hashf32,
        color: Color,
    },
    Vector {
        x: Hashf32,
        y: Hashf32,
        color: Option<Color>,
        label: Option<String>,
    },
    Polygon {
        polygon: Polygon,
        px_per_meter: u32,
        color: Color,
    },
    Text {
        text: String,
        font_name: String,
        color: Color,
    },
}

// TODO: Make CacheableGraphic generic over HashableGraphicProps
// Suggestion: Implement cacheable and non-cacheable graphics, then another structs that is made of
// elementary cacheable and non-cacheable graphics.
trait CacheableGraphic {
    fn hashable_properties(&self) -> HashableGraphicProps;
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn pixel_anchor(&self) -> (i32, i32);
    fn world_anchor(&self) -> Option<Coords>;
    // only implement `world_anchor` for graphics meant to be positioned based on world coordinates

    // TODO: Leverage Rust's type system to enforce that world_anchor is only implemented for
    //       those graphics

    fn render_texture<'tc, T>(
        &self,
        texture_creator: &'tc render::TextureCreator<T>,
    ) -> Result<render::Texture<'tc>, CacheableGraphicError>;

    fn get_texture<'tc, T>(
        &self,
        texture_creator: &'tc render::TextureCreator<T>,
        texture_cache: &mut Option<TextureCache<'tc>>,
    ) -> Result<Rc<render::Texture<'tc>>, CacheableGraphicError> {
        // used pattern matching on Entry::Vacant to avoid clippy's map_entry warning
        let shape = self.hashable_properties();

        match texture_cache
            .as_mut()
            .map(|tc| tc.texture_cache.entry(shape))
        {
            Some(hash_map::Entry::Occupied(entry)) => Ok(Rc::clone(entry.get())),
            other => {
                let texture = self.render_texture(texture_creator)?;

                if let Some(hash_map::Entry::Vacant(entry)) = other {
                    let rc_texture = Rc::new(texture);
                    entry.insert(Rc::clone(&rc_texture));
                    return Ok(rc_texture);
                }

                Ok(Rc::new(texture))
            }
        }
    }

    fn get_rect(&self, position: DrawPosition) -> Result<rect::Rect, CacheableGraphicError> {
        let mut rect = rect::Rect::new(0, 0, self.width(), self.height());

        let (anchor_x, anchor_y) = self.pixel_anchor();
        match position {
            DrawPosition::ScreenCoords { x, y } => {
                rect.set_x(x - anchor_x);
                rect.set_y(y - anchor_y);
            }
            DrawPosition::RealCoords { camera } => {
                // TODO : Leverage Rust types to avoid the panic completely
                let fpoint_of_world_anchor: rect::Point = self
                    .world_anchor()
                    .unwrap_or_else(|| {
                        panic!("A world anchor was expected for a non-world-base graphic")
                    })
                    .as_point(camera);
                let screen_x: i32 = fpoint_of_world_anchor.x();
                let screen_y: i32 = fpoint_of_world_anchor.y();
                rect.set_x(screen_x - anchor_x);
                rect.set_y(screen_y - anchor_y);
            }
        }

        Ok(rect)
    }

    fn draw_onto<'tc, T: RenderTarget, U>(
        &self,
        canvas: &mut Canvas<T>,
        texture_creator: &'tc render::TextureCreator<U>,
        texture_cache: &mut Option<TextureCache<'tc>>,
        position: DrawPosition,
    ) -> Result<(), CacheableGraphicError> {
        if self.width() == 0 || self.height() == 0 {
            return Ok(());
        }
        let texture = self.get_texture(texture_creator, texture_cache)?;
        let rect = self.get_rect(position)?;
        canvas
            .copy(&texture, None, rect)
            .map_err(CacheableGraphicError::TextureCopy)?;
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Debug)]
enum CacheableGraphicError {
    SurfaceCreation(sdl3::Error),
    SurfaceToCanvas(sdl3::Error),
    CanvasRender(sdl3::Error),
    TextureCreation(render::TextureValueError),
    TextureCopy(sdl3::Error),
    Other(String),
}

struct Circle {
    radius: f32,
    color: Color,
}

impl CacheableGraphic for Circle {
    fn hashable_properties(&self) -> HashableGraphicProps {
        HashableGraphicProps::Circle {
            radius: Hashf32::new(self.radius),
            color: self.color,
        }
    }

    fn width(&self) -> u32 {
        self.radius.ceil() as u32 * 2 // TODO: Debug missing last line & row form texture
    }

    fn height(&self) -> u32 {
        self.radius.ceil() as u32 * 2
    }

    fn pixel_anchor(&self) -> (i32, i32) {
        (self.radius.ceil() as i32, self.radius.ceil() as i32)
    }

    fn world_anchor(&self) -> Option<Coords> {
        None
    }

    fn render_texture<'tc, T>(
        &self,
        texture_creator: &'tc render::TextureCreator<T>,
    ) -> Result<render::Texture<'tc>, CacheableGraphicError> {
        let mut canvas = Surface::new(self.width(), self.height(), pixels::PixelFormat::RGBA32)
            .map_err(CacheableGraphicError::SurfaceCreation)?
            .into_canvas()
            .map_err(CacheableGraphicError::SurfaceToCanvas)?;

        canvas.set_draw_color(self.color);

        let radius: f32 = self.radius;
        for i in 0..36 {
            let theta1: f32 = (i as f32) * 2.0 * std::f32::consts::PI / 36.;
            let theta2: f32 = ((i + 1) as f32) * 2.0 * std::f32::consts::PI / 36.;
            let x1: f32 = radius * theta1.cos() + radius;
            let y1: f32 = radius * theta1.sin() + radius;
            let x2: f32 = radius * theta2.cos() + radius;
            let y2: f32 = radius * theta2.sin() + radius;
            canvas
                .draw_line((x1, y1), (x2, y2))
                .map_err(CacheableGraphicError::CanvasRender)?;
        }

        let texture = texture_creator
            .create_texture_from_surface(canvas.into_surface())
            .map_err(CacheableGraphicError::TextureCreation)?;

        Ok(texture)
    }
}

struct Disc {
    radius: f32,
    color: Color,
}

impl CacheableGraphic for Disc {
    fn hashable_properties(&self) -> HashableGraphicProps {
        HashableGraphicProps::Disc {
            radius: Hashf32::new(self.radius),
            color: self.color,
        }
    }

    fn width(&self) -> u32 {
        self.radius.ceil() as u32 * 2
    }

    fn height(&self) -> u32 {
        self.radius.ceil() as u32 * 2
    }

    fn pixel_anchor(&self) -> (i32, i32) {
        (self.radius.ceil() as i32, self.radius.ceil() as i32)
    }

    fn world_anchor(&self) -> Option<Coords> {
        None
    }

    fn render_texture<'tc, T>(
        &self,
        texture_creator: &'tc render::TextureCreator<T>,
    ) -> Result<render::Texture<'tc>, CacheableGraphicError> {
        let mut canvas = Surface::new(self.width(), self.height(), pixels::PixelFormat::RGBA32)
            .map_err(CacheableGraphicError::SurfaceCreation)?
            .into_canvas()
            .map_err(CacheableGraphicError::SurfaceToCanvas)?;

        canvas.set_draw_color(self.color);

        let radius: f32 = self.radius;
        for i in -(radius as i16)..=(radius as i16) {
            let y_offset = (radius * radius - (i as f32) * (i as f32)).sqrt() as i16;
            for j in -y_offset..=y_offset {
                canvas
                    .draw_point((radius + i as f32, radius + j as f32))
                    .map_err(CacheableGraphicError::CanvasRender)?;
            }
        }

        let texture = texture_creator
            .create_texture_from_surface(canvas.into_surface())
            .map_err(CacheableGraphicError::TextureCreation)?;

        Ok(texture)
    }
}

struct CacheableGVector {
    x: f32,
    y: f32,
    base_point: Coords,
    color: Option<Color>,
    label: Option<String>,
}

impl CacheableGVector {
    fn new(
        vec: &Vector,
        base_point: Coords,
        camera: &Camera,
        color: Option<Color>,
        label: Option<&str>,
    ) -> Self {
        Self {
            x: vec.x.as_px_dist(camera).round(),
            y: -vec.y.as_px_dist(camera).round(),
            base_point,
            color,
            label: label.map(|s| s.to_string()),
        }
    }

    fn norm(&self) -> f32 {
        (self.x.powi(2) + self.y.powi(2)).sqrt()
    }
}

impl CacheableGraphic for CacheableGVector {
    fn hashable_properties(&self) -> HashableGraphicProps {
        HashableGraphicProps::Vector {
            x: Hashf32::new(self.x),
            y: Hashf32::new(self.y),
            color: self.color,
            label: self.label.clone(),
            // TODO: find a way to seperate the label and the vector graphic itself to avoid hashing
            //       a vector two times when only the label changes
            // A suggestion is to seperate between elementary and non-elementary graphics
        }
    }

    fn width(&self) -> u32 {
        (2.0 * self.norm().ceil()) as u32
    }

    fn height(&self) -> u32 {
        (2.0 * self.norm().ceil()) as u32
    }

    fn pixel_anchor(&self) -> (i32, i32) {
        (self.width() as i32 / 2, self.height() as i32 / 2)
    }

    fn world_anchor(&self) -> Option<Coords> {
        Some(self.base_point)
    }

    fn render_texture<'tc, T>(
        &self,
        texture_creator: &'tc render::TextureCreator<T>,
    ) -> Result<render::Texture<'tc>, CacheableGraphicError> {
        let mut canvas = Surface::new(self.width(), self.height(), pixels::PixelFormat::RGBA32)
            .map_err(CacheableGraphicError::SurfaceCreation)?
            .into_canvas()
            .map_err(CacheableGraphicError::SurfaceToCanvas)?;

        let (start_x, start_y) = (self.width() as i32 / 2, self.height() as i32 / 2);
        let end_x = start_x + self.x.ceil() as i32;
        let end_y = start_y + self.y.ceil() as i32;

        let start_point = rect::Point::new(start_x, start_y);
        let end_point = rect::Point::new(end_x, end_y);

        if let Some(color) = self.color {
            canvas.set_draw_color(color);
        }

        canvas
            .draw_line(start_point, end_point)
            .map_err(CacheableGraphicError::CanvasRender)?;

        // TODO: Simplify this spaghetti by implementing a vector type with rotation and scaling
        //       operations, this struct should be different from the IRL Vector previously
        //       implemented.
        let tip_size = self.x.abs().max(self.y.abs()) * 0.25;

        let theta = std::f32::consts::PI / 6.0; // 30 degrees for the arrow tip
        let vec: (i32, i32) = (start_x - end_x, start_y - end_y);

        let vec_length = (vec.0.pow(2) + vec.1.pow(2)) as f32;
        let vec_length = vec_length.sqrt();

        if vec_length > 1e-6 {
            let vec_rotated1 = (
                vec.0 as f32 * theta.cos() - vec.1 as f32 * theta.sin(),
                vec.0 as f32 * theta.sin() + vec.1 as f32 * theta.cos(),
            );
            let vec_rotated2 = (
                vec.0 as f32 * theta.cos() + vec.1 as f32 * theta.sin(),
                -(vec.0 as f32) * theta.sin() + vec.1 as f32 * theta.cos(),
            );

            let vec_rotated1 = (
                vec_rotated1.0 * tip_size / vec_length,
                vec_rotated1.1 * tip_size / vec_length,
            );
            let vec_rotated2 = (
                vec_rotated2.0 * tip_size / vec_length,
                vec_rotated2.1 * tip_size / vec_length,
            );

            let tip_point1 = rect::Point::new(
                (end_x as f32 + vec_rotated1.0) as i32,
                (end_y as f32 + vec_rotated1.1) as i32,
            );
            let tip_point2 = rect::Point::new(
                (end_x as f32 + vec_rotated2.0) as i32,
                (end_y as f32 + vec_rotated2.1) as i32,
            );

            canvas
                .draw_line(end_point, tip_point1)
                .map_err(CacheableGraphicError::CanvasRender)?;
            canvas
                .draw_line(end_point, tip_point2)
                .map_err(CacheableGraphicError::CanvasRender)?;
        }
        let texture = texture_creator
            .create_texture_from_surface(canvas.into_surface())
            .map_err(CacheableGraphicError::TextureCreation)?;
        Ok(texture)
    }
}

struct CacheableGPolygon {
    fpoints: Vec<render::FPoint>,
    anchor: rect::Point,
    color: Color,
    filled: bool,

    // for hashing
    polygon: Polygon,
    px_per_meter: u32,
}

impl CacheableGPolygon {
    fn new(polygon: &Polygon, color: Color, filled: bool, camera: &Camera) -> Self {
        // TODO : Cleanup this spaghetti code (or just leave it cuz it works!!)

        // the pixel corresponding to the world coordinate (0, 0)
        let origin_pixel = Coords::init(0., 0.).as_point(camera);

        // compute the anchor pixel
        let min_x = polygon
            .vertices
            .iter()
            .map(|coords| coords.x)
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        let max_y = polygon
            .vertices
            .iter()
            .map(|coords| coords.y)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        let top_left_pixel_coords = Coords::init(min_x.0, max_y.0);
        let top_left_pixel = top_left_pixel_coords.as_fpoint(camera);

        let anchor_pixel = rect::Point::new(
            origin_pixel.x - top_left_pixel.x as i32,
            origin_pixel.y - top_left_pixel.y as i32,
        );

        // define the vectors of pixels corresponding to the polygon vertices coords
        let mut fpoints: Vec<render::FPoint> = polygon
            .vertices
            .iter()
            .map(|coords| coords.as_fpoint(camera))
            // .map(|p| render::FPoint::new(p.x, p.y))
            .map(|p| render::FPoint::new(p.x + anchor_pixel.x as f32, p.y + anchor_pixel.y as f32)) // substract anchor point
            .collect();

        let min_fpoint_x = fpoints.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let min_fpoint_y = fpoints.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);

        for p in &mut fpoints {
            p.x -= min_fpoint_x;
            p.y -= min_fpoint_y;
        }

        // return the object
        Self {
            polygon: polygon.clone(),
            px_per_meter: camera.px_per_meter,
            fpoints,
            anchor: anchor_pixel,
            color,
            filled,
        }
    }
}

impl CacheableGraphic for CacheableGPolygon {
    fn hashable_properties(&self) -> HashableGraphicProps {
        HashableGraphicProps::Polygon {
            px_per_meter: self.px_per_meter,
            polygon: self.polygon.clone(),
            color: self.color,
        }
    }

    fn width(&self) -> u32 {
        let max = self
            .fpoints
            .iter()
            .map(|p| p.x.ceil() as i32)
            .max()
            .unwrap();
        let min = self
            .fpoints
            .iter()
            .map(|p| p.x.floor() as i32)
            .min()
            .unwrap();
        (max - min) as u32
    }

    fn height(&self) -> u32 {
        let max = self
            .fpoints
            .iter()
            .map(|p| p.y.ceil() as i32)
            .max()
            .unwrap();
        let min = self
            .fpoints
            .iter()
            .map(|p| p.y.floor() as i32)
            .min()
            .unwrap();
        (max - min) as u32
    }

    fn pixel_anchor(&self) -> (i32, i32) {
        let anchor_x = self.anchor.x;
        let anchor_y = self.anchor.y;
        (anchor_x, anchor_y)
    }

    fn world_anchor(&self) -> Option<Coords> {
        Some(Coords::init(0., 0.))
    }

    fn render_texture<'tc, T>(
        &self,
        texture_creator: &'tc sdl3::render::TextureCreator<T>,
    ) -> Result<sdl3::render::Texture<'tc>, CacheableGraphicError> {
        fn render_filled<T: RenderTarget>(
            gpolygon: &CacheableGPolygon,
            canvas: &mut Canvas<T>,
        ) -> Result<(), sdl3::Error> {
            // Get canvas size for clipping bounds
            let (canvas_w, canvas_h) = canvas.output_size()?;

            // Find vertical bounds clamped to screen dimensions
            let y_min = gpolygon
                .fpoints
                .iter()
                .map(|p| p.y)
                .fold(f32::MAX, f32::min)
                .max(0.0) as i32;
            let y_max = gpolygon
                .fpoints
                .iter()
                .map(|p| p.y)
                .fold(f32::MIN, f32::max)
                .min(canvas_h as f32 - 1.0) as i32;

            if y_min > y_max {
                return Ok(());
            }

            let n = gpolygon.fpoints.len();

            // Process row by row (scanline)
            for y in y_min..=y_max {
                let y_f = y as f32;
                let mut intersections = Vec::new();

                // Find all edge intersections with the current scanline y
                let mut j = n - 1;
                for i in 0..n {
                    let p1 = gpolygon.fpoints[j];
                    let p2 = gpolygon.fpoints[i];

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
                            render::FPoint::new(x_start as f32, y as f32),
                            render::FPoint::new(x_end as f32, y as f32),
                        )?;
                    }
                    i += 2;
                }
            }
            Ok(())
        }

        fn render_outline<T: RenderTarget>(
            gpolygon: &CacheableGPolygon,
            canvas: &mut Canvas<T>,
        ) -> Result<(), sdl3::Error> {
            canvas.set_draw_color(gpolygon.color);

            let mut fpoints: Vec<render::FPoint> = gpolygon.fpoints.clone();
            fpoints.push(gpolygon.fpoints[0]);
            canvas.draw_lines(fpoints.as_slice())?;
            Ok(())
        }

        let mut canvas = Surface::new(self.width(), self.height(), pixels::PixelFormat::RGBA32)
            .map_err(CacheableGraphicError::SurfaceCreation)?
            .into_canvas()
            .map_err(CacheableGraphicError::SurfaceToCanvas)?;

        if self.fpoints.len() > 2 {
            canvas.set_draw_color(self.color);

            if self.filled {
                render_filled(self, &mut canvas).map_err(CacheableGraphicError::CanvasRender)?;
            } else {
                render_outline(self, &mut canvas).map_err(CacheableGraphicError::CanvasRender)?;
            }
        }

        let texture = texture_creator
            .create_texture_from_surface(canvas.into_surface())
            .map_err(CacheableGraphicError::TextureCreation)?;
        Ok(texture)
    }
}

// TODO: Consider including the texture creator in the struct...
//       ...if not doing so can cause an invalid texture render (when working with multiple canvas)
struct TextureCache<'tc> {
    texture_cache: HashMap<HashableGraphicProps, Rc<render::Texture<'tc>>>,
}

impl<'tc> TextureCache<'tc> {
    fn init() -> TextureCache<'tc> {
        Self {
            texture_cache: HashMap::new(),
        }
    }
}

// TODO: Make this a cacheable graphic, don't forget to add the color as a property.
struct TextFont<'tc> {
    font: ttf::Font<'tc>,
    ttf_name: String,
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

impl<'tc> TextFont<'tc> {
    fn init(font: ttf::Font<'tc>, font_name: &str) -> Self {
        Self {
            font,
            ttf_name: font_name.to_string(),
        }
    }
}

struct CacheableText<'ft, 'tc> {
    text: String,
    font: &'ft TextFont<'tc>,
    color: Color,
}

impl<'ft, 'tc> CacheableText<'ft, 'tc> {
    fn new(text: &str, font: &'ft TextFont<'tc>, color: Color) -> Self {
        Self {
            text: text.to_string(),
            font,
            color,
        }
    }
}

impl<'ft, 'tc> CacheableGraphic for CacheableText<'ft, 'tc> {
    fn hashable_properties(&self) -> HashableGraphicProps {
        HashableGraphicProps::Text {
            text: self.text.clone(),
            font_name: self.font.ttf_name.clone(),
            color: self.color,
        }
    }

    fn width(&self) -> u32 {
        self.font.font.size_of(&self.text).unwrap().0
    }

    fn height(&self) -> u32 {
        self.font.font.size_of(&self.text).unwrap().1
    }

    fn pixel_anchor(&self) -> (i32, i32) {
        (0, 0)
    }

    fn world_anchor(&self) -> Option<Coords> {
        None
    }

    fn render_texture<'txc, T>(
        &self,
        texture_creator: &'txc render::TextureCreator<T>,
    ) -> Result<render::Texture<'txc>, CacheableGraphicError> {
        let surface = self
            .font
            .font
            .render(&self.text)
            .blended(self.color)
            .map_err(|e| CacheableGraphicError::Other(e.to_string()))?;
        let texture = texture_creator
            .create_texture_from_surface(&surface)
            .map_err(CacheableGraphicError::TextureCreation)?;
        Ok(texture)
    }
}

struct SideJoystickGUI {
    position: (i16, i16),
    is_clicked: bool,
    direction: Vector,
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
            direction: Vector::new(0.0, 0.0),
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
            self.direction = (1. / max_reach) * Vector::new(dx, -dy);
            if self.direction.norm().0 > 1.0 {
                self.direction = self.direction.normalized();
            }
        } else {
            self.direction = Vector::new(0.0, 0.0);
        }
    }

    fn render<'tc, T: RenderTarget, U>(
        &self,
        canvas: &mut Canvas<T>,
        texture_creator: &'tc render::TextureCreator<U>,
        texture_cache: &mut Option<TextureCache<'tc>>,
    ) -> Result<(), CacheableGraphicError> {
        let (center_x, center_y) = self.position;
        Circle {
            radius: self.outer_radius,
            color: Color::RGB(200, 200, 200),
        }
        .draw_onto(
            canvas,
            texture_creator,
            texture_cache,
            DrawPosition::ScreenCoords {
                x: center_x as i32,
                y: center_y as i32,
            },
        )?;

        let max_reach = self.outer_radius - 0.8 * self.inner_radius;
        let pos_x: f32 = center_x as f32 + self.direction.x.0 * max_reach;
        let pos_y: f32 = center_y as f32 - self.direction.y.0 * max_reach;

        Disc {
            radius: self.inner_radius,
            color: if self.is_clicked {
                Color::RGB(125, 125, 125)
            } else {
                Color::RGB(175, 175, 175)
            },
        }
        .draw_onto(
            canvas,
            texture_creator,
            texture_cache,
            DrawPosition::ScreenCoords {
                x: pos_x as i32,
                y: pos_y as i32,
            },
        )?;
        Ok(())
    }
}

enum PlayerControl {
    Gamepad(Joystick),
    SideJoystick(SideJoystickGUI),
    CenteredJoystick { button: Scancode },
    RustFunction(fn(&Car, &Track) -> Vector),
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
    outer_border: Polygon,
    inner_border: Polygon,
    circuit_polygon: Polygon,
    bezier_path: BezierPath,
    starting_point: Coords,
    starting_direction: Vector,
}

impl Track {
    fn from_bezier(bezier_path: BezierPath, path_width: f32) -> Self {
        let (outer_border, inner_border) = bezier_path.generate_path_borders(path_width);
        let starting_point: Coords = bezier_path.starting_point;
        let starting_direction: Vector = bezier_path.starting_direction;
        let inner_border = Polygon::new(
            inner_border
                .iter()
                .chain(iter::once(inner_border.first().unwrap()))
                .copied()
                .collect::<Vec<Coords>>()
                .as_slice(),
        );
        let outer_border = Polygon::new(
            outer_border
                .iter()
                .chain(iter::once(outer_border.first().unwrap()))
                .copied()
                .collect::<Vec<Coords>>()
                .as_slice(),
        );
        let circuit_polygon: Polygon = Polygon::new(
            inner_border
                .vertices
                .iter()
                .chain(outer_border.vertices.iter().rev())
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
                let joystick_direction = Vector::new(axis_0, -axis_1);
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
                direction: Vector::new(0., 0.),
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
                let engine_acc_vec = if joystick_direction.norm().0 < 0.2 {
                    Vector::new(0.0, 0.0)
                } else {
                    joystick_direction.normalized()
                };
                car.set_engine_acc(ENGINE_ACCELERATION * engine_acc_vec);
            } else {
                car.set_engine_acc(Vector::new(0.0, 0.0));
            }

            // Check if any of the 4 corners of the car rect is outside the circuit polygon
            let mut friction_multiplier: f32 = 2.0;
            let car_corners: [Coords; 4] = car.corner_coords();
            let mut is_car_inside: bool = true;
            for corner in &car_corners {
                if !self.track.circuit_polygon.contains(*corner) {
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
    direction: Vector,
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
    let mut car_camera: Camera = Camera {
        width: CAMERA_WIDTH,
        height: CAMERA_HEIGHT,
        coords: game.players[0].car.coords,
        px_per_meter: (CAMERA_WIDTH as f32 / (game.players[0].car.length.0 / 0.04)).round() as u32,
        // car length is 4% of the camera width
    };

    let followed_car_index: usize = 0;

    let fix_camera: Camera = Camera {
        width: CAMERA_WIDTH,
        height: CAMERA_HEIGHT,
        coords: Coords::init(0., 0.),
        px_per_meter: 2,
    };

    let mut current_camera: usize = 0; // 0 for car_camera, 1 for fix_camera

    // Initialize Font
    let ttf_context: ttf::Sdl3TtfContext = ttf::init().unwrap();
    let text_font: TextFont = TextFont::init(
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
    let mut texture_cache: Option<TextureCache> = Some(TextureCache::init());

    // Render text textures
    let hold_space_text = CacheableText::new(
        "PRESS ENTER TO TOGGLE CAMERAS",
        &text_font,
        Color::RGB(255, 255, 255),
    );

    let js_name_str = if let PlayerControl::Gamepad(js) = &game.players[0].profile.control {
        format!("Controller: {}", js.name())
    } else {
        "No controller detected at startup".to_string()
    };

    let js_name_text = CacheableText::new(&js_name_str, &text_font, Color::RGB(255, 255, 255));

    let seed_text = CacheableText::new(
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
        car_camera.coords = game.players[followed_car_index].car.coords;

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
        failed_rendering |= CacheableGPolygon::new(
            &game.track.inner_border,
            Color::RGB(10, 70, 10),
            true,
            current_camera,
        )
        .draw_onto(
            &mut canvas,
            &texture_creator,
            &mut texture_cache,
            DrawPosition::RealCoords {
                camera: current_camera,
            },
        )
        .map_err(|e| eprintln!("Failed to render the enclosed area: {:?}", e))
        .is_err();

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        failed_rendering |= CacheableGPolygon::new(
            &game.track.circuit_polygon,
            Color::RGB(80, 80, 80),
            true,
            current_camera,
        )
        .draw_onto(
            &mut canvas,
            &texture_creator,
            &mut texture_cache,
            DrawPosition::RealCoords {
                camera: current_camera,
            },
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
            .draw_onto(
                &mut canvas,
                &texture_creator,
                &mut texture_cache,
                DrawPosition::ScreenCoords {
                    x: 10,
                    y: SCREEN_HEIGHT as i32 - hold_space_text.height() as i32 - 10,
                },
            )
            .map_err(|e| eprintln!("Failed to render the hold space text: {:?}", e))
            .is_err();

        failed_rendering |= js_name_text
            .draw_onto(
                &mut canvas,
                &texture_creator,
                &mut texture_cache,
                DrawPosition::ScreenCoords {
                    x: SCREEN_WIDTH as i32 - js_name_text.width() as i32 - 10,
                    y: 10,
                },
            )
            .map_err(|e| eprintln!("Failed to render the joystick name text: {:?}", e))
            .is_err();

        failed_rendering |= seed_text
            .draw_onto(
                &mut canvas,
                &texture_creator,
                &mut texture_cache,
                DrawPosition::ScreenCoords { x: 10, y: 10 },
            )
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
