
use std::io::{Write, BufRead, BufReader};

use log::{info, error};

use shakmaty::{Position, fen::Fen, san::San};

use crate::{gametype::VsComputer, user::{self, ChallengeColour}, VIRIDITHAS_EXECUTABLE_PATH, MAIA_EXECUTABLE_PATH};

pub fn main() {
    let (uname, schema) = user::get_challenge_schema::<VsComputer>();

    let executable_path = match uname.as_str() {
        "v" | "viridithas" => VIRIDITHAS_EXECUTABLE_PATH,
        "m" | "maia" => MAIA_EXECUTABLE_PATH,
        _ => {
            error!("invalid username for computer opponent: {uname}");
            return;
        }
    };

    info!("launching engine: {executable_path}");

    // launch the engine process with stdout and stdin pipes
    let mut engine = std::process::Command::new(executable_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to start engine");
    
    let mut game_state = shakmaty::Chess::default();
    let human_turn = match schema.color {
        ChallengeColour::White => shakmaty::Color::White,
        ChallengeColour::Black => shakmaty::Color::Black,
        ChallengeColour::Random => {
            if rand::random() {
                shakmaty::Color::White
            } else {
                shakmaty::Color::Black
            }
        }
    };

    loop {
        if game_state.is_game_over() {
            // eof
            print!("\x04");
            break;
        }
        if human_turn == game_state.turn() {
            let mut line = String::new();
            let mv = loop {
                line.clear();
                std::io::stdin().read_line(&mut line).unwrap();
                let Ok(san) = line.trim().parse::<shakmaty::san::San>() else {
                    error!("invalid SAN format: {line}");
                    continue;
                };
                let Ok(mv) = san.to_move(&game_state) else {
                    error!("illegal SAN move: {line}");
                    continue;
                };
                break mv;
            };
            game_state.play_unchecked(&mv);
        } else {
            let mvstr = get_engine_move(&mut engine, &Fen::from_position(game_state.clone(), shakmaty::EnPassantMode::Legal).to_string(), 1000);
            info!("engine move: {mvstr}");
            let uci = mvstr.parse::<shakmaty::uci::Uci>().unwrap();
            let mv = uci.to_move(&game_state).unwrap();
            let san = San::from_move(&game_state, &mv);
            game_state.play_unchecked(&mv);
            println!("{san}");
        }
    }

    // wait for the engine to finish
    info!("waiting for engine to finish");
    let status = engine.wait().unwrap();

    // print the engine's exit status
    info!("engine exited.");
    info!("Engine exited with status: {status}");
}

fn send_line(process: &mut std::process::Child, line: &str) {
    process
        .stdin
        .as_mut()
        .unwrap()
        .write_all(line.as_bytes())
        .unwrap();
}

fn get_engine_move(process: &mut std::process::Child, fen: &str, time: u64) -> String {
    info!("sending command: position fen {fen}");
    send_line(process, &format!("position fen {fen}\n"));
    info!("sending command: go movetime {time}");
    send_line(process, &format!("go movetime {time}\n"));
    let stdout = process.stdout.as_mut().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    loop {
        line.clear();
        reader.read_line(&mut line).unwrap();
        if line.starts_with("bestmove") {
            break line.split_whitespace().nth(1).unwrap().to_string();
        }
    }
}