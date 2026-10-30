use core::f32;
use rand::{SeedableRng, rngs::StdRng};
use rand_distr::{Distribution, Normal};
use sdl3::{
    event::Event,
    joystick::Joystick,
    keyboard::{KeyboardState, Keycode, Scancode},
    mouse::MouseButton,
    pixels::Color,
    rect,
    render::{self, Canvas},
    ttf,
    video::{Window, WindowContext},
};
use std::{
    collections::VecDeque,
    iter, ops,
    time::{self, Duration},
};

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

    fn as_fpoint(&self, camera: &Camera) -> render::FPoint {
        let px_coords: Vector = (camera.px_per_meter as f32) * (*self - camera.coords);
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

    fn render(
        &self,
        base_point: Coords,
        camera: &Camera,
        canvas: &mut Canvas<Window>,
        label: Option<(&str, &TextRendererConfig)>,
    ) -> Result<(), RenderVectorError> {
        if self.norm().0 < 0.5 {
            return Ok(());
        }

        let start: rect::Point = (base_point).as_point(camera);
        let end: rect::Point = (base_point + *self).as_point(camera);
        canvas
            .draw_line(start, end)
            .map_err(RenderVectorError::SdlError)?;

        let arrow_tip: rect::Point =
            (base_point + *self + 0.2 * self.rotated(150.0f32.to_radians())).as_point(camera);
        canvas
            .draw_line(end, arrow_tip)
            .map_err(RenderVectorError::SdlError)?;
        let arrow_tip2: rect::Point =
            (base_point + *self + 0.2 * self.rotated(210.0f32.to_radians())).as_point(camera);
        canvas
            .draw_line(end, arrow_tip2)
            .map_err(RenderVectorError::SdlError)?;
        if let Some(label) = label {
            let tip_point: rect::Point = (base_point + *self).as_point(camera);
            let mut label_render_info = text_render_info(label.0, label.1)
                .map_err(RenderVectorError::TextRenderInfoError)?;
            label_render_info.rect.x = tip_point.x - label_render_info.width() / 2;
            label_render_info.rect.y = tip_point.y - label_render_info.height() / 2;
            render_text(canvas, &label_render_info).map_err(RenderVectorError::SdlError)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
enum RenderVectorError {
    SdlError(sdl3::Error),
    TextRenderInfoError(TextRenderInfoError),
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
    engine_acc: Vector,
    velocity: Vector,
    friction_factor: f32,
}

impl Car {
    fn init(coords: Coords, direction: Vector) -> Car {
        Car {
            coords,
            direction_vec: direction.normalized(),
            length: Meter(5.),
            width: Meter(2.),
            engine_acc: Vector::init(0., 0.),
            velocity: Vector::init(0., 0.),
            friction_factor: FRICTION_FACTOR,
        }
    }

    fn render(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), ()> {
        let corners: Vec<render::FPoint> = [(1., 1.), (-1., 1.), (-1., -1.), (1., -1.), (1., 1.)]
            .into_iter()
            .map(|(d1, d2)| {
                self.coords
                    + (d1 * (self.length.0 / 2.)) * self.direction_vec
                    + (d2 * (self.width.0 / 2.)) * self.direction_vec.orthogonal()
            })
            .map(|coords: Coords| coords.as_fpoint(camera))
            .collect();

        canvas.draw_lines(corners.as_slice()).map_err(|_| ())
    }

    fn render_debug_vectors(
        &self,
        camera: &Camera,
        canvas: &mut Canvas<Window>,
        text_renderer_config: &TextRendererConfig,
    ) -> Result<(), RenderVectorError> {
        let debug_scalar: f32 = 0.3;
        let base_point: Coords = self.coords;
        let friction_acc = -self.friction_factor * self.velocity;
        canvas.set_draw_color(Color::RGB(255, 0, 0));
        (debug_scalar * self.velocity).render(
            base_point,
            camera,
            canvas,
            Some(("v", text_renderer_config)),
        )?;
        canvas.set_draw_color(Color::RGB(0, 0, 255));
        (debug_scalar.powi(2) * self.engine_acc).render(
            base_point,
            camera,
            canvas,
            Some(("p", text_renderer_config)),
        )?;
        canvas.set_draw_color(Color::RGB(0, 255, 0));
        (debug_scalar.powi(2) * friction_acc).render(
            base_point,
            camera,
            canvas,
            Some(("f", text_renderer_config)),
        )?;
        canvas.set_draw_color(Color::RGB(255, 255, 0));
        (debug_scalar.powi(2) * (self.engine_acc + friction_acc)).render(
            base_point,
            camera,
            canvas,
            Some(("a", text_renderer_config)),
        )?;
        Ok(())
    }

    fn set_engine_acc(&mut self, acc: Vector) {
        self.engine_acc = acc
    }

    fn update_coords(&mut self, dt: f32) {
        let friction_acc = -self.friction_factor * self.velocity;
        self.velocity = self.velocity + dt * self.engine_acc + dt * friction_acc;
        let velocity_norm: f32 = self.velocity.norm().0;
        if velocity_norm != 0. {
            self.direction_vec = (1. / velocity_norm) * self.velocity;
        }
        self.coords = self.coords + dt * self.velocity;
    }
}

fn render_polygon(
    verticies: &[Coords],
    camera: &Camera,
    canvas: &mut Canvas<Window>,
) -> Result<(), sdl3::Error> {
    let mut fpoints: Vec<render::FPoint> = verticies
        .iter()
        .map(|coord: &Coords| coord.as_fpoint(camera))
        .collect();
    fpoints.push(verticies[0].as_fpoint(camera));
    canvas.draw_lines(fpoints.as_slice())
}

fn fill_polygon(
    verticies: &[Coords],
    camera: &Camera,
    canvas: &mut Canvas<Window>,
) -> Result<(), sdl3::Error> {
    // This function is fully generated by Gemini

    let fpoints: Vec<render::FPoint> = verticies
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

#[derive(Clone)]
struct BezierPathConfig {
    center: Coords,
    n_anchors: i32,
    mean_anchor_radius: f32,
    stdev_anchor_radius: f32,
    stdev_subanchor_angle_dev: f32,
    points_per_bezier_curve: usize,
}

impl BezierPathConfig {
    fn init_circular() -> Self {
        BezierPathConfig {
            center: Coords::init(0., 0.),
            n_anchors: 12,
            mean_anchor_radius: 50.,
            stdev_anchor_radius: 0.,
            stdev_subanchor_angle_dev: 0.,
            points_per_bezier_curve: 20,
        }
    }
}

impl Default for BezierPathConfig {
    fn default() -> Self {
        BezierPathConfig {
            center: Coords::init(0., 0.),
            n_anchors: 9,
            mean_anchor_radius: 60. * 2.,
            stdev_anchor_radius: 20. * 2.,
            stdev_subanchor_angle_dev: 10.,
            points_per_bezier_curve: 15,
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
        render_polygon(&self.coords, camera, canvas)
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

fn draw_circle(
    canvas: &mut Canvas<Window>,
    x: i16,
    y: i16,
    radius: f32,
    color: Color,
) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(color);
    let segments: i32 = 36;
    for i in 0..segments {
        let theta1: f32 = (i as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
        let theta2: f32 = ((i + 1) as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
        let x1: f32 = x as f32 + radius * theta1.cos();
        let y1: f32 = y as f32 + radius * theta1.sin();
        let x2: f32 = x as f32 + radius * theta2.cos();
        let y2: f32 = y as f32 + radius * theta2.sin();
        canvas.draw_line((x1, y1), (x2, y2))?;
    }
    Ok(())
}

fn draw_disc(
    canvas: &mut Canvas<Window>,
    x: i16,
    y: i16,
    radius: f32,
    color: Color,
) -> Result<(), sdl3::Error> {
    canvas.set_draw_color(color);
    for i in -(radius as i16)..=(radius as i16) {
        let y_offset = (radius * radius - (i as f32) * (i as f32)).sqrt() as i16;
        for j in -y_offset..=y_offset {
            canvas.draw_point((x as f32 + i as f32, y as f32 + j as f32))?;
        }
    }
    Ok(())
}

fn render_joystick(
    position: (u32, u32),
    direction: Vector,
    pressed: bool,
    canvas: &mut Canvas<Window>,
) -> Result<(), sdl3::Error> {
    let (x, y) = position;
    draw_circle(
        canvas,
        x as i16,
        y as i16,
        JOYSTICK_OUTER_RADIUS,
        Color::RGB(200, 200, 200),
    )?;
    let dir_x: f32 = x as f32 + direction.x.0 * JOYSTICK_MAX_REACH_RADIUS;
    let dir_y: f32 = y as f32 - direction.y.0 * JOYSTICK_MAX_REACH_RADIUS;
    draw_disc(
        canvas,
        dir_x as i16,
        dir_y as i16,
        JOYSTICK_INNER_RADIUS,
        if pressed {
            Color::RGB(125, 125, 125)
        } else {
            Color::RGB(175, 175, 175)
        },
    )?;
    Ok(())
}

struct TextRendererConfig<'a> {
    font: ttf::Font<'a>,
    texture_creator: render::TextureCreator<WindowContext>,
}

struct TextRenderInfo<'a> {
    texture: render::Texture<'a>,
    rect: rect::Rect,
}

impl TextRenderInfo<'_> {
    fn set_pos(&mut self, x: i32, y: i32) {
        self.rect.set_x(x);
        self.rect.set_y(y);
    }

    fn width(&self) -> i32 {
        self.rect.width() as i32
    }

    fn height(&self) -> i32 {
        self.rect.height() as i32
    }
}

fn text_render_info<'a>(
    text: &str,
    text_renderer_config: &'a TextRendererConfig,
) -> Result<TextRenderInfo<'a>, TextRenderInfoError> {
    let surface = text_renderer_config
        .font
        .render(text)
        .blended(Color::RGB(255, 255, 255))
        .map_err(TextRenderInfoError::FontRenderError)?;
    let texture = text_renderer_config
        .texture_creator
        .create_texture_from_surface(&surface)
        .map_err(TextRenderInfoError::CreateTextureError)?;
    let rect = rect::Rect::new(0, 0, surface.width(), surface.height());
    Ok(TextRenderInfo { texture, rect })
}

#[derive(Debug)]
enum TextRenderInfoError {
    FontRenderError(ttf::FontError),
    CreateTextureError(render::TextureValueError),
}

fn render_text(
    canvas: &mut Canvas<Window>,
    render_info: &TextRenderInfo,
) -> Result<(), sdl3::Error> {
    canvas.copy(&render_info.texture, None, render_info.rect)?;
    Ok(())
}

const CAMERA_WIDTH: u32 = 1200;
const CAMERA_HEIGHT: u32 = 900;
const SIDE_PANEL_WIDTH: u32 = 400;

const SCREEN_WIDTH: u32 = CAMERA_WIDTH + SIDE_PANEL_WIDTH;
const SCREEN_HEIGHT: u32 = CAMERA_HEIGHT;

const JOYSTICK_POSITION: (u32, u32) = (CAMERA_WIDTH + SIDE_PANEL_WIDTH / 2, CAMERA_HEIGHT / 3);
const JOYSTICK_INNER_RADIUS: f32 = 50.;
const JOYSTICK_OUTER_RADIUS: f32 = 100.;
const JOYSTICK_MAX_REACH_RADIUS: f32 = JOYSTICK_OUTER_RADIUS - 0.8 * JOYSTICK_INNER_RADIUS;

const TIME_TO_99_PERC_SPEED: f32 = 5.; // in s
const THEORETICAL_MAX_SPEED: f32 = 100.; // in m/s
const FRICTION_FACTOR: f32 = 4.6 / TIME_TO_99_PERC_SPEED;
const ENGINE_ACCELERATION: f32 = THEORETICAL_MAX_SPEED * FRICTION_FACTOR;
const PATH_WIDTH: f32 = 12.0;

const GAME_FPS: f32 = 60.0;
const DT: f32 = GAME_FPS.recip();

pub fn main() {
    // Initialize SDL
    sdl3::hint::set("SDL_RENDER_VSYNC", "1");
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
    let circuit_debug: bool = false;
    let mouvement_debug: bool = true;
    let path_config: BezierPathConfig = BezierPathConfig::default();
    let path: BezierPath = path_config.generate_path(Some(seed));
    let (outer_border, inner_border) = path.generate_path_borders(PATH_WIDTH);
    let starting_point: Coords = path.starting_point;
    let starting_direction: Vector = path.starting_direction;

    // Initialize a car
    let mut car: Car = Car::init(starting_point, starting_direction);

    // Initialize the cameras
    let mut car_camera: Camera = Camera {
        width: CAMERA_WIDTH,
        height: CAMERA_HEIGHT,
        coords: car.coords,
        px_per_meter: (CAMERA_WIDTH as f32 / (car.length.0 / 0.05)).round() as u32,
        // car length is 5% of the camera width
    };

    let fix_camera: Camera = Camera {
        width: CAMERA_WIDTH,
        height: CAMERA_HEIGHT,
        coords: Coords::init(0., 0.),
        px_per_meter: 2,
    };

    let mut current_camera: usize = 0; // 0 for car_camera, 1 for fix_camera

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

    // Implement mouse and keyboard controls for the car
    let mut joystick_pressed: bool = false;

    // Initialize the TTF context for rendering text
    let ttf_context: ttf::Sdl3TtfContext = ttf::init().unwrap();
    let text_renderer_config: TextRendererConfig = TextRendererConfig {
        font: ttf_context
            .load_font("assets/Basic-Regular.ttf", 25.)
            .unwrap(),
        texture_creator: canvas.texture_creator(),
    };

    // Instantiate text render info for displaying instructions and information
    let mut hold_space_render_info: TextRenderInfo<'_> =
        text_render_info("PRESS ENTER TO TOGGLE CAMERAS", &text_renderer_config).unwrap();
    // TODO : Implement position specification for `text_render_info` to avoid having to
    //        call `set_pos` separately
    hold_space_render_info.set_pos(
        10,
        SCREEN_HEIGHT as i32 - hold_space_render_info.height() - 10,
    );

    let mut js_name_render_info: TextRenderInfo<'_> =
        text_render_info("No controller detected at startup", &text_renderer_config).unwrap();
    if let Some(js) = &joystick {
        println!("Joystick connected: {:?}", js.name());
        js_name_render_info =
            text_render_info(&format!("Controller: {}", js.name()), &text_renderer_config).unwrap();
    }
    js_name_render_info.set_pos(SCREEN_WIDTH as i32 - js_name_render_info.width() - 10, 10);

    let mut seed_render_info: TextRenderInfo<'_> =
        text_render_info(&format!("SEED: {}", seed), &text_renderer_config).unwrap();
    seed_render_info.set_pos(10, 10);

    // Initialize an FPS sample queue to monitor average FPS across 100 frames
    let mut fps_sample: VecDeque<f32> = VecDeque::with_capacity(100);
    for _ in 0..100 {
        fps_sample.push_back(GAME_FPS);
    }

    // Initialize the event pump for handling SDL events
    let mut event_pump: sdl3::EventPump = sdl_context.event_pump().unwrap();

    'running: loop {
        let time: time::Instant = time::Instant::now();

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
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    x,
                    y,
                    ..
                } => {
                    let dist_to_joystick: f32 = ((x - JOYSTICK_POSITION.0 as f32).powi(2)
                        + (y - JOYSTICK_POSITION.1 as f32).powi(2))
                    .sqrt();
                    if dist_to_joystick < JOYSTICK_INNER_RADIUS {
                        joystick_pressed = true;
                    }
                }
                Event::MouseButtonUp {
                    mouse_btn: MouseButton::Left,
                    ..
                } => {
                    joystick_pressed = false;
                }
                _ => (),
            }
        }

        let keyboard_state: KeyboardState<'_> = event_pump.keyboard_state();
        let space_pressed: bool = keyboard_state.is_scancode_pressed(Scancode::Space);

        let mut acceleration_pressed: bool = false;
        let mut joystick_direction = Vector::init(0.0, 0.0);

        // Read joystick input
        if let Some(js) = &joystick {
            let axis_0: f32 = js.axis(0).unwrap_or(0) as f32 / 32768.0;
            let axis_1: f32 = js.axis(1).unwrap_or(0) as f32 / 32768.0;
            joystick_direction = Vector::init(axis_0, -axis_1);
            acceleration_pressed = js.button(0).unwrap_or(false);
        }

        // Read mouse and keyboard output
        acceleration_pressed |= space_pressed;
        if joystick_pressed {
            let mouse_x: f32 = event_pump.mouse_state().x();
            let mouse_y: f32 = event_pump.mouse_state().y();
            let dx: f32 = mouse_x - JOYSTICK_POSITION.0 as f32;
            let dy: f32 = mouse_y - JOYSTICK_POSITION.1 as f32;
            joystick_direction = (1. / JOYSTICK_MAX_REACH_RADIUS) * Vector::init(dx, -dy);
            if joystick_direction.norm().0 > 1.0 {
                joystick_direction = joystick_direction.normalized();
            }
        }

        // Apply player input to the car's acceleration
        if acceleration_pressed {
            let engine_acc_vec = if joystick_direction.norm().0 < 0.2 {
                Vector::init(0.0, 0.0)
            } else {
                joystick_direction.normalized()
            };
            car.set_engine_acc(ENGINE_ACCELERATION * engine_acc_vec);
        } else {
            car.set_engine_acc(Vector::init(0.0, 0.0));
        }

        // Update the car's coordinates based on its current acceleration and velocity
        car.update_coords(DT);
        // println!("Theoretical max speed: {}km/h", THEORETICAL_MAX_SPEED * 3.6);
        // println!("Car acceleration: {}km/h", car.velocity.norm().0 * 3.6);

        // Update cameras
        car_camera.coords = car.coords;

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

        // Draw the circuit
        // canvas.set_draw_color(Color::RGB(200, 200, 200));
        // failed_rendering |= render_polygon(&inner_border, current_camera, &mut canvas).is_err();
        // failed_rendering |= render_polygon(&outer_border, current_camera, &mut canvas).is_err();

        // Fill all the inside of the circuit
        canvas.set_draw_color(Color::RGB(10, 70, 10));
        failed_rendering |= fill_polygon(&inner_border, current_camera, &mut canvas).is_err();

        canvas.set_draw_color(Color::RGB(80, 80, 80));
        let inner_border_loop = inner_border
            .iter()
            .chain(iter::once(inner_border.first().unwrap()));
        let outer_border_loop = outer_border
            .iter()
            .chain(iter::once(outer_border.first().unwrap()));

        failed_rendering |= fill_polygon(
            inner_border_loop
                .chain(outer_border_loop.rev())
                .copied()
                .collect::<Vec<_>>()
                .as_slice(),
            current_camera,
            &mut canvas,
        )
        .is_err();

        if circuit_debug {
            failed_rendering |= path.render(current_camera, &mut canvas).is_err();
            failed_rendering |= path
                .render_debug_anchors(current_camera, &mut canvas)
                .is_err();
        }

        // Render the car
        canvas.set_draw_color(Color::RGB(200, 0, 0));
        failed_rendering |= car.render(current_camera, &mut canvas).is_err();

        // Debug car vectors
        if mouvement_debug {
            failed_rendering |= car
                .render_debug_vectors(current_camera, &mut canvas, &text_renderer_config)
                .is_err();
        }

        // Write "PRESS ENTER TO TOGGLE CAMERAS"
        failed_rendering |= render_text(&mut canvas, &hold_space_render_info).is_err();

        // Render controller name
        failed_rendering |= render_text(&mut canvas, &js_name_render_info).is_err();

        // Render seed text
        failed_rendering |= render_text(&mut canvas, &seed_render_info).is_err();

        // Draw a seperating line between the game area and the side panel
        canvas.set_draw_color(Color::RGB(100, 100, 100));
        let (x1, y1) = (CAMERA_WIDTH, 0u32);
        let (x2, y2) = (CAMERA_WIDTH, CAMERA_HEIGHT);
        failed_rendering |= canvas
            .draw_line((x1 as f32, y1 as f32), (x2 as f32, y2 as f32))
            .is_err();

        // Render the joystick
        failed_rendering |= render_joystick(
            JOYSTICK_POSITION,
            joystick_direction,
            acceleration_pressed,
            &mut canvas,
        )
        .is_err();

        // Apply the changes on the screen
        canvas.present();

        // Detect failed rendering attempts
        if failed_rendering {
            eprintln!("Warning: Failed rendering one or more elements.");
        }

        // Control the frame rate
        let frame_duration = Duration::new(0, (1_000_000_000f32 / GAME_FPS).round() as u32);
        if let Some(remaining_time) = frame_duration.checked_sub(time.elapsed()) {
            std::thread::sleep(remaining_time);
        }

        fps_sample.push_back(1.0 / time.elapsed().as_secs_f32());
        fps_sample.pop_front();

        let average_fps: f32 = fps_sample.iter().sum::<f32>() / fps_sample.len() as f32;

        // println!("Average FPS: {}", average_fps);
        // If average frame rate drops below 80% of the frame target, log a warning
        if average_fps < 0.8 * GAME_FPS {
            eprintln!("Warning: Frame rate dropped below 80% of the target.");
        }

        game_tick_counter += 1;
    }
}
