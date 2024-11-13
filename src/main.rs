use std::collections::HashMap;
use std::sync::Mutex;

use macroquad::prelude::*;
use macroquad::Window;

const BACKGROUND_COLOR: Color = Color::new(0.168, 0.149, 0.152, 1.0);
const SCREEN_SIZE: i32 = 256;
const TICKRATE: u8 = 20;
const TICK_DELTA: f32 = 1.0 / TICKRATE as f32;

/// This queue will hold all of the intents received from the server
/// that are intended to be processed on the next local tick.
static TICK_QUEUE: Mutex<Vec<Tick>> = Mutex::new(Vec::new());

//// Here we define the host FFI; for this project, it happens to
//// almost entirely entail just the methods necessary to communicate
//// with other clients over the host net.
extern "C" {
	fn send_tick_data(
		tick_index: usize,
		data_ptr: *mut u8,
		data_size: usize
	);
}

//// Below, we define the client FFI; these are the methods that the host
//// will use to control the client. In a real-world scenario you would
//// likely want some authorization + encryption mechanism to ensure data
//// is not only correct, but has been issued by an authorized server.
////
//// For the purposes of this example, this security aspect has been
//// skipped entirely, as auth/validation flows are *not* the subject
//// of this demo.

#[no_mangle]
extern "C" fn start_game(client_id: usize) {
	Window::from_config(Conf {
		window_width: SCREEN_SIZE,
		window_height: SCREEN_SIZE,
		window_resizable: false,
		..Default::default()
	}, amain(client_id));
}

#[no_mangle]
extern "C" fn receive_tick(
	players: *mut u8,
	players_len: usize,
	player_intents: *mut u8,
	player_intents_len: usize
) {
	let mut queue = TICK_QUEUE
		.lock()
		.unwrap();

	let player_ids = unsafe {
		std::slice::from_raw_parts(players, players_len)
	};

	let player_intents = unsafe {
		std::slice::from_raw_parts(player_intents, player_intents_len)
	};

	let mut intent_map = HashMap::new();

	let mut index = 0;
	for player_id in player_ids {
		let mut intents = vec![];

		let intent_bytes = &player_intents[(3 * index)..(3 * index + 3)];
		for &intent in intent_bytes {
			if intent != 0 {
				intents.push(PlayerIntent::from(intent));
			}
		}

		intent_map.insert((*player_id).into(), intents);
		index += 1;
	}

	let tick = Tick {
		intents: intent_map
	};

	queue.push(tick);
}

/// Represents all actions that a player may take.
#[derive(Clone, Copy)]
enum PlayerIntent {
	MoveLeft = 1,
	MoveRight = 2,
	Jump = 3
}

impl From<u8> for PlayerIntent {
	fn from(value: u8) -> Self {
		use PlayerIntent::*;

		match value {
			1 => MoveLeft,
			2 => MoveRight,
			3 => Jump,
			_ => panic!()
		}
	}
}

impl From<PlayerIntent> for u8 {
	fn from(intent: PlayerIntent) -> u8 {
		match intent {
			PlayerIntent::MoveLeft => 1,
			PlayerIntent::MoveRight => 2,
			PlayerIntent::Jump => 3
		}
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

	pub fn local() -> Self {
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

	fn simulate(&mut self, tick: Tick) {
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

fn main() { }

async fn amain(client_id: usize) {
	let mut tick_time = 0.0;
	let mut tick_index = 0;

	let mut game = Game {
		client_id,
		players: HashMap::new(),
		ticks: Vec::new(),
	};

	game.players.insert(
		client_id,
		Player::local()
	);

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

			unsafe {
				let mut tick_data_bytes: Vec<u8> = local_intents
					.iter()
					.map(|x| x.to_owned().into())
					.collect();

				tick_data_bytes.shrink_to_fit();

				send_tick_data(
					tick_index,
					tick_data_bytes.as_mut_ptr(),
					tick_data_bytes.len()
				);

				std::mem::forget(tick_data_bytes);
			}

			let mut tick = Tick {
				intents: HashMap::new()
			};
			tick.intents.insert(game.client_id, local_intents);

			let mut net_intents = TICK_QUEUE.lock().unwrap();
			for net_tick in net_intents.iter_mut() {
				for (&player_id, intents) in &mut net_tick.intents {
					if player_id == game.client_id {
						continue;
					}

					let net_to_local_intent_binding = tick
						.intents
						.entry(player_id)
						.or_insert(Vec::new());
					net_to_local_intent_binding.append(intents);
				}
			}

			game.simulate(tick);

			tick_time -= TICK_DELTA;
			tick_index += 1;
		}

		clear_background(BACKGROUND_COLOR);
		present(&mut game, tick_time);
		draw_text(&format!("{}", game.client_id), 15.0, 15.0, 16.0, RED);

		next_frame().await;
	}
}

fn present(game: &mut Game, tick_time: f32) {
	let smoothing = tick_time / TICK_DELTA;

	for (_, player) in &game.players {
		player.draw(smoothing);
	}
}