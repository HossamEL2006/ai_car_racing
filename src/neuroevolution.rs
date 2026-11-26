use crate::{
    default_game_config::{ENGINE_ACCELERATION, FRICTION_FACTOR, THEORETICAL_MAX_SPEED},
    game::{Game, GameConfig, PlayerDecision},
    screen_graphic::TextFont,
    texture_cache::{HashableGraphicProps, TextureCache},
    world_graphic::{WorldGraphic, WorldVectorGraphic},
    world_objects::{Camera, Meter, WorldCoords, WorldPolygon, WorldVector},
};
use ndarray::Array2;
use ndarray_rand::{RandomExt, rand_distr::StandardNormal};
use sdl3::{event::Event, keyboard::Keycode, pixels::Color, ttf, video::Window};
use std::time::{self, Duration};

const NN_ARCHITECTURE: [usize; 4] = [14, 20, 20, 5];

const SCREEN_WIDTH: u32 = 1600;
const SCREEN_HEIGHT: u32 = 900;

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

struct NeuralNetwork {
    layers: Vec<Array2<f32>>,
}

impl NeuralNetwork {
    fn random_normal(architecture: &[usize]) -> Self {
        let layers = architecture
            .windows(2)
            .map(|w| Array2::<f32>::random((w[0], w[1]), StandardNormal))
            .collect::<Vec<_>>();
        Self { layers }
    }

    fn forward(&self, input: &Array2<f32>) -> Array2<f32> {
        let mut output = input.clone();
        for layer in &self.layers {
            output = output.dot(layer);
            output.mapv_inplace(sigmoid);
        }
        output
    }
}

pub fn run_neuroevolution_training() {
    let sdl_context: sdl3::Sdl = sdl3::init().unwrap();
    let video_subsystem: sdl3::VideoSubsystem = sdl_context.video().unwrap();
    let window: Window = video_subsystem
        .window("Car Racing Game", SCREEN_WIDTH, SCREEN_HEIGHT)
        .position_centered()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas();

    let texture_creator = canvas.texture_creator();
    let mut texture_cache: TextureCache<HashableGraphicProps> = TextureCache::new();
    let ttf_context: ttf::Sdl3TtfContext = ttf::init().unwrap();
    let text_font: TextFont = TextFont::new(
        ttf_context
            .load_font("assets/Basic-Regular.ttf", 25.)
            .unwrap(),
        "Basic-Regular",
    );

    // Initialize the game and neural networks for the players
    const N_PLAYERS: usize = 100;
    let game_config = GameConfig::default();
    let game_fps = game_config.dt.recip();
    let mut game: Game = Game::init_game(N_PLAYERS, game_config);
    let nns = (0..N_PLAYERS)
        .map(|_| NeuralNetwork::random_normal(&NN_ARCHITECTURE))
        .collect::<Vec<_>>();

    let camera = Camera::new(SCREEN_WIDTH, SCREEN_HEIGHT, WorldCoords::origin(), 2);
    let mut moving_camera = Camera::new(SCREEN_WIDTH, SCREEN_HEIGHT, WorldCoords::origin(), 6);

    'running: loop {
        let time: time::Instant = time::Instant::now();

        // Read the events
        let mut event_pump = sdl_context.event_pump().unwrap();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => (),
            }
        }

        let mut player_decisions: [PlayerDecision; N_PLAYERS] = [PlayerDecision {
            direction: WorldVector::zero(),
            accelerate: false,
        }; N_PLAYERS];

        let mut debug_vectors: Vec<WorldVectorGraphic> = Vec::new();

        for (id, player) in game.players().iter().enumerate() {
            let mut input = Array2::<f32>::zeros((1, NN_ARCHITECTURE[0]));

            input[[0, 0]] = THEORETICAL_MAX_SPEED.recip() * player.car().velocity().norm().value();

            let car_acc = player.car().acceleration();
            let car_dir = player.car().direction().normalized();
            let acc_relative_x = car_acc.dot(&car_dir);
            let car_orthogonal_dir = car_dir.orthogonal();
            let acc_relative_y = car_acc.dot(&car_orthogonal_dir);
            input[[0, 1]] = acc_relative_x.value() / ENGINE_ACCELERATION;
            input[[0, 2]] = acc_relative_y.value() / ENGINE_ACCELERATION;
            input[[0, 3]] = player.car().curr_friction_factor() / (6. * FRICTION_FACTOR);

            let n_angles = 10;

            for i in 0..n_angles {
                let angle = 2.0 * std::f32::consts::PI * (i as f32) / (n_angles as f32);
                let car_dir = player.car().direction();
                let ray_dir = car_dir.rotated(angle);

                let intersection = get_first_intersection(
                    &player.car().coords(),
                    &ray_dir,
                    &[game.track().inner_border(), game.track().outer_border()],
                );
                let distance = intersection.map(|intersection_point| {
                    let vec = intersection_point - player.car().coords();
                    debug_vectors
                        .push(vec.as_graphic(player.car().coords(), Color::RGB(200, 0, 0)));
                    vec.norm().value()
                });
                input[[0, 4 + i]] = distance.map(|x| x / 60.).unwrap_or(10.);
            }

            let output = nns[id].forward(&input);

            let accelerate = output[[0, 0]] > 0.5;
            let pos_x_acceleration = output[[0, 1]];
            let neg_x_acceleration = output[[0, 2]];
            let pos_y_acceleration = output[[0, 3]];
            let neg_y_acceleration = output[[0, 4]];
            let x_acceleration = pos_x_acceleration - neg_x_acceleration;
            let y_acceleration = pos_y_acceleration - neg_y_acceleration;

            player_decisions[id] = PlayerDecision {
                direction: x_acceleration * car_dir + y_acceleration * car_orthogonal_dir,
                accelerate,
            };

            if id == 0 && game.game_tick().is_multiple_of(60) {
                println!("\n-----------\nInput vector for player 0:");
                for x in input.row(0) {
                    println!("{:?}", x);
                }
            }
        }

        for (id, player) in game.mut_players().iter_mut().enumerate() {
            player.make_decision(player_decisions[id]);
        }

        game.update();

        moving_camera.set_coords(game.players()[0].car().coords());

        game.render_onto(
            &moving_camera,
            &mut canvas,
            &texture_creator,
            &mut texture_cache,
            &text_font,
        );

        for debug_vector in &debug_vectors {
            debug_vector
                .render_onto(&moving_camera, &mut canvas)
                .unwrap();
        }

        canvas.present();

        let frame_duration: Duration =
            Duration::new(0, (1_000_000_000f32 / game_fps).round() as u32);
        if let Some(remaining_time) = frame_duration.checked_sub(time.elapsed()) {
            std::thread::sleep(remaining_time);
        }

        // println!("FPS: {}", 1_000_000 / time.elapsed().as_micros());
    }
}

pub fn get_first_intersection(
    base_point: &WorldCoords,
    direction: &WorldVector,
    polygons: &[&WorldPolygon],
) -> Option<WorldCoords> {
    // This function was generated by a Claude AI model

    const EPSILON: f32 = 1e-6;

    let direction = direction.normalized();

    let mut closest_t = f32::INFINITY;

    for polygon in polygons {
        let vertices = polygon.vertices();
        let n = vertices.len();
        if n < 2 {
            continue;
        }

        let (px, py) = (base_point.x().value(), base_point.y().value());
        let (dx, dy) = (direction.x().value(), direction.y().value());

        for i in 0..n {
            let a = vertices[i];
            let b = vertices[(i + 1) % n]; // wraps around to close the polygon

            // Edge vector e = b - a
            let (ex, ey) = (b.x().value() - a.x().value(), b.y().value() - a.y().value());

            // Solve  base + t * d = a + u * e  using 2D cross products.
            let denom = dx * ey - dy * ex; // d × e
            if denom.abs() < EPSILON {
                continue; // ray is parallel to this edge (or direction is zero)
            }

            let (wx, wy) = (a.x().value() - px, a.y().value() - py); // w = a - base
            let t = (wx * ey - wy * ex) / denom; // w × e / d × e
            let u = (wx * dy - wy * dx) / denom; // w × d / d × e

            // t >= 0: hit is in front of the ray; 0 <= u <= 1: hit lies on the edge
            if t >= 0. && (0.0..=1.0).contains(&u) && t < closest_t {
                closest_t = t;
            }
        }
    }

    if closest_t.is_finite() {
        Some(*base_point + closest_t * direction)
    } else {
        None
    }
}
