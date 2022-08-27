use clap::{Arg, Command};

mod fox;
mod config;

fn main() {
    let matches = Command::new("fox")
        .about("simple code editor")
        .version(env!("CARGO_PKG_VERSION"))
        .arg_required_else_help(true)
        .author("lucky4luuk")
        .subcommand(
            Command::new("help")
                .about("help page")
        )
        .arg(
            Arg::new("filename")
        )
        .get_matches();

    if let Some(filename) = matches.get_one::<String>("filename") {
        fox::run(filename).expect("Failed to run fox editor!");
    } else {
        panic!("How did we get here?");
    }
}
