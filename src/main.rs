#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(dead_code)]

use std::io::Write;

use shakmaty::{Move, uci::Uci, Position, Chess, san::San};
use log::{info, warn, debug};
use reqwest::{Response, Client};
use futures_util::StreamExt;

const LICHESS_TOKEN: &str = include_str!("../token.txt");
const LICHESS_HOST: &str = "https://lichess.org";
const SCOPES: [&str; 1] = ["board:play"];
const CLIENT_ID: &str = "lichess-board-api-demo";

// send/receive messages to/from lichess using reqwest
// make a connection and read the ongoing games

#[tokio::main]
async fn main() {
    env_logger::init();

    let client = Client::builder()
        .user_agent("lichess-board-api-demo")
        .build()
        .unwrap();

    let res = client
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
    let v: serde_json::Value = serde_json::from_str(&res).unwrap();
    // info!("{v:?}");

    let current_games = &v["nowPlaying"];
    info!("current games: {current_games:?}");

    let n_current_games = current_games.as_array().unwrap().len();
    info!("number of current games: {n_current_games}");
    if n_current_games == 0 {
        warn!("no current games, exiting.");
        return;
    }
    if n_current_games > 1 {
        warn!("more than one current game, selecting the first one.");
    }

    // get the game id
    let Some(game_id) = current_games[0].get("gameId").map(|game_id| game_id.as_str().unwrap()) else {
        warn!("no 'gameId' field in json string");
        return;
    };
    info!("current game ID: {game_id}");

    // get the color
    let colour = v["nowPlaying"][0]["color"].as_str().unwrap();
    let is_our_turn = v["nowPlaying"][0]["isMyTurn"].as_bool().unwrap();
    info!("is our turn: {is_our_turn}");
    let colour = if is_our_turn { colour } else { match colour { "white" => "black", "black" => "white", _ => panic!("invalid colour") } };
    info!("colour: {colour}");

    // stream the game
    info!("streaming game {game_id}");
    let mut stream = client
        .get(format!("{LICHESS_HOST}/api/board/game/stream/{game_id}"))
        .bearer_auth(LICHESS_TOKEN)
        .send()
        .await
        .unwrap()
        .bytes_stream();

    info!("entering game stream loop");
    let mut stm = colour;
    while let Some(line) = stream.next().await {
        let line = line.unwrap();

        if line.is_empty() {
            continue;
        }

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
        for mv in moves.split_whitespace() {
            let mv: Uci = mv.parse().unwrap();
            let mv = mv.to_move(&board).unwrap();
            board = board.play(&mv).unwrap();
        }

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

        let res = send_move_to_game(&client, game_id, &user_move).await;

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

async fn request_live_games(client: &Client) -> Response {
    client
        .get(format!("{LICHESS_HOST}/api/account/playing"))
        .bearer_auth(LICHESS_TOKEN)
        .send()
        .await
        .unwrap()
}