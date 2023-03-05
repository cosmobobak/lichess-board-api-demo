

pub fn main(executable_path: String) {
    // launch the engine process
    let mut engine = std::process::Command::new(executable_path)
        .spawn()
        .expect("Failed to start engine");

    // launch a depth-10 search by passing the command
    // "go depth 10" to the engine
    engine
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"go depth 10\n")
        .unwrap();

    // read the engine's output
    let mut output = String::new();
    engine
        .stdout
        .as_mut()
        .unwrap()
        .read_to_string(&mut output)
        .unwrap();

    // print the engine's output
    println!("{}", output);

    // wait for the engine to finish
    engine.wait().unwrap();

    // print the engine's exit status
    println!("Engine exited with status: {}", engine);
}