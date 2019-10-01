
#[derive(Debug, Clone, Copy)]
pub struct GameState {
	pub player1_score: i32,
	pub player2_score: i32,
}

impl GameState {
	pub fn new() -> Self {
		Self {
			player1_score: 0,
			player2_score: 0,
		}
	}
}