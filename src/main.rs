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

	pub fn execute_command(&mut self, command: PlayerIntent) {
		match command {
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

struct Game {
	local_player: Player,
	net_players: Vec<Player>,
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
		local_player: Player::new(),
		net_players: vec![]
	};

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
			tick(&mut game);
			tick_time -= TICK_DELTA;
		}

		clear_background(BACKGROUND_COLOR);
		present(&mut game, tick_time);

		next_frame().await;
	}
}

fn tick(game: &mut Game) {
	game.local_player.snapshot_position();
	for net_player in &mut game.net_players {
		net_player.snapshot_position();
	}

	if is_key_down(KeyCode::Up) {
		game.local_player.execute_command(PlayerIntent::Jump);
	}

	if is_key_down(KeyCode::Left) {
		game.local_player.execute_command(PlayerIntent::MoveLeft);
	}

	if is_key_down(KeyCode::Right) {
		game.local_player.execute_command(PlayerIntent::MoveRight);
	}

	game.local_player.update_physics();
	for net_player in &mut game.net_players {
		net_player.update_physics();
	}
}

fn present(game: &mut Game, tick_time: f32) {
	let smoothing = tick_time / TICK_DELTA;

	game.local_player.draw(smoothing);
	for net_player in &game.net_players {
		net_player.draw(smoothing);
	}
}