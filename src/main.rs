#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(dead_code)]

use std::io::{Read, Write};

const LICHESS_TOKEN: &str = include_str!("../token.txt");
const LICHESS_HOST: &str = "https://lichess.org";
const SCOPES: [&str; 1] = ["board:play"];
const CLIENT_ID: &str = "lichess-board-api-demo";

// send/receive messages to/from lichess using reqwest
// make a connection and read the ongoing games

fn main() {
    let client = reqwest::blocking::Client::builder()
        .user_agent("lichess-board-api-demo")
        .build()
        .unwrap();

    let mut res = client
        .get(format!("{LICHESS_HOST}/api/account/playing"))
        .bearer_auth(LICHESS_TOKEN)
        .send()
        .unwrap();
    
    let mut body = String::new();
    res.read_to_string(&mut body).unwrap();

    // result is in the form of a json string
    // like this: {"nowPlaying":[{"fullId":"Pee5EWwqV5tu","gameId":"Pee5EWwq","fen":"rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq - 0 1","color":"black","lastMove":"d2d4","source":"friend","variant":{"key":"standard","name":"Standard"},"speed":"classical","perf":"classical","rated":false,"hasMoved":false,"opponent":{"id":"maia5","username":"BOT maia5","rating":1555},"isMyTurn":true,"secondsLeft":3600}]}
    // println!("{body}");

    // parse the json string
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    // println!("{v:?}");

    // get the game id
    let game_id = v["nowPlaying"][0]["gameId"].as_str().unwrap();
    println!("current game ID: {game_id}");

    // get the fen
    let fen = v["nowPlaying"][0]["fen"].as_str().unwrap();
    println!("fen: {fen}");

    // print the legal moves: 
    let current_position = cozy_chess::Board::from_fen(fen, false).unwrap();
    let mut legal_moves = Vec::new();
    current_position.generate_moves(|piece_moves| {
        legal_moves.extend(piece_moves);
        false // don't abort early
    });

    print!("legal moves: ");
    for m in &legal_moves {
        print!("{m}, ");
    }
    println!();

    let mut user_input = String::new();
    print!("enter move: ");
    std::io::stdout().flush().unwrap();
    std::io::stdin().read_line(&mut user_input).unwrap();
    let user_input = user_input.trim();

    if !legal_moves.iter().map(ToString::to_string).any(|m| m == user_input) {
        println!("illegal move!: {user_input}");
        return;
    }

    // post the move in the form of a json string
    // like this: https://lichess.org/api/board/game/{gameId}/move/{move}

    let mut res = client
        .post(format!("{LICHESS_HOST}/api/board/game/{game_id}/move/{user_input}"))
        .bearer_auth(LICHESS_TOKEN)
        .send()
        .unwrap();

    let mut body = String::new();
    res.read_to_string(&mut body).unwrap();

    println!("{body}");
}
