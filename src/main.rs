#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

mod cliargs;

use std::{io::Write, str::FromStr};

use shakmaty::{Move, uci::Uci, Position, Chess, san::San};
use log::{info, warn, debug, error};
use reqwest::{Response, Client};
use futures_util::StreamExt;

use serde::Serialize;

const LICHESS_TOKEN: &str = include_str!("../token.txt");
const LICHESS_HOST: &str = "https://lichess.org";

// send/receive messages to/from lichess using reqwest
// make a connection and read the ongoing games

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
enum ChallengeColour {
    #[serde(rename = "white")]
    White,
    #[serde(rename = "black")]
    Black,
    #[serde(rename = "random")]
    Random,
}

impl FromStr for ChallengeColour {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "white" => Ok(ChallengeColour::White),
            "black" => Ok(ChallengeColour::Black),
            "random" => Ok(ChallengeColour::Random),
            _ => Err(()),
        }
    }
}

#[derive(Serialize)]
struct ChallengeSchema {
    rated: bool,
    #[serde(rename = "clock.limit")]
    clock_limit: u32,
    #[serde(rename = "clock.increment")]
    clock_increment: u32,
    color: ChallengeColour,
    variant: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fen: Option<String>,
    #[serde(rename = "keepAliveStream")]
    keep_alive_stream: bool,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    println!("Welcome to the Lichess Board API interface.");

    info!("creating reqwest client");
    let client = Client::builder()
        .user_agent("lichess-board-api-demo")
        .build()
        .unwrap();

    info!("getting ongoing games");
    let ongoing_games = client
        .get(format!("{LICHESS_HOST}/api/account/playing"))
        .bearer_auth(LICHESS_TOKEN)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    // result is in the form of a json string
    // like this: {"nowPlaying":[{"fullId":"Pee5EWwqV5tu","gameId":"Pee5EWwq","fen":"rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq - 0 1","color":"black","lastMove":"d2d4","source":"friend","variant":{"key":"standard","name":"Standard"},"speed":"classical","perf":"classical","rated":false,"hasMoved":false,"opponent":{"id":"maia5","username":"BOT maia5","rating":1555},"isMyTurn":true,"secondsLeft":3600}]}
    // info!("{body}");

    // parse the json string
    let v: serde_json::Value = serde_json::from_str(&ongoing_games).unwrap();
    // info!("{v:?}");

    let current_games = &v["nowPlaying"];
    info!("current games: {current_games:?}");

    let n_current_games = current_games.as_array().unwrap().len();
    info!("number of currently active games: {n_current_games}");

    println!("create a new game or join an existing one? [C|J] (you have {n_current_games} ongoing game{})", if n_current_games == 1 { "" } else { "s" });
    let mut user_input = String::new();
    std::io::stdin().read_line(&mut user_input).unwrap();
    let user_input = user_input.trim().to_lowercase();
    let game_id = if user_input == "c" {
        loop {
            let game_id = create_new_game(&client).await;
            if let Some(game_id) = game_id {
                break game_id;
            }
            println!("error creating game, try again? [Y|N]");
            let mut user_input = String::new();
            std::io::stdin().read_line(&mut user_input).unwrap();
            let user_input = user_input.trim().to_lowercase();
            if user_input != "y" {
                return;
            }
        }
    } else if user_input == "j" {
        if n_current_games == 0 {
            error!("no ongoing games, exiting.");
            return;
        } else if n_current_games > 1 {
            warn!("more than one current game, selecting the first one.");
        }
        // get the game id
        let Some(game_id) = current_games[0].get("gameId").map(|game_id| game_id.as_str().unwrap()) else {
            error!("no 'gameId' field in json string ({json}), exiting.", json = current_games[0]);
            return;
        };
        game_id.to_string()
    } else {
        error!("invalid input, exiting.");
        return;
    };

    info!("current game ID: {game_id}");

    // stream the game
    info!("streaming game {game_id}");
    let mut stream = client
        .get(format!("{LICHESS_HOST}/api/board/game/stream/{game_id}"))
        .bearer_auth(LICHESS_TOKEN)
        .send()
        .await
        .unwrap()
        .bytes_stream();

    let game = stream.next().await.unwrap().unwrap();
    let game: serde_json::Value = serde_json::from_slice(&game).unwrap();

    // get the color
    let colour = game["color"].as_str().unwrap();
    let is_our_turn = game["isMyTurn"].as_bool().unwrap();
    info!("is our turn: {is_our_turn}");
    let colour = if is_our_turn { colour } else { match colour { "white" => "black", "black" => "white", _ => panic!("invalid colour") } };
    info!("colour: {colour}");

    info!("entering game stream loop");
    let mut stm = colour;
    while let Some(line) = stream.next().await {
        let line = line.unwrap();

        // skip newlines
        let line_ref = std::str::from_utf8(line.as_ref()).unwrap();
        if line_ref.trim().is_empty() {
            continue;
        }

        debug!("line: {line:?}");

        // parse the json string
        let v: serde_json::Value = serde_json::from_slice(&line).unwrap();
        debug!("serdejson value: {v:?}\n");

        if v.get("type").map(|t| t.as_str().unwrap()) == Some("chatLine") {
            info!("chat line: {} says {}", v["username"], v["text"]);
            continue;
        }

        // continue if we're not to move:
        if stm != colour {
            stm = match stm {
                "white" => "black",
                "black" => "white",
                _ => panic!("invalid stm"),
            };
            continue;
        }

        // get the fen
        let Some(moves) = (match v.get("state") {
            Some(state) => state.get("moves").and_then(|moves| moves.as_str()),
            None => v.get("moves").and_then(|moves| moves.as_str()),
        }) else {
            warn!("no 'moves' or 'state' field in json string");
            return;
        };
        info!("moves made so far: {moves}");
        let mut board = Chess::default();
        let moves = moves.split_whitespace().collect::<Vec<_>>();
        for mv in &moves {
            let mv: Uci = mv.parse().unwrap();
            let mv = mv.to_move(&board).unwrap();
            board = board.play(&mv).unwrap();
        }

        println!("opponent's move: {}", moves.last().unwrap());

        // print the legal moves:
        let legal_moves = board.legal_moves();

        print!("legal moves: ");
        for m in &legal_moves {
            print!("{}, ", San::from_move(&board, m));
        }
        println!();

        let mut user_input = String::new();
        print!("enter move: ");
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_line(&mut user_input).unwrap();
        let user_input = user_input.trim();

        let user_move = user_input.parse::<San>().unwrap().to_move(&board).unwrap();

        // post the move in the form of a json string
        // like this: https://lichess.org/api/board/game/{gameId}/move/{move}

        let res = send_move_to_game(&client, &game_id, &user_move).await;

        let body = res.text().await.unwrap();

        info!("{body}");

        stm = match stm {
            "white" => "black",
            "black" => "white",
            _ => panic!("invalid stm"),
        };
    }
}

async fn send_move_to_game(client: &Client, game_id: &str, mv: &Move) -> Response {
    client
        .post(format!("{LICHESS_HOST}/api/board/game/{game_id}/move/{}", mv.to_uci(shakmaty::CastlingMode::Standard)))
        .bearer_auth(LICHESS_TOKEN)
        .send()
        .await
        .unwrap()
}

async fn send_challenge(client: &Client, username: &str, schema: &ChallengeSchema) -> Response {
    info!("sending challenge to {username}");
    debug!("challenge schema: {body}", body = serde_json::to_string_pretty(schema).unwrap());
    client
        .post(format!("{LICHESS_HOST}/api/challenge/{username}"))
        .bearer_auth(LICHESS_TOKEN)
        .body(serde_json::to_string(schema).unwrap())
        .send()
        .await
        .unwrap()
}

fn get_challenge_options_from_user() -> (String, ChallengeSchema) {
    println!("enter the username to challenge:");
    let mut user_input = String::new();
    std::io::stdin().read_line(&mut user_input).unwrap();
    let username = user_input.trim().to_lowercase();
    println!("enter the time control in the form of 'min+inc' (e.g. '5+2' for 5 minutes + 2 seconds increment):");
    user_input.clear();
    std::io::stdin().read_line(&mut user_input).unwrap();
    let time_control = user_input.trim().to_lowercase();
    println!("should the game be rated? [Y|N]");
    user_input.clear();
    std::io::stdin().read_line(&mut user_input).unwrap();
    let rated = match user_input.trim().to_lowercase().as_str() {
        "y" => true,
        "n" => false,
        _ => panic!("invalid input"),
    };
    println!("enter the challenge colour [white|black|random]:");
    user_input.clear();
    std::io::stdin().read_line(&mut user_input).unwrap();
    let colour = user_input.trim().to_lowercase();
    let colour = colour.parse::<ChallengeColour>().unwrap();
    let keep_alive_stream = true;
    (username, ChallengeSchema {
        rated,
        clock_limit: time_control.split('+').next().unwrap().parse::<u32>().unwrap() * 60,
        clock_increment: time_control.split('+').last().unwrap().parse().unwrap(),
        color: colour,
        variant: "standard".to_string(),
        fen: None,
        keep_alive_stream,
    })
}

async fn create_new_game(client: &Client) -> Option<String> {
    let (username, schema) = get_challenge_options_from_user();
    let response = send_challenge(client, &username, &schema).await;
    debug!("challenge sent, response: {response:?}");
    let mut stream = response.bytes_stream();
    let mut game_id = None;
    while let Some(Ok(bytes)) = stream.next().await {
        let s = String::from_utf8_lossy(&bytes);
        debug!("received {s}");
        if s.contains("gameId") {
            let v: serde_json::Value = serde_json::from_str(&s).unwrap();
            game_id = v["gameId"].as_str().map(|s| s.to_string());
            break;
        }
    }
    if let Some(game_id) = game_id {
        info!("game ID: {game_id}");
        Some(game_id)
    } else {
        warn!("no game ID received, exiting.");
        None
    }
}