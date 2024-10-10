# Zung Parsers - Data Format Parsing in Rust

This Library consists of varouis feature-full (on going) data format parsers implemented in rust. Each project is separated out in different modules as listed out in [Table of Contents](#table-of-contents) below.

## Zung Family

This library is part of the [zung](https://github.com/ishaan26/zung) family.
Install the zung binary with `cargo install zung` to try out some of the features of this
library.

## Disclaimer

_This library is intended for **learning purposes only**. While I will do my best to write the most professional code I can (with my limited coding knowledge), it is not my intention for this library to be used in any production environment._

Ateast not yet...

## Table of Contents

- [Bencode](#bencode)
  - [Features](#features)

# Bencode

**_Encode and decode data in the [Bencode](https://en.wikipedia.org/wiki/Bencode) format._**

Bencode is a simple binary encoding format used in various contexts, most notably in
BitTorrent. This type provides functionality to encode Rust data structures into Bencode format
and decode Bencode strings into Rust data structures or json or yaml. See the implemented
methods for more information,

## Features

- Full [serde](https://serde.rs) support.
- Good error reporting (I tried).
- [`Value`](https://docs.rs/zung_parsers/latest/zung_parsers/bencode/enum.Value.html) implementation to reprasent parsed bencode data in rust data types.

# Usage

See the [docs](https://docs.rs/zung_parsers/latest/zung_parsers/) for how to use each module of this library.
