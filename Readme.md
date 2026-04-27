# light-epub

A specialized Rust library for memory-efficient EPUB parsing in `no_std` environments.

This library provides access to EPUB resources by manually parsing ZIP structures and XML streams. 

## Features

* **Manual ZIP Parsing**: Uses `nom` to parse the ZIP Central Directory, allowing for cached file offsets and faster resource lookups without walking the entire archive.
* **Efficient Memory Usage**: Leverages `Cow<'a, [u8]>` to return borrowed data from the source buffer when files are uncompressed (**Stored**).
* **Scratch Buffer Support**: Allows providing a pre-allocated buffer for **DEFLATE** decompression to manage memory manually and avoid heap fragmentation.
* **Dual TOC Support**: Basic parsing for both EPUB 2 (**NCX**) and EPUB 3 (**NAV**) navigation structures.
* **`no_std` Support**: Core logic is independent of std, requiring only `alloc` for metadata and container structures.


## Current State

* **Standard EPUBs only**: Optimized for standard OCF/OPF structures.
* **Compression**: Supports `Stored` (0) and `Deflate` (8) methods via `miniz_oxide`.
* **Encryption**: Does not support DRM or encrypted EPUB containers.
* **Validation**: Performs basic OCF compliance checks (mimetype validation), but is not a full-suite EPUB validator.


## Usage

### Metadata Extraction
Quickly grab metadata without keeping the full book structure in memory.

```rust
let metadata = Book::get_metadata(epub_bytes)?;
println!("Title: {}, Author: {}", metadata.title, metadata.author);
```

## Resource Access

Access chapters or assets. If a scratch buffer is provided, decompression happens within that buffer to save on allocations.

```rust
let book = Book::new(epub_bytes)?;

// Access a chapter. 
// Returns Cow::Borrowed if the zip entry is uncompressed (Stored).
let chapter = book.get_chapter(epub_bytes, 0, None)?;

// Use a reusable scratch buffer to manage memory manually
let mut scratch = [0u8; 65536];
let chapter = book.get_chapter(epub_bytes, 0, Some(&mut scratch))?;
```



## Roadmap

The following features are being considered for future releases:

* **Detection of Encrypted Resources**: Identifying items in the manifest that require decryption.
* **DRM Support Hooks**: Investigating a trait-based system to allow external decryption providers (e.g., for Readium LCP) without bloating the `no_std` core.
* **Enhanced EPUB 3 Support**: Better handling of Media Overlays and advanced metadata fields.
* **Expanded Search**: Utilities for locating specific IDs or properties within the manifest.

## License

This project is licensed under either the **MIT** or **Apache-2.0** license at your option.