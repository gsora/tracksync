# `tracksync`: if `rsync` was ID3-aware

`tracksync` synchronizes music files from a source to a destination, keeping a database for both.

Tracks need to be added to the source database first, and then can be synced on the destination.

There is no maximum amount of destinations you can have, each one will maintain its database and can be kept in sync
with the source.

The destination tree structure is ordered by artist, album, disc number, and track name, as detailed by each ID3 tag.

Pass `-h` to each subcommand to understand how to use it!

`tracksync` can also create hardlinks instead of copies of your files: pass the `--link` flag to `sync` to do so.

## Installing

```sh
cargo install tracksync
```

### From sources

You need a [Rust](https://rustup.rs/) compiler.

Once you have that setup:

```sh
git clone https://github.com/gsora/tracksync
cd tracksync
cargo build --release
./target/release/tracksync -h
```

## Filtering

You might want to exclude some tracks from the syncing process, based on various assumption.

Since this tool has been built primarily for my own consumption, I added a programmable way of defining filters.

Each destination can contain a filter written in the [Rhai](https://rhai.rs/): you have the full
power of a regex matching function -- `regex_match` -- and a Turing-complete programming language, have fun!

Filters can be created in-place or read from a file.

Each filter must define the `filter(track)` function in order to be evaluated:

```rhai
fn filter(track) {
  // your logic goes here
  true
}
```

As you can see, `filter` returns a boolean value:
 - `true`: copy this track
 - `false`: the opposite

The `track` argument is an object that contains the following fields:

```rust
pub struct BaseTrack {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub number: i64,
    pub file_path: String,
    pub disc_number: i64,
    pub disc_total: i64,
    pub extension: String,
}
```

As you can see, there's lots of stuff you can do with this functionality.

For example, here's a filter I built to avoid copying instrumental tracks from special edition albums:

```rhai
fn filter(track) {
	track.title.make_lower();
	track.artist.make_lower();

	let excluded_artists = [
		"periphery",
		"i built the sky",
		"anup sastry",
		"louis cole",
		"vulfpeck"
	];

	for ea in excluded_artists {
		if track.artist == ea {
			return false
		}
	}

	regex_match("instru*", track.title)
}
```

## A note on stability

This is the first CLI tool I wrote in Rust, as a way of making myself familiar with the language: expect bugs.

The database schema might break suddenly, making your source and destination(s) libraries unusable: a `rescan` command
is in the works -- I will make sure to keep those at a minimum.
