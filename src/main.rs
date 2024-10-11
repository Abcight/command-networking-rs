use std::collections::HashMap;

use macroquad::prelude::*;
use macroquad::Window;

const BACKGROUND_COLOR: Color = Color::new(0.168, 0.149, 0.152, 1.0);
const SCREEN_SIZE: i32 = 256;
const TICKRATE: u8 = 20;
const TICK_DELTA: f32 = 1.0 / TICKRATE as f32;

//// Here we define the host FFI; for this project, it happens to
//// almost entirely entail just the methods necessary to communicate
//// with other clients over the host net.
extern "C" {
	fn send_start_game(
		game: *mut Game
	);

	fn send_tick_data(
		tick_index: usize,
		data_ptr: *mut PlayerIntent,
		len: usize,
		capacity: usize
	);
}

//// Below, we define the client FFI; these are the methods that the host
//// will use to control the client.

#[no_mangle]
extern "C" fn receive_tick_data(
	game: *mut Game,
	player_index: usize,
	tick_index: usize,
	data_ptr: *mut PlayerIntent,
	len: usize,
	capacity: usize
) {
	let data = unsafe {
		Vec::from_raw_parts(data_ptr, len, capacity)
	};

	todo!()
}

/// Represents all actions that a player may take.
#[repr(u8)]
enum PlayerIntent {
	MoveLeft,
	MoveRight,
	Jump
}

impl From<PlayerIntent> for u8 {
    fn from(command: PlayerIntent) -> u8 {
        command as u8
    }
}

#[derive(Default)]
struct Player {
	x: f32,
	y: f32,
	last_tick_x: f32,
	last_tick_y: f32,
	vertical_velocity: f32,
	grounded: bool,
	color: Color,
}

impl Player {
	const MOVE_SPEED: f32 = 200.0;
	const WIDTH: f32 = 30.0;
	const HEIGHT: f32 = 30.0;

	pub fn new() -> Self {
		Self {
			color: BLUE,
			..Default::default()
		}
	}

	pub fn enemy() -> Self {
		Self {
			color: RED,
			..Default::default()
		}
	}

	pub fn draw(&self, smoothing: f32) {
		let smooth_x = (1.0 - smoothing) * self.last_tick_x + self.x * smoothing;
		let smooth_y = (1.0 - smoothing) * self.last_tick_y + self.y * smoothing;

		draw_rectangle(
			smooth_x,
			smooth_y,
			Self::WIDTH,
			Self::HEIGHT,
			self.color
		);
	}

	pub fn snapshot_position(&mut self) {
		self.last_tick_x = self.x;
		self.last_tick_y = self.y;
	}

	pub fn update_physics(&mut self) {
		self.y += self.vertical_velocity * TICK_DELTA * 10.0;
		self.vertical_velocity -= 9.81 * TICK_DELTA * 10.0;

		if self.y <= 0.0 {
			self.y = 0.0;
			self.vertical_velocity = 0.0;
			self.grounded = true;
		}
	}

	pub fn execute_intent(&mut self, intent: &PlayerIntent) {
		match intent {
			PlayerIntent::MoveLeft => {
				self.x -= Self::MOVE_SPEED * TICK_DELTA;
				self.x = self.x.clamp(0.0, SCREEN_SIZE as f32 - Self::WIDTH);
			},
			PlayerIntent::MoveRight => {
				self.x += Self::MOVE_SPEED * TICK_DELTA;
				self.x = self.x.clamp(0.0, SCREEN_SIZE as f32 - Self::WIDTH);
			}
			PlayerIntent::Jump => {
				if self.grounded {
					self.vertical_velocity = 50.0;
					self.grounded = false;
				}
			},
		}
	}
}

pub type ClientId = usize;

struct Tick {
	intents: HashMap<ClientId, Vec<PlayerIntent>>
}

struct Game {
	client_id: ClientId,
	players: HashMap<ClientId, Player>,
	ticks: Vec<Tick>
}

impl Game {
	fn poll_intents(&self) -> Vec<PlayerIntent> {
		let mut intents = vec![];

		if is_key_down(KeyCode::Up) {
			intents.push(PlayerIntent::Jump);
		}

		if is_key_down(KeyCode::Left) {
			intents.push(PlayerIntent::MoveLeft);
		}

		if is_key_down(KeyCode::Right) {
			intents.push(PlayerIntent::MoveRight);
		}

		intents
	}

	fn simulate_tick(&mut self, tick: Tick) {
		for (_, player) in &mut self.players {
			player.snapshot_position();
		}

		for (id, intents) in &tick.intents {
			let player = self.players.entry(*id).or_insert(Player::enemy());
			for intent in intents {
				player.execute_intent(intent);
			}
		}

		for (_, player) in &mut self.players {
			player.update_physics();
		}

		self.ticks.push(tick);
	}
}

fn main() {
	Window::from_config(Conf {
		window_width: SCREEN_SIZE,
		window_height: SCREEN_SIZE,
		window_resizable: false,
		..Default::default()
	}, amain());
}

async fn amain() {
	let mut tick_time = 0.0;

	let mut game = Game {
		client_id: 0,
		players: HashMap::new(),
		ticks: Vec::new(),
	};

	game.players.insert(0, Player::new());

	let rect = Rect::new(
		0.0,
		0.0,
		SCREEN_SIZE as f32,
		SCREEN_SIZE as f32
	);

	let camera = Camera2D::from_display_rect(rect);
	set_camera(&camera);

	loop {
		tick_time += get_frame_time();

		while tick_time >= TICK_DELTA {
			let local_intents = game.poll_intents();
			let mut tick = Tick {
				intents: HashMap::new()
			};
			tick.intents.insert(game.client_id, local_intents);

			game.simulate_tick(tick);
			tick_time -= TICK_DELTA;
		}

		clear_background(BACKGROUND_COLOR);
		present(&mut game, tick_time);

		next_frame().await;
	}
}

fn present(game: &mut Game, tick_time: f32) {
	let smoothing = tick_time / TICK_DELTA;

	for (_, player) in &game.players {
		player.draw(smoothing);
	}
}