#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

mod cliargs;
mod lichess;
mod engine;

pub const LICHESS_TOKEN: &str = include_str!("../token.txt");
pub const LICHESS_HOST: &str = "https://lichess.org";

// send/receive messages to/from lichess using reqwest
// make a connection and read the ongoing games

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = <cliargs::Cli as clap::Parser>::parse();

    if args.debug {
        log::set_max_level(log::LevelFilter::Debug);
    }

    if args.lichess {
        lichess::main().await;
    }

    if let Some(engine_path) = args.engine {
        log::debug!("engine path: {engine_path}");
        engine::main(engine_path);
    }
}
