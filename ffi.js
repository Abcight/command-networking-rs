let tick_data = [];
let current_tick_index = 0;
let next_guest_id = 0;

let register_ffi = function(guest) {
	guest.env.send_tick_data = function(
		tick_index,	// u8
		data_ptr,	// *mut u8
		data_len,	// usize
	) {
		// the player got ahead of themselves, and starts
		// sending ticks from the future...
		if(tick_index > tick_data.length) {
			return;
		}

		// construct a tick entry if it's not there yet
		if(tick_index == tick_data.length) {
			tick_data[tick_index] = {};
		}

		// grab the intents passed by the guest ...
		let intents = new Uint8Array(
			guest.wasm_memory.buffer,
			data_ptr,
			data_len
		);

		// ... and include them in the tick
		tick_data[tick_index][guest.wasm_exports.memory.id] = intents;
	}
}

let start_round = function(guests) {
	tick_data = [];
	current_tick_index = 0;

	for(let i = 0; i < guests.length; i++) {
		let guest_id = next_guest_id++;
		guests[i].wasm_exports.memory.id = guest_id;
		guests[i].wasm_exports.start_game(guest_id);
	}

	let server_loop = setInterval(() => {
		if(current_tick_index in tick_data) {
			let tick = tick_data[current_tick_index];

			let all_player_data_arrived = Object.keys(tick).length >= guests.length;
			if(!all_player_data_arrived) {
				return;
			}

			for(let i = 0; i < guests.length; i++) {
				let guest = guests[i].wasm_exports;

				let wasm_players_len = guests.length;
				let wasm_players_ptr = guest.allocate_vec_u8(wasm_players_len);
				let wasm_players = new Uint8Array(
					guest.memory.buffer,
					wasm_players_ptr,
					guests.length - 1
				);

				let wasm_intents_len = 3 * guests.length;
				let wasm_intents_ptr = guest.allocate_vec_u8(wasm_intents_len);
				let wasm_intents = new Uint8Array(
					guest.memory.buffer,
					wasm_intents_ptr,
					wasm_intents_len
				);

				for(let j = 0; j < guests.length; j++) {
					let id = guests[j].wasm_memory.id;
					wasm_players[j] = j;
					wasm_intents[3 * j + 0] = tick[id][0] ?? 0;
					wasm_intents[3 * j + 1] = tick[id][1] ?? 0;
					wasm_intents[3 * j + 2] = tick[id][2] ?? 0;
				}

				guests[i].wasm_exports.receive_tick(
					wasm_players_ptr,
					wasm_players_len,
					wasm_intents_ptr,
					wasm_intents_len
				);
			}
			current_tick_index++;
		}
	}, 20);

	return server_loop;
}

miniquad_add_plugin({
	register_plugin: register_ffi
});