# Riplex

> A simple filesystem search engine for finding local files.

## Overview
Riplex is a tool for local storage. It's like the offline, file-centric parallel of what we normally think of as a search engine: it can find files by name — and by type — inside a file system. It becomes most useful as storage trees grow.

## Features
- **Concurrent Scanning:** It uses several threads to speed up the search, very useful for massive storage devices.
- **File Type Filtering:** It can search for files of a given type based on a collection of file extensions for several types of data. As an aid, it always searches for directories regardless of file type filters.
- **Search UI/UX:** While not designed to be scriptable, it has clear, readable output providing reliable information for longer searches — for example, a progress bar and a counter for scanned files.

## Build
To build the tool from source, clone the repo:

```
git clone https://github.com/durakitus/riplex.git
cd riplex
cargo build --release
```

## Usage
A few example commands could be:
- `riplex <filename>` — a simple, direct search for a filename, using the current working directory by default.
- `riplex -o -f <file_type> -s <path_to_search_directory>` — a search mode that shows absolute paths with the first option. The «file type» can be one of the following: "video", "audio", "image", "text", "code".

Run `riplex -h` — or use `cargo run -- -h` from inside the project folder if you haven't copied it to your `PATH` — for more information on usage, if you decide to build it locally.
