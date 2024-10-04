# Zung Mini - Mini Rust Projects

This Library consists of varouis small rust projects that target different features of rust such as traits, lifetimes, type system etc. Each project is separated out in different modules as listed out in [Table of Contents](#table-of-contents) below.

### _Disclaimer_

_This library is intended for **learning purposes only**. While I will do my best to write the most professional code I can (with my limited coding knowledge), it is not my intention for this library to be used in any production environment._

## Table of Contents

- [Mini Project 1](#mini-project-1---progbar)
  - [Features](#features)
- [Mini Project 2](#mini-project-2---strsplit)
  - [Features](#features)
- [Mini Project 3](#mini-project-3---orst)
  - [Features](#features)
- [Usage](#usage)

# Mini Project 1 - ProgBar

**_Implementation of a progress bar over iterators based on ["Type-Driven API Design in Rust" by Will Crichton](https://www.youtube.com/watch?v=bnnacleqg6k)_**

The [`ProgBar`](https://docs.rs/zung_mini/latest/zung_mini/progbar/index.html) module is a lightweight and customizable progress bar library for Rust iterators. It allows you to easily track the progress of any iterator in your terminal with visual feedback. Whether your iterator is bounded (has a known size) or unbounded (infinite loop), `ProgBar` adapts to display the progress accordingly.

## Features

- **Simple Integration**: Easily add progress bars to any iterator with minimal code changes.
- **Supports Bounded & Unbounded Iterators**: Progress can be tracked for both finite and infinite iterators.
- **Customizable Appearance**: Modify the style and delimiters of the progress bar.
- **Real-time Terminal Display**: Live updates of the progress bar in the terminal as your iterator progresses.

# Mini Project 2 - Strsplit

**_A custom string splitting utility targeting lifetimes in rust following [Crust of Rust: Lifetime Annotations](https://www.youtube.com/watch?v=rAl-9HwD858)_**

The [`Strsplit`](https://docs.rs/zung_mini/latest/zung_mini/strsplit/index.html) module provides an efficient, iterator-based string splitting utility for Rust. It extends both `String` and `&str` types with a `strsplit` method, allowing users to split strings based on a specified delimiter and iterate over the resulting substrings lazily or collect them all at once. This is particularly useful when you need efficient and flexible string splitting behavior.

# Mini Project 3 - Orst

**\_ _Implementation of custom sorting algorithms along with a benchmark following [Crust of Rust: Sorting Algorithms](https://www.youtube.com/watch?v=h4RkCyJyXmM)_**

The [`Orst`](https://docs.rs/zung_mini/latest/zung_mini/orst/index.html) module provides an custom implementations of sorting algorithms along with a simple algorithm.

## Features

- It sorts stuff.
- Easy to use.

---

# Usage

See the [docs](https://docs.rs/zung_mini/latest/zung_mini/) for how to use each module of this library.
