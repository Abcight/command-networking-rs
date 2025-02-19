use std::collections::HashMap;
use std::collections::VecDeque;
use sha2::{Sha256, Digest};

use macroquad::prelude::*;
use macroquad::Window;

const BACKGROUND_COLOR: Color = Color::new(0.168, 0.149, 0.152, 1.0);
const SCREEN_SIZE: i32 = 256;
const TICKRATE: u8 = 20;
const TICK_DELTA: f32 = 1.0 / TICKRATE as f32;

//// Here we define the host FFI; because this demo is going to use a dummy
//// network (embedded & simulated inside a JS environment), all of the
//// communication is going to happen through mock functions.
extern "C" {
	fn send_predicted_tick(
		data_ptr: *mut u8,
		data_size: usize
	);
}

//// Below, we define the client FFI; these are the methods that the JS host
//// will use to interface with the client. In a real-world scenario you
//// would want some authorization mechanism to ensure data has been issued
//// by an authorized server.
////
//// For the purposes of this example, this security aspect has been
//// skipped entirely, as auth/validation flows are *not* the subject
//// of this demo.

#[no_mangle]
extern "C" fn start_game(client_id: u8) {
	Window::from_config(Conf {
		window_width: SCREEN_SIZE,
		window_height: SCREEN_SIZE,
		window_resizable: false,
		..Default::default()
	}, amain(client_id));
}

/// This trait defines the methods that must be implemented by all types
/// which will be sent over the wire. It's just byte-format serialization.
pub trait NetType: Sized {
	fn to_bytes(&self, buffer: &mut Buffer);
	fn from_bytes(buffer: &mut Buffer) -> Result<Self, ()>;
}

/// We'll use a double ended queue for serialization/deserialization, as the
/// latter will happen by consuming the data from the front.
pub type Buffer = VecDeque<u8>;

/// Represents all actions that a player may take.
#[derive(Clone, Copy)]
#[repr(u8)]
enum PlayerIntent {
	/// Player wants to move to the left.
	MoveLeft = 0,
	/// Player wants to move to the right.
	MoveRight = 1,
	/// Player wants to jump.
	Jump = 2,
}

impl NetType for PlayerIntent {
	fn to_bytes(&self, buffer: &mut Buffer) {
		buffer.push_back(*self as u8);
	}

	fn from_bytes(buffer: &mut Buffer) -> Result<Self, ()> {
		let Some(tag) = buffer.pop_front() else { return Err(()) };
		match tag {
			0 => Ok(PlayerIntent::MoveLeft),
			1 => Ok(PlayerIntent::MoveRight),
			2 => Ok(PlayerIntent::Jump),
			_ => Err(())
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
		self.vertical_velocity += 9.81 * TICK_DELTA * 10.0;

		if self.y >= SCREEN_SIZE as f32 - 30.0 {
			self.y = SCREEN_SIZE as f32 - 30.0;
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
			},
			PlayerIntent::Jump => {
				if self.grounded {
					self.vertical_velocity = -50.0;
					self.grounded = false;
				}
			},
		}
	}
}

/// The ClientId is assigned to each player *by the server they connect to*.
pub type ClientId = u8;

/// A command frame is a collection of a player's intents, and their unique ClientId.
#[derive(Clone)]
struct CommandFrame {
	owner: ClientId,
	intents: Vec<PlayerIntent>
}

impl CommandFrame {
	pub fn update_hasher(&self, hasher: &mut impl Digest) {
		hasher.update(&[self.owner, self.intents.len() as u8]);
		for intent in &self.intents {
			hasher.update(&[*intent as u8]);
		}
	}
}

impl NetType for CommandFrame {
	fn to_bytes(&self, buffer: &mut Buffer) {
		buffer.push_back(self.owner);
		buffer.push_back(self.intents.len() as u8);
		for intent in &self.intents {
			intent.to_bytes(buffer);
		}
	}

	fn from_bytes(buffer: &mut Buffer) -> Result<Self, ()> {
		let Some(owner) = buffer.pop_front() else { return Err(()) };
		let Some(len) = buffer.pop_front() else { return Err(()) };
		let mut intents = Vec::new();

		for _ in 0..len {
			let intent = PlayerIntent::from_bytes(buffer)?;
			intents.push(intent);
		}

		Ok(CommandFrame {
			owner,
			intents
		})
	}
}

/// An ordinally indexed collection of CommandFrames, with a SHA256 checksum.
#[derive(Clone)]
struct Tick {
	index: u64,
	command_frames: Vec<CommandFrame>,
	hash: [u8; 32]
}

impl Tick {
	fn new(index: u64, command_frames: Vec<CommandFrame>) -> Self {
		let mut tick = Tick {
			index,
			command_frames,
			hash: [0; 32]
		};
		tick.recalculate_hash();
		tick
	}

	fn recalculate_hash(&mut self) {
		let mut hasher = Sha256::new();
		hasher.update(self.index.to_le_bytes());
		hasher.update(&[self.command_frames.len() as u8]);
		for command_frame in &self.command_frames {
			command_frame.update_hasher(&mut hasher);
		}
		self.hash = hasher.finalize().into();
	}
}

impl NetType for Tick {
	fn to_bytes(&self, buffer: &mut Buffer) {
		for byte in self.index.to_le_bytes() {
			buffer.push_back(byte);
		}
		buffer.push_back(self.command_frames.len() as u8);
		for command_frame in &self.command_frames {
			command_frame.to_bytes(buffer);
		}
	}

	fn from_bytes(buffer: &mut Buffer) -> Result<Self, ()> {
		let index: Option<u64> = {Some(u64::from_le_bytes([
			buffer.pop_front().ok_or(())?,
			buffer.pop_front().ok_or(())?,
			buffer.pop_front().ok_or(())?,
			buffer.pop_front().ok_or(())?,
			buffer.pop_front().ok_or(())?,
			buffer.pop_front().ok_or(())?,
			buffer.pop_front().ok_or(())?,
			buffer.pop_front().ok_or(())?,
		]))};
		let Some(index) = index else { return Err(()) };
		let Some(len) = buffer.pop_front() else { return Err(()) };

		let mut command_frames = Vec::new();
		for _ in 0..len {
			let command_frame = CommandFrame::from_bytes(buffer)?;
			command_frames.push(command_frame);
		}

		Ok(Tick::new(
			index,
			command_frames
		))
	}
}

/// A structure representing the local gamestate.
struct Game {
	/// ClientId denoting the local player
	client_id: ClientId,
	/// A map of all players and their respective ClientIds.
	players: HashMap<ClientId, Player>,
	/// All ticks processed by the client locally. Includes predicted ticks.
	ticks: Vec<Tick>,
	/// Index into ticks denoting the latest tick confirmed "correct" by the server.
	accepted_head: u64,
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

	fn simulate(&mut self, tick: &Tick) {
		for (_, player) in &mut self.players {
			player.snapshot_position();
		}

		for frame in &tick.command_frames {
			let player = self.players.entry(frame.owner).or_insert(Player::enemy());
			for intent in &frame.intents {
				player.execute_intent(intent);
			}
		}

		for (_, player) in &mut self.players {
			player.update_physics();
		}
	}

	fn predict_tick(&self) -> Tick {
		// Poll local intents and construct a command frame
		let intents = self.poll_intents();
		let local_frame = CommandFrame {
			owner: self.client_id,
			intents
		};

		// Predict player intents for the upcoming tick.
		// A good enough heuristic is simply repeating whatever they were doing
		// last tick.
		let Some(previous_tick) = self.ticks.last() else {
			return Tick::new(0, vec![local_frame]);
		};

		let mut anticipated_frames: Vec<CommandFrame> = previous_tick
			.command_frames
			.iter()
			.filter(|x| x.owner != self.client_id)
			.map(|x| x.clone())
			.collect();

		anticipated_frames.push(local_frame);

		Tick::new(previous_tick.index + 1, anticipated_frames)
	}

	fn print_debug(&self) {
		draw_text(&format!("Client ID: {}", self.client_id), 10.0, 20.0, 16.0, RED);
		if let Some(tick) = self.ticks.last() {
			draw_text(&format!("Local tick index: {}", tick.index), 10.0, 35.0, 16.0, RED);
			draw_text(&format!("Confirmed tick index: {}", self.accepted_head), 10.0, 50.0, 16.0, RED);
			draw_text(&format!("Running {} ticks ahead of server", tick.index - self.accepted_head), 10.0, 65.0, 16.0, RED);
		}
	}
}

#[cfg(target_os = "windows")]
fn main() {
	Window::from_config(Conf {
		window_width: SCREEN_SIZE,
		window_height: SCREEN_SIZE,
		window_resizable: false,
		..Default::default()
	}, amain(0));
}

#[cfg(not(target_os = "windows"))]
fn main() { }

async fn amain(client_id: u8) {
	let mut tick_time = 0.0;

	let mut game = Game {
		client_id,
		players: HashMap::new(),
		ticks: Vec::new(),
		accepted_head: 0
	};

	game.players.insert(
		client_id,
		Player::local()
	);

	loop {
		tick_time += get_frame_time();

		while tick_time >= TICK_DELTA {
			let tick_to_propose: Tick = game.predict_tick();

			// Send the proposed tick to the server
			unsafe {
				let mut tick_buffer = Buffer::new();
				tick_to_propose.to_bytes(&mut tick_buffer);
				tick_buffer.make_contiguous();

				send_predicted_tick(
					tick_buffer.as_mut_slices().0.as_mut_ptr(),
					tick_buffer.len()
				);
			}

			// Execute the proposed tick locally, anticipating that it's a correct prediction
			game.simulate(&tick_to_propose);

			// Add the tick to the local tick list
			game.ticks.push(tick_to_propose);

			tick_time -= TICK_DELTA;
		}

		clear_background(BACKGROUND_COLOR);
		present(&mut game, tick_time);

		game.print_debug();

		next_frame().await;
	}
}

fn present(game: &mut Game, tick_time: f32) {
	let smoothing = tick_time / TICK_DELTA;

	for (_, player) in &game.players {
		player.draw(smoothing);
	}
}