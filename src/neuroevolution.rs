use crate::{
    game::{Game, GameConfig, PlayerDecision},
    screen_graphic::TextFont,
    texture_cache::{HashableGraphicProps, TextureCache},
    world_objects::{Camera, Meter, WorldCoords, WorldVector},
};
use ndarray::Array2;
use ndarray_rand::{RandomExt, rand_distr::StandardNormal};
use sdl3::{event::Event, keyboard::Keycode, ttf, video::Window};
use std::time::{self, Duration};

const NN_ARCHITECTURE: [usize; 4] = [10, 20, 20, 5];

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
            // apply sigmoid
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
    let n_players = 50;
    let game_config = GameConfig::default();
    let game_fps = game_config.dt.recip();
    let mut game: Game = Game::init_game(n_players, game_config);
    let nns = (0..n_players)
        .map(|_| NeuralNetwork::random_normal(&NN_ARCHITECTURE))
        .collect::<Vec<_>>();

    let camera = Camera::new(SCREEN_WIDTH, SCREEN_HEIGHT, WorldCoords::origin(), 2);

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

        for (id, player) in game.mut_players().iter_mut().enumerate() {
            // generate random input for the neural network
            let input = Array2::<f32>::random((1, NN_ARCHITECTURE[0]), StandardNormal);
            let output = nns[id].forward(&input);

            let accelerate = output[[0, 0]] > 0.5;
            let pos_x_acceleration = output[[0, 1]];
            let neg_x_acceleration = output[[0, 2]];
            let pos_y_acceleration = output[[0, 3]];
            let neg_y_acceleration = output[[0, 4]];
            let x_acceleration = Meter::new(pos_x_acceleration - neg_x_acceleration);
            let y_acceleration = Meter::new(pos_y_acceleration - neg_y_acceleration);

            player.make_decision(PlayerDecision {
                direction: WorldVector::new(x_acceleration, y_acceleration),
                accelerate,
            });
        }

        game.update();

        game.render_onto(
            &camera,
            &mut canvas,
            &texture_creator,
            &mut texture_cache,
            &text_font,
        );
        canvas.present();

        let frame_duration: Duration =
            Duration::new(0, (1_000_000_000f32 / game_fps).round() as u32);
        if let Some(remaining_time) = frame_duration.checked_sub(time.elapsed()) {
            std::thread::sleep(remaining_time);
        }

        println!("FPS: {}", 1_000_000 / time.elapsed().as_micros());
    }
}
