let next_client_id = 0;
let client_id_to_gameptr_map = {};

let register_ffi = function(guest) {
	guest.env.send_game = function(
		game_ptr,	// *mut Game
	) {
		let id = next_client_id;
		client_id_to_gameptr_map[next_client_id] = game_ptr;
		next_client_id += 1;
		return id;
	}

	guest.env.send_tick_data = function(
		tick_index,	// u8
		data_ptr	// [u8; 3]
	) {
		console.error("TODO");
	}
}

miniquad_add_plugin({
	register_plugin: register_ffi
});