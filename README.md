# scoop-search

![Build](https://github.com/batkiz/scoop-search/actions/workflows/build.yml/badge.svg)
![GitHub release](https://img.shields.io/github/v/release/batkiz/scoop-search)
![GitHub downloads](https://img.shields.io/github/downloads/batkiz/scoop-search/total)

scoop-search is a tool for a windows package manager [scoop](https://scoop.sh/)

`scoop-search` instead of `scoop search`

![sample](https://raw.githubusercontent.com/tokiedokie/scoop-search/master/images/sample.gif)

## Installation

```sh
scoop install https://raw.githubusercontent.com/batkiz/scoop-search/master/scoop-search.json
```

## Usage

Windows x64 binaries and SHA256 checksums are also available on the
[Releases page](https://github.com/batkiz/scoop-search/releases).

```sh
scoop-search <query>
```
Searches both package names and executable names/aliases in all local buckets.
Normal matching uses case-insensitive literal substrings, not regular expressions.
Each package is printed once, with buckets and packages sorted by name.
By default, all searches and fuzzy suggestions use local buckets only, with no
remote requests. Pass `--remote` to search package names in other known GitHub
buckets listed in Scoop's `buckets.json` when the local search has no results.
Remote failures are reported on stderr and do not prevent searching the remaining
buckets. When the enabled searches find no literal match, fuzzy suggestions are
printed instead.


```sh
scoop-search --bin <query>
```
`--bin` remains accepted for compatibility; searching binaries is now the default.

```sh
scoop-search --name-only git
scoop-search --local git
scoop-search --local --name-only git
scoop-search --remote git
```

Use `--name-only` to search package names only, avoiding reads of unrelated
manifests. Use `--remote` to enable remote fallback. `--local` is still accepted
and explicitly selects the default local-only behavior. If both are supplied,
the last option wins. Options may appear before or after the query. Use `--` before a literal
query starting with `--`. No query (or `*`) lists all local packages.

### Fuzzy search

```sh
# No literal match: automatically suggests ripgrep and other close names.
scoop-search --local ripgrp

# Explicitly include similar names even when literal matches exist.
scoop-search --local --fuzzy ripgrep

# Restrict candidates to package names, excluding commands and aliases.
scoop-search --local --fuzzy --name-only gti

# Ordered abbreviations: rg -> ripgrep, vsc -> visual-studio-code.
scoop-search --local --fuzzy --name-only rg
scoop-search --local --fuzzy --name-only vsc
```

Automatic fallback prints `No literal matches found. Did you mean?`.
Explicit `--fuzzy` prints `Fuzzy matches`. Both show at most 10 candidates as
`bucket/package`, ranked across buckets, with deterministic name/bucket tie
breaking. A package appears only once per bucket. Literal results retain their
existing output format and are not mixed with automatic suggestions.

Ranking prefers exact names, prefixes, other substrings, names differing only
in hyphens/underscores/spaces, then spelling corrections, then ordered subsequence
matches. Spelling correction uses
[`strsim::osa_distance`](https://docs.rs/strsim/latest/strsim/fn.osa_distance.html)
instead of a custom distance implementation. Corrections allow
insertion, deletion, substitution, and adjacent-character transposition:
one edit for 3–5 characters, or up to two edits for 6–128 characters. Shorter and
longer queries still support literal and separator-insensitive matching but do
not receive typo corrections. Matching is case-insensitive and handles Unicode
characters.

Ordered subsequence matching uses
[`nucleo-matcher`](https://docs.rs/nucleo-matcher/), Nucleo's core matching library:
`rg` can find `ripgrep`, and `vsc` can find `visual-studio-code`. Characters must
appear in the same order. Within this group, higher Nucleo scores rank first;
shorter names break score ties. Subsequence matching applies to queries of 2–128
characters containing at least one letter or number. One-character and
punctuation-only queries do not gain extra subsequence candidates.

Both engines apply to automatic suggestions and explicit `--fuzzy` searches,
including executable names/aliases and remote package names. Nucleo's matcher
and Unicode conversion buffer are reused per worker thread. The high-level
interactive `nucleo` picker is not needed for this command-line search.

Command candidates also match without common executable suffixes (`.exe`, `.cmd`,
`.bat`, `.ps1`, `.com`, `.sh`). `--name-only` disables command candidates.
An empty query or `*` still lists every package, even with `--fuzzy`.

Explicit fuzzy search checks local buckets first and consults known remote
buckets only if `--remote` is supplied and there are no local candidates.
Automatic fallback waits until the enabled literal search finishes, then ranks
local and, with `--remote`, already-fetched remote candidates together. Remote suggestions search
package names only and include the command needed to add their bucket; no second
network request is made for fuzzy matching.

The suggestion experience is inspired by
[Homebrew search](https://github.com/Homebrew/brew/blob/main/Library/Homebrew/search.rb).
The ranking and edit-distance rules above are specific to this implementation.

Local manifests are scanned using up to 16 worker threads, bounded by the available
CPU parallelism. Both `bucket/` and legacy repository-root layouts are supported,
including nested directories. Hidden directories and symbolic links are not
traversed. Invalid manifests are skipped. For shim arrays, only the executable
and alias are searched, not command arguments. Executable extensions remain part
of the searchable name in normal literal searches.

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

## Publishing a release

1. Update the version in `Cargo.toml` and run `cargo check` to update `Cargo.lock`.
2. Add release notes at `docs/releases/v<VERSION>.md` and commit the changes.
3. Push the commit, then create and push its annotated tag:

   ```sh
   git push origin master
   git tag -a v<VERSION> -m "Release v<VERSION>"
   git push origin v<VERSION>
   ```

4. The Release workflow validates the tag/version, runs checks, builds Windows
   x64, and publishes `scoop-search.exe` and `SHA256SUMS`. Inspect the workflow
   result before proceeding.
5. Download the published assets and verify the executable's SHA256. Update
   `scoop-search.json` with the released version, URL, and verified hash; commit
   and push the manifest update. Use the published binary's hash, not a local
   build's hash. Release tags remain on the source commit used for the build.

This fork is based on [tokiedokie/scoop-search](https://github.com/tokiedokie/scoop-search).
