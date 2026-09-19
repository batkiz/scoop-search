use std::error::Error;

mod app;
mod bucket;
pub mod scoop;
use app::App;
use bucket::Bucket;
use scoop::Scoop;

#[derive(Debug, PartialEq)]
pub struct Args {
    pub query: String,
    pub exclude_bin: bool,
    pub local_only: bool,
}

pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Args, &'static str> {
    let mut result = Args {
        query: String::new(),
        exclude_bin: false,
        local_only: false,
    };
    let mut query = None;
    let mut options = true;
    for arg in args.into_iter().skip(1) {
        match arg.as_str() {
            "--" if options => options = false,
            "--bin" if options => result.exclude_bin = false,
            "--name-only" if options => result.exclude_bin = true,
            "--local" if options => result.local_only = true,
            _ if options && arg.starts_with("--") => return Err(
                "Unknown option. Use --bin, --name-only, --local, or -- before a literal query.",
            ),
            _ => {
                if query.replace(arg).is_some() {
                    return Err("Expected at most one query.");
                }
            }
        }
    }
    result.query = query
        .filter(|s| s != "*")
        .unwrap_or_default()
        .to_lowercase();
    Ok(result)
}

pub fn run(scoop: &Scoop, args: &Args) -> Result<(), Box<dyn Error>> {
    let paths = Bucket::paths(scoop)?;
    let mut buckets = Bucket::search(&paths, &args.query, args.exclude_bin)?;
    if buckets.is_empty() && !args.local_only {
        buckets = Bucket::remote(scoop, &paths, &args.query);
        if !buckets.is_empty() {
            println!("Results from other known buckets...");
            println!("(add them using 'scoop bucket add <name>')\n");
        }
    }
    if buckets.is_empty() {
        println!("No matches found.");
    }
    for bucket in buckets {
        display_apps(&bucket.name, &bucket.apps);
    }
    Ok(())
}

fn display_apps(bucket_name: &str, apps: &[App]) {
    println!("'{}' bucket:", bucket_name);
    for app in apps {
        if app.version.is_empty() {
            println!("    {}", app.name);
        } else if let Some(bin) = app.bin.first() {
            println!("    {} ({}) --> includes '{}'", app.name, app.version, bin);
        } else {
            println!("    {} ({})", app.name, app.version);
        }
    }
    println!();
}

#[cfg(test)]
mod tests;
