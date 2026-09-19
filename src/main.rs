use scoop_search::{parse_args, run, scoop::Scoop};
use std::{env, process};

fn main() {
    let result = parse_args(env::args())
        .map_err(Into::into)
        .and_then(|args| Scoop::new().and_then(|scoop| run(&scoop, &args)));
    if let Err(error) = result {
        eprintln!("{}", error);
        process::exit(1);
    }
}
