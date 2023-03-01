#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

mod cliargs;
mod lichess_control;

use std::io::Write;

use futures_util::StreamExt;
use log::{debug, info, warn};
use reqwest::Client;
use shakmaty::{san::San, uci::Uci, Chess, Position};

pub const LICHESS_TOKEN: &str = include_str!("../token.txt");
pub const LICHESS_HOST: &str = "https://lichess.org";

// send/receive messages to/from lichess using reqwest
// make a connection and read the ongoing games

#[allow(clippy::too_many_lines)]
#[tokio::main]
async fn main() {
    env_logger::init();

    info!("creating reqwest client");
    let client = Client::builder()
        .user_agent("flagfall-lichess-api")
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

    // parse the json string
    let v: serde_json::Value = serde_json::from_str(&ongoing_games).unwrap();
    // info!("{v:?}");

    let current_games = &v["nowPlaying"];
    info!("current games: {current_games:?}");

    let n_current_games = current_games.as_array().unwrap().len();
    info!("number of currently active games: {n_current_games}");
    let game_id = lichess_control::join_game(&client, n_current_games, &current_games[0])
        .await
        .unwrap();

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
    let colour = if is_our_turn {
        colour
    } else {
        match colour {
            "white" => "black",
            "black" => "white",
            _ => panic!("invalid colour"),
        }
    };
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
        let Some(moves) = v.get("state")
            .map_or_else(
                || v.get("moves").and_then(serde_json::Value::as_str), 
                |state| state.get("moves").and_then(serde_json::Value::as_str)
        ) else {
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

        let res = lichess_control::send_move_to_game(&client, &game_id, &user_move).await;

        let body = res.text().await.unwrap();

        info!("{body}");

        stm = match stm {
            "white" => "black",
            "black" => "white",
            _ => panic!("invalid stm"),
        };
    }
}
