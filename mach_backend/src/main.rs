#![allow(unused_imports)]
#![feature(try_blocks)]

use std::{
	future::{Future},
	task::{Poll},
	net::{SocketAddr},
	time::{Instant, Duration},
	sync::{Arc, Mutex},
	collections::{HashMap},
	path::{Path, PathBuf},
	env::{self},
};
use futures::{
	FutureExt,
	StreamExt,
	TryStreamExt,
};
use tokio::{
	io::{AsyncRead, AsyncReadExt},
};
use snafu::{ResultExt, Snafu};
use hyper::{
	Body, Chunk, Client, Server, Request, Response, Method, StatusCode,
	service::{self},
};
use serde_derive::{Serialize, Deserialize};

use crate::{
	game::*,
};

pub mod game;

#[derive(Debug, Snafu)]
pub enum BackendError {
	#[snafu(display("Failed to create a Service"))]
	MakeServiceError,
	#[snafu(display("OOF"))]
	Oof,
}

#[derive(Debug, Snafu)]
pub enum ClientError {
	#[snafu(display("Client was stoopid"))]
	Stoopid,
}

#[tokio::main]
async fn main() -> Result<(), BackendError> {
	setup_logging();	
	let addr: SocketAddr = ([0, 0, 0, 0], 7878).into();
	
	let mach = Arc::new(Mutex::new(MachBackend::new()));
		
	let make_service
		= service::make_service_fn(|_target| {
		let mach_backend = Arc::clone(&mach);
		async move {
			Ok::<_, BackendError>(service::service_fn(move |request: Request<Body>| {
				let mach_backend = Arc::clone(&mach_backend);
				let request = request;
				async move {
					let site_dir = env::var("MACH_SITE_DIR").expect("MACH_SITE_DIR environment variable not set");
					let response = if request.method() == Method::GET {
						log::info!("Get for {:?}", request.uri().path());
						if request.uri().path().starts_with("/static") {
							let mut path = PathBuf::from(site_dir);
							path.push(PathBuf::from(request.uri().path()).strip_prefix("/").unwrap());
							log::info!("Have path {:?}", path);
							let mut file = tokio::fs::File::open(&path).await.unwrap();
							let mut buf = Vec::with_capacity(1024 * 8);
							file.read_to_end(&mut buf).await.unwrap();
							Response::new(Body::from(buf))
						} else if request.uri().path() == "/" {
							let mut path = PathBuf::from(site_dir);
							path.push("static/index.html");
							let mut file = tokio::fs::File::open(&path).await.unwrap();
							let mut buf = Vec::with_capacity(1024 * 8);
							file.read_to_end(&mut buf).await.unwrap();
							Response::new(Body::from(buf))
						} else {
							Response::builder()
								.status(StatusCode::NOT_FOUND)
								.body(Body::from("404 Not Found"))
								.unwrap()
						}
					} else if request.method() == Method::POST {
						if request.uri().path() == "/game_call" {
							let chunk = request.into_body().try_concat().await
								.map_err(|_| BackendError::Oof)?;
							let res: Result<Response<Body>, ClientError> = try {
								let bytes = chunk.into_bytes();
								let string = std::str::from_utf8(bytes.as_ref())
									.map_err(|_| ClientError::Stoopid)?;
								handle_client_action(string, Arc::clone(&mach_backend))?
							};
							match res {
								Ok(response) => response,
								Err(_) => Response::builder()
									.status(StatusCode::BAD_REQUEST)
									.body(Body::from("I bet u feel pretty stoopid right now"))
									.unwrap(),
							}
						} else {
							Response::builder()
								.status(StatusCode::NOT_FOUND)
								.body(Body::from("400 Bad Request"))
								.unwrap()
						}
					} else {
						Response::builder()
								.status(StatusCode::NOT_FOUND)
								.body(Body::from("400 Bad Request"))
								.unwrap()
					};
					Ok::<_, BackendError>(response)
				}
			}))
		}
	});
	
	let server = Server::bind(&addr)
		.serve(make_service);
	
	match server.await {
		Ok(()) => log::info!("Server exited successfully"),
		Err(e) => log::error!("Server encountered an error: {}", e),
	}
	
	Ok(())
}

#[derive(Debug, Clone, Deserialize)]
pub enum ClientAction {
	Register {
		name: String,
	},
	HostGame {
		player: usize,
	},
	JoinGame {
		player: usize,
		game_name: String,
	},
	IncreaseScore {
		player: usize,
	},
	StateCheck {
		player: usize,
	},
}

#[derive(Debug, Clone, Serialize)]
pub enum ServerAction {
	RegisterResponse {
		id: usize,
	},
	HostGameResponse {
		id: usize,
		game_name: String,
	},
	JoinGameResponse {
		success: bool,
		game_id: usize,
	},
	Steady,
	StateCheckResponse(StateCheckResponse),
	BadRequest,
}

#[derive(Debug, Clone, Serialize)]
pub enum StateCheckResponse {
	Waiting {
		game_name: String,
	},
	InGame {
		player: usize,
		player1_score: i32,
		player2_score: i32,
	},
}

fn handle_client_action(action_str: &str, mach: Arc<Mutex<MachBackend>>) -> Result<Response<Body>, ClientError> {
	let mut mach = mach.lock().unwrap();
	let client_action = serde_json::from_str(action_str).unwrap();
	let server_action = match client_action {
		ClientAction::Register { name } => {
			let id = next_id();
			let player = Player { id };
			mach.players_map.insert(player, PlayerData {
				id: player,
				name: name.clone(),
			});
			/* if let Some(waiting) = mach.waiting_games.pop() {
				log::info!("Joining {:?} in waiting game {} with {:?}", player, waiting.game_name, waiting.player);
				let id = next_id();
				let game = Game { id };
				mach.games_map.insert(game, GameData {
					id: game,
					player1: waiting.player,
					player2: player,
					game_state: GameState {
						player1_score: 0,
						player2_score: 0,
					}
				});
			} else {
				let waiting = gen_waiting_game(player);
				log::info!("No available games putting {:?} in queue for game {}", player, waiting.game_name);
				mach.waiting_games.push(waiting);
			} */
			ServerAction::RegisterResponse {
				id: id.0,
			}
		},
		ClientAction::HostGame { player } => {
			let player = Player { id: Id(player) };
			let waiting = gen_waiting_game(player);
			mach.waiting_games.push(waiting.clone());
			
			ServerAction::HostGameResponse {
				id: waiting.id.id.0,
				game_name: waiting.game_name,
			}
		},
		ClientAction::JoinGame { player, game_name } => {
			let player = Player { id: Id(player) };
			let mut waiting = None;
			for i in 0..mach.waiting_games.len() {
				if mach.waiting_games[i].game_name == game_name {
					waiting = Some(mach.waiting_games.remove(i));
					break;
				}
			}
			
			if let Some(waiting) = waiting {
				mach.games_map.insert(waiting.id, GameData {
					id: waiting.id,
					player1: waiting.player,
					player2: player,
					game_state: GameState {
						player1_score: 0,
						player2_score: 0,
					},
				});
				ServerAction::JoinGameResponse {
					success: true,
					game_id: waiting.id.id.0,
				}
			} else {
				ServerAction::JoinGameResponse {
					success: false,
					game_id: 0,
				}
			}
		},
		ClientAction::IncreaseScore { player } => {
			let player = Player { id: Id(player) };
			for game in mach.games_map.values_mut() {
				if game.player1 == player {
					game.game_state.player1_score += 1;
					break;
				} else if game.player2 == player {
					game.game_state.player2_score += 1;
					break;
				}
			}
			ServerAction::Steady
		},
		ClientAction::StateCheck { player }=> {
			log::info!("{:?} requested a state check", player);
			let player = Player { id: Id(player) };
			let mut response_opt = None;
			for game_data in mach.games_map.values_mut() {
				if game_data.player1 == player || game_data.player2 == player {
					log::info!("Found a game matching those parameters");
					response_opt = Some(StateCheckResponse::InGame {
						player: if game_data.player1 == player { 1 } else { 2 },
						player1_score: game_data.game_state.player1_score,
						player2_score: game_data.game_state.player2_score,
					})
				}
			}
			if response_opt.is_none() {
				for waiting in &mach.waiting_games {
					if waiting.player == player {
						response_opt = Some(StateCheckResponse::Waiting {
							game_name: waiting.game_name.clone(),
						});
						break;
					}
				}
			}
			log::info!("Done checking state");
			if let Some(response) = response_opt {
				ServerAction::StateCheckResponse(response)
			} else {
				ServerAction::BadRequest
			}
		}
	};
	
	let body = Body::from(serde_json::to_string(&server_action).unwrap());
	Ok(
		Response::builder()
			.status(StatusCode::OK)
			.header(hyper::header::CONTENT_TYPE, "application/json")
			.body(body)
			.unwrap()
	)
}

fn gen_waiting_game(player: Player) -> WaitingGame {
	let id = next_id();
	let ascii_char = (id.0 % 26 + 65) as u8 as char;
	let mul = id.0 / 26 + 1;
	let mut buf = String::new();
	for _ in 0..mul {
		buf.push(ascii_char);
	}
	WaitingGame {
		id: Game { id },
		game_name: buf,
		player,
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id(usize);

pub struct MachBackend {
	players_map: HashMap<Player, PlayerData>,
	games_map: HashMap<Game, GameData>,
	waiting_games: Vec<WaitingGame>,
}

impl MachBackend {
	pub fn new() -> Self {
		Self {
			players_map: HashMap::new(),
			games_map: HashMap::new(),
			waiting_games: Vec::new(),
		}
	}
}

#[derive(Debug, Clone)]
pub struct WaitingGame {
	id: Game,
	game_name: String,
	player: Player,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Game {
	id: Id,
}

#[derive(Debug, Clone)]
pub struct GameData {
	id: Game,
	player1: Player,
	player2: Player,
	game_state: GameState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Player {
	id: Id,
}

#[derive(Debug, Clone)]
pub struct PlayerData {
	id: Player,
	name: String,
}

fn setup_logging() {
	fern::Dispatch::new()
		.format(|out, message, record| out.finish(format_args!("[{}] {}", record.level(), message)))
		.level(log::LevelFilter::Info)
		.level_for("mach_backend", log::LevelFilter::Trace)
		.chain(std::io::stderr())
		.apply()
		.unwrap();
}

static CURRENT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

pub fn next_id() -> Id {
	Id(CURRENT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}