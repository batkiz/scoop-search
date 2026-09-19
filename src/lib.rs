use std::error::Error;

mod app;
mod bucket;
mod fuzzy;
mod installed;
pub mod scoop;
use app::App;
use bucket::Bucket;
use scoop::Scoop;

#[derive(Debug, PartialEq)]
pub struct Args {
    pub query: String,
    pub exclude_bin: bool,
    pub local_only: bool,
    pub fuzzy: bool,
}

pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Args, &'static str> {
    let mut result = Args {
        query: String::new(),
        exclude_bin: false,
        local_only: true,
        fuzzy: false,
    };
    let mut query = None;
    let mut options = true;
    for arg in args.into_iter().skip(1) {
        match arg.as_str() {
            "--" if options => options = false,
            "--bin" if options => result.exclude_bin = false,
            "--name-only" if options => result.exclude_bin = true,
            "--local" if options => result.local_only = true,
            "--remote" if options => result.local_only = false,
            "--fuzzy" if options => result.fuzzy = true,
            _ if options && arg.starts_with("--") => return Err(
                "Unknown option. Use --bin, --name-only, --local, --remote, --fuzzy, or -- before a literal query.",
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
    let installed = installed::Installed::load(scoop);
    // Empty queries still list everything, even with --fuzzy.
    if args.fuzzy && !args.query.is_empty() {
        let mut suggestions = Bucket::fuzzy_search(&paths, &args.query, args.exclude_bin)?;
        if suggestions.is_empty() && !args.local_only {
            suggestions = Bucket::remote(scoop, &paths, &args.query).1;
            if !suggestions.is_empty() {
                println!(
                    "Results from other known buckets (add with 'scoop bucket add <name>')..."
                );
            }
        }
        display_suggestions(&suggestions, false, &installed);
        return Ok(());
    }
    let mut buckets = Bucket::search(&paths, &args.query, args.exclude_bin)?;
    let mut suggestions = Vec::new();
    if buckets.is_empty() && !args.local_only {
        (buckets, suggestions) = Bucket::remote(scoop, &paths, &args.query);
        if !buckets.is_empty() {
            println!("Results from other known buckets...");
            println!("(add them using 'scoop bucket add <name>')\n");
        }
    }
    if buckets.is_empty() {
        suggestions.extend(Bucket::fuzzy_search(&paths, &args.query, args.exclude_bin)?);
        bucket::rank_suggestions(&mut suggestions);
        display_suggestions(&suggestions, true, &installed);
        return Ok(());
    }
    for bucket in buckets {
        display_apps(&bucket.name, &bucket.apps, &installed);
    }
    Ok(())
}

fn display_suggestions(
    suggestions: &[bucket::Suggestion],
    fallback: bool,
    installed: &installed::Installed,
) {
    if suggestions.is_empty() {
        println!("No matches found.");
        return;
    }
    if fallback {
        println!("No literal matches found. Did you mean? (up to 10 results)");
    } else {
        println!("Fuzzy matches (up to 10 results):");
    }
    for suggestion in suggestions {
        let app = &suggestion.app;
        print!("    {}/{}", suggestion.bucket, app.name);
        if !app.version.is_empty() {
            print!(" ({})", app.version);
        }
        if let Some(bin) = app.bin.first() {
            print!(" --> includes '{}'", bin);
        }
        if suggestion.remote {
            print!(" [add bucket: scoop bucket add {}]", suggestion.bucket);
        }
        println!("{}", installed.label(&suggestion.bucket, &app.name));
    }
}

fn display_apps(bucket_name: &str, apps: &[App], installed: &installed::Installed) {
    println!("'{}' bucket:", bucket_name);
    for app in apps {
        if app.version.is_empty() {
            print!("    {}", app.name);
        } else if let Some(bin) = app.bin.first() {
            print!("    {} ({}) --> includes '{}'", app.name, app.version, bin);
        } else {
            print!("    {} ({})", app.name, app.version);
        }
        println!("{}", installed.label(bucket_name, &app.name));
    }
    println!();
}

#[cfg(test)]
mod tests;
