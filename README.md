# scoop-search

![Build](https://github.com/tokiedokie/scoop-search/workflows/Build/badge.svg)
![GitHub release (latest by date including pre-releases)](https://img.shields.io/github/v/release/tokiedokie/scoop-search?include_prereleases)
![GitHub All Releases](https://img.shields.io/github/downloads/tokiedokie/scoop-search/total)

scoop-search is a tool for a windows package manager [scoop](https://scoop.sh/)

`scoop-search` instead of `scoop search`

![sample](https://raw.githubusercontent.com/tokiedokie/scoop-search/master/images/sample.gif)

## Installation

```sh
scoop install https://raw.githubusercontent.com/tokiedokie/scoop-search/master/scoop-search.json
```

## Usage

```sh
scoop-search <query>
```
Searches both package names and executable names/aliases in all local buckets.
Matching uses case-insensitive literal substrings, not regular expressions.
Each package is printed once, with buckets and packages sorted by name.
If no local results are found, it searches package names in other known GitHub
buckets listed in Scoop's `buckets.json`. Remote failures are reported on stderr
and do not prevent searching the remaining buckets.


```sh
scoop-search --bin <query>
```
`--bin` remains accepted for compatibility; searching binaries is now the default.

```sh
scoop-search --name-only git
scoop-search --local git
scoop-search --local --name-only git
```

Use `--name-only` to search package names only, avoiding reads of unrelated
manifests. Use `--local` to disable remote requests, including when nothing
matches. Options may appear before or after the query. Use `--` before a literal
query starting with `--`. No query (or `*`) lists all local packages.

Local manifests are scanned using up to 16 worker threads, bounded by the available
CPU parallelism. Both `bucket/` and legacy repository-root layouts are supported,
including nested directories. Hidden directories and symbolic links are not
traversed. Invalid manifests are skipped. For shim arrays, only the executable
and alias are searched, not command arguments. Executable extensions remain part
of the searchable name.

Scoop's root is resolved from `SCOOP`, then `root_path` in
`$XDG_CONFIG_HOME/scoop/config.json` (defaulting to
`$USERPROFILE/.config/scoop/config.json`), then `$USERPROFILE/scoop`.
The legacy `rootPath` key is also accepted.

## Development

Requires Rust 1.70 or newer. Tests use temporary fixtures and do not require a
Scoop installation or network access.

```sh
cargo build --release --locked
cargo test --locked
cargo fmt -- --check
```
