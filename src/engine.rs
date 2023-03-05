
use std::io::{Write, BufRead};

use log::info;

pub fn main(executable_path: String) {
    info!("launching engine: {executable_path}");

    // launch the engine process with stdout and stdin pipes
    let mut engine = std::process::Command::new(executable_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to start engine");

    // launch a depth-10 search by passing the command
    // "go depth 10" to the engine
    info!("process open, sending command: go depth 10");
    engine
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"go depth 10\n")
        .unwrap();

    // read the engine's output
    info!("reading engine output");
    let mut output = Vec::new();
    let stdout = engine
        .stdout
        .as_mut()
        .unwrap();
    let mut reader = std::io::BufReader::new(stdout);
    reader.read_until(b'\n', &mut output).unwrap();
    let output = String::from_utf8(output).unwrap();

    // print the engine's output
    info!("received engine output.");
    println!("{output}");

    // send "quit" to the engine to terminate it
    info!("sending command: quit");
    engine
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"quit\n")
        .unwrap();

    // wait for the engine to finish
    info!("waiting for engine to finish");
    let status = engine.wait().unwrap();

    // print the engine's exit status
    info!("engine exited.");
    println!("Engine exited with status: {status}");
}