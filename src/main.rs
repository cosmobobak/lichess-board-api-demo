#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

use std::io::Read;

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

    println!("{body}");
}
