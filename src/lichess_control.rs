use std::str::FromStr;

use futures_util::StreamExt;
use log::{debug, error, info, warn};

use super::{LICHESS_HOST, LICHESS_TOKEN};

use reqwest::Client;
use reqwest::Response;
use serde::Serialize;
use shakmaty::Move;

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
            "white" => Ok(Self::White),
            "black" => Ok(Self::Black),
            "random" => Ok(Self::Random),
            _ => Err(()),
        }
    }
}

#[derive(Serialize)]
pub struct ChallengeSchema {
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

pub async fn send_move_to_game(client: &Client, game_id: &str, mv: &Move) -> Response {
    client
        .post(format!(
            "{LICHESS_HOST}/api/board/game/{game_id}/move/{}",
            mv.to_uci(shakmaty::CastlingMode::Standard)
        ))
        .bearer_auth(LICHESS_TOKEN)
        .send()
        .await
        .unwrap()
}

pub async fn send_challenge(client: &Client, username: &str, schema: &ChallengeSchema) -> Response {
    info!("sending challenge to {username}");
    let body = serde_urlencoded::to_string(schema).unwrap();
    debug!("challenge schema: {body}");
    client
        .post(format!("{LICHESS_HOST}/api/challenge/{username}"))
        .bearer_auth(LICHESS_TOKEN)
        .body(body)
        .send()
        .await
        .unwrap()
}

pub fn get_challenge_options_from_user() -> (String, ChallengeSchema) {
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
    let clock_limit = time_control
        .split('+')
        .next()
        .unwrap()
        .parse::<u32>()
        .unwrap()
        * 60;
    let clock_increment = time_control
        .split('+')
        .last()
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let keep_alive_stream = true;
    (
        username,
        ChallengeSchema {
            rated,
            clock_limit,
            clock_increment,
            color: colour,
            variant: "standard".to_string(),
            fen: None,
            keep_alive_stream,
        },
    )
}

pub async fn create_new_game(client: &Client) -> Option<String> {
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
            game_id = v["gameId"].as_str().map(ToString::to_string);
            break;
        }
    }
    game_id.map_or_else(
        || {
            warn!("no game ID received, exiting.");
            None
        },
        |game_id| {
            info!("game ID: {game_id}");
            Some(game_id)
        },
    )
}

pub async fn join_game(
    client: &Client,
    n_current_games: usize,
    default_game: &serde_json::Value,
) -> Option<String> {
    println!("create a new game or join an existing one? [C|J] (you have {n_current_games} ongoing game{})", if n_current_games == 1 { "" } else { "s" });
    let mut user_input = String::new();
    std::io::stdin().read_line(&mut user_input).unwrap();
    let user_input = user_input.trim().to_lowercase();
    if user_input == "c" {
        loop {
            let game_id = create_new_game(client).await;
            if let Some(game_id) = game_id {
                return Some(game_id);
            }
            println!("error creating game, try again? [Y|N]");
            let mut user_input = String::new();
            std::io::stdin().read_line(&mut user_input).unwrap();
            let user_input = user_input.trim().to_lowercase();
            if user_input != "y" {
                return None;
            }
        }
    } else if user_input == "j" {
        if n_current_games == 0 {
            error!("no ongoing games, exiting.");
            return None;
        } else if n_current_games > 1 {
            warn!("more than one current game, selecting the first one.");
        }
        // get the game id
        let Some(game_id) = default_game.get("gameId").map(|game_id| game_id.as_str().unwrap()) else {
            error!("no 'gameId' field in json string ({json}), exiting.", json = default_game);
            return None;
        };
        return Some(game_id.to_string());
    } else {
        error!("invalid input, exiting.");
        return None;
    };
}
