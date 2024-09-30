//! Implementation of a progress bar over iterators based on ["Type-Driven API Design in Rust" by
//! Will Crichton](https://www.youtube.com/watch?v=bnnacleqg6k)
//!
//! # Introduction
//!
//! This library provides a simple, customizable progress bar for iterators in Rust.
//! It is designed to visually track the progress of any iterator, whether it is bounded (i.e., has
//! a known size) or unbounded (i.e., no pre-determined size).
//!
//! With `ProgBar`, you can easily wrap your iterators and track their progress in the terminal.
//! The library offers flexibility in styling and customization, allowing you to modify the
//! appearance of the progress bar, such as the bar's delimiters and the style of the fill.
//!
//! # Features
//!
//! - **Easy to use**: The [`.progbar()`](ProgBarExt::progbar()) method can be applied to any
//! iterator, making progress tracking effortless.
//! - **Bounded & Unbounded Progress Bars**: Automatically handles both finite and infinite
//! iterators. Bounded bars can display percentages, while unbounded bars display the progress
//! incrementally.
//! - **Customizable Styles**: Modify the progress bar's appearance with custom delimiters and bar
//! styles.
//! - **Terminal Display**: The library displays a live progress bar in the terminal, updating in
//! real-time during iteration.
//!
//! # Example Usage
//!
//! ## Basic Progress Bar
//!
//! This example shows how to create a simple progress bar for a range of numbers:
//!
//! ```rust
//! use zung_mini::progbar::ProgBarExt;
//!
//! for i in (0..).progbar() {
//!     // Perform work
//!     println!("Processing item: {}", i);
//!     # break;
//! }
//! ```
//!
//! The output will look something like this in the terminal as the progress bar updates:
//! ```text
//! #####
//! ```
//!
//! ## Bounded Progress Bar with Custom Delimiters
//!
//! When the total length of the iterator is known, you can use the
//! [`with_bounds()`](`ProgBar::with_bounds()`) method to show percentage-based progress:
//!
//! ```rust
//! use zung_mini::progbar::ProgBarExt;
//!
//! let progbar = (0..100).progbar().with_bounds('[', ']');
//! for _ in progbar {
//!     // Perform work
//! }
//! ```
//!
//! The output will look something like this in the terminal as the progress bar updates:
//! ```text
//! [ 50%] [#####      ]
//! ```
//!
//! ### Custom Bar Style
//!
//! You can also customize the bar's appearance by using the
//! [`bar_style()`](`ProgBar::bar_style()`) method to change the fill character:
//!
//! ```rust
//! use zung_mini::progbar::ProgBarExt;
//!
//! let progbar = (0..10).progbar().bar_style("=").with_bounds('<', '>');
//! for _ in progbar {
//!     // Perform work
//! }
//! ```
//!
//! The output might look like this:
//! ```text
//! [ 30%] <===         >
//! ```

use std::cell::Cell;
use std::fmt::{Debug, Display};
use std::io::{self, Write};

// `BarStyle` is used to define the appearance of the progress bar. It contains
// a single string field that holds the character(s) used to visually represent the progress.
#[derive(Debug)]
struct BarStyle(String);

impl BarStyle {
    fn new(bar_style: String) -> Self {
        Self(bar_style)
    }
}

impl Display for BarStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for BarStyle {
    fn default() -> Self {
        Self::new(String::from("#"))
    }
}

/// Internal state of `ProgBar`. UnBounded means the Iterator is never ending. This is the default
/// state of the [`ProgBar`]. See [`ProgBar::with_bounds`] method if you want to use a [`Bounded`]
/// ProgBar.
pub struct UnBounded;

/// Internal state of `ProgBar`. Bounded means the size of the Iterator is known. This is
/// constructed with [`ProgBar::with_bounds`] method.
pub struct Bounded<D: Display> {
    len: usize,
    percentage: Cell<u8>,
    delims: (D, D),
}

/// Creates a ProgBar type where the `progbar()` method is called over any iterator.
#[derive(Debug)]
pub struct ProgBar<T, Bound> {
    iterator: T,
    step: usize,
    bound: Bound,
    bar: BarStyle,
}

impl<T> ProgBar<T, UnBounded> {
    // Generate a new [`ProgBar`] with [`UnBounded`] State.
    fn new(iterator: T) -> Self {
        Self {
            iterator,
            step: 0,
            bound: UnBounded,
            bar: BarStyle::default(),
        }
    }
}

impl<T, Bound> ProgBar<T, Bound> {
    /// Sets the style of the progress bar.
    ///
    /// This method allows customizing the appearance of the progress bar by specifying
    /// a type that implements the `BarStyle` trait. The provided `bar` parameter
    /// should be a reference to an instance of a type that implements both `Display`
    /// and `Debug` traits.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///
    /// use zung_mini::progbar::ProgBarExt;
    ///
    /// (0..100)
    ///    .progbar()
    ///    .bar_style(&String::from("="))
    ///    .with_bounds("|", "|")
    ///    .for_each(|_| {
    ///         // Do some calculation
    ///    });
    ///    
    /// // or
    ///
    /// for _ in (0..).progbar().bar_style(&"<>") {
    ///     // Do some calculation
    /// }
    ///
    /// ```
    pub fn bar_style(mut self, bar: impl Display) -> Self {
        self.bar = BarStyle::new(bar.to_string());
        self
    }
}

trait ProgBarDisplay: Sized {
    fn display<T>(&self, progress: &ProgBar<T, Self>);
}

impl ProgBarDisplay for UnBounded {
    fn display<T>(&self, progress: &ProgBar<T, Self>) {
        print!("{}", progress.bar);
        io::stdout().flush().unwrap();
    }
}

impl<D> ProgBarDisplay for Bounded<D>
where
    D: Display,
{
    fn display<T>(&self, progbar: &ProgBar<T, Self>) {
        progbar.calculate_percentage();
        if progbar.step <= 1 {
            print!("[{:>3}%] \r", 0);
        }

        print!(
            "[{:>3}%] {}{}{}{}\r",
            self.percentage.get(),
            self.delims.0,
            progbar.bar.to_string().repeat(progbar.step),
            " ".repeat(self.len - progbar.step),
            self.delims.1
        );
        io::stdout().flush().unwrap();
    }
}

// Give bounds where the iterator's exact size is known
impl<T> ProgBar<T, UnBounded>
where
    T: ExactSizeIterator,
{
    /// Converts the default [`UnBounded`] [`ProgBar`] created with [`ProgBarExt`] into a
    /// [`Bounded`] one.
    ///
    /// This method takes two delimiters (`bound_start` and `bound_end`) to define
    /// the boundaries for a progress bar. When working with an iterator whose size is
    /// known (i.e., implements [`ExactSizeIterator`]), this method is used to create
    /// a [`Bounded`] progress bar, allowing for percentage calculation and visual display
    /// with clear starting and ending points.
    ///
    /// # Arguments
    ///
    /// * `bound_start` - A character or string that marks the start of the progress bar.
    /// * `bound_end` - A character or string that marks the end of the progress bar.
    ///
    /// # Examples
    ///
    /// Creating a bounded progress bar with square brackets as delimiters:
    ///
    /// ```rust
    /// use zung_mini::progbar::ProgBarExt;
    /// let progbar = (0..10).progbar().with_bounds('[', ']');
    /// for i in progbar {
    ///     // Do some work for each iteration
    ///     println!("{}", i);
    /// }
    /// ```
    ///
    /// The progress bar will look like this in the terminal:
    ///
    /// ```text
    /// [ 30%] [###       ]
    /// ```
    ///
    /// # Requirements
    ///
    /// The iterator must implement the [`ExactSizeIterator`] trait because the method
    /// requires knowledge of the total number of elements in the iterator to calculate
    /// and display the progress accurately.
    pub fn with_bounds<D>(self, bound_start: D, bound_end: D) -> ProgBar<T, Bounded<D>>
    where
        D: Display,
    {
        let bound = Bounded {
            len: self.iterator.len(),
            percentage: Cell::new(0),
            delims: (bound_start, bound_end),
        };

        ProgBar {
            iterator: self.iterator,
            step: self.step,
            bound,
            bar: self.bar,
        }
    }
}

impl<T, D> ProgBar<T, Bounded<D>>
where
    D: Display,
{
    fn calculate_percentage(&self) {
        self.bound
            .percentage
            .set(((self.step as f64 / self.bound.len as f64) * 100.0) as u8);
    }
}

/// A trait that extends any iterator to support progress bar functionality.
///
/// `ProgBarExt` serves as the foundation of the progress bar library, allowing any iterator
/// to be easily wrapped in a progress bar. By implementing this trait, any iterator can
/// be transformed into a `ProgBar` with zung_minimal effort, using the `.progbar()` method.
///
/// The progress bar created by [.progbar()](`ProgBarExt::progbar()`) starts in an unbounded state, meaning it
/// doesn't initially know the total size of the iteration. However, for iterators that
/// implement `ExactSizeIterator`, the progress bar can be converted into a bounded one
/// using the [`with_bounds()`](`ProgBar::with_bounds()`) method, which enables percentage-based progress tracking.
///
/// # Usage
///
/// To use [`ProgBarExt`], simply import the trait and call [`.progbar()`] on any iterator:
///
/// ```rust
/// use zung_mini::progbar::ProgBarExt;
///
/// for i in (0..10).progbar() {
///     // Do some work with the iterator
///     println!("{}", i);
/// }
/// ```
///
/// This will display a progress bar in the terminal that updates as the iterator progresses.
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust
/// use zung_mini::progbar::ProgBarExt;
///
/// // Using progbar on a range
/// for i in (0..5).progbar() {
///     println!("Processing: {}", i);
/// }
/// ```
///
/// ## Bounded Progress Bar
///
/// ```rust
/// use zung_mini::progbar::ProgBarExt;
///
/// let progbar = (0..10).progbar().with_bounds('[', ']');
/// for i in progbar {
///     println!("Processing: {}", i);
/// }
/// ```
///
/// ## Custom Bar Style
///
/// ```rust
/// use zung_mini::progbar::ProgBarExt;
///
/// let progbar = (0..10).progbar().bar_style("=").with_bounds('<', '>');
/// for _ in progbar {
///     // Simulate some work
/// }
/// ```
///
/// # Returns
///
/// The `.progbar()` method returns a `ProgBar<Self, UnBounded>`, where `Self` is the
/// type of the iterator. By default, the progress bar is in an unbounded state but can
/// be converted to a bounded one with additional methods.
pub trait ProgBarExt: Sized {
    /// Transforms the iterator into a [`ProgBar`] with an unbounded state.
    ///
    /// This method wraps any iterator and creates a [`ProgBar`] instance, which
    /// can track the progress of iteration. The initial state of the progress bar is
    /// unbounded, meaning it doesn't have information about the total length of the iterator.
    ///
    /// This method is the entry point to creating a progress bar. By wrapping
    /// an iterator, it allows you to visually track progress during iteration with a
    /// terminal display. The appearance of the progress bar can be customized with methods
    /// like `bar_style()` and `with_bounds()`.
    ///
    /// For usage expamples and more information see [`ProgBarExt`] documentation.
    fn progbar(self) -> ProgBar<Self, UnBounded>;
}

impl<T> ProgBarExt for T
where
    T: Iterator,
{
    fn progbar(self) -> ProgBar<Self, UnBounded> {
        ProgBar::new(self)
    }
}

impl<T, Bound> Iterator for ProgBar<T, Bound>
where
    T: Iterator,
    Bound: ProgBarDisplay,
{
    type Item = T::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.iterator.next();

        self.bound.display(self);
        if next.is_none() {
            println!();
        }
        self.step += 1;
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progbar_creation() {
        let progbar = (0..10).progbar().with_bounds('[', ']');
        assert_eq!(progbar.bound.len, 10);
        assert_eq!(progbar.step, 0);
        assert_eq!(progbar.bound.percentage.get(), 0);
        assert_eq!(progbar.bar.to_string(), "#");
    }

    #[test]
    fn test_progbar_iteration() {
        let mut progbar = (0..5).progbar().with_bounds('[', ']');
        assert_eq!(progbar.next(), Some(0));
        assert_eq!(progbar.step, 1);
        assert_eq!(progbar.next(), Some(1));
        assert_eq!(progbar.step, 2);
        assert_eq!(progbar.next(), Some(2));
        assert_eq!(progbar.step, 3);
        assert_eq!(progbar.next(), Some(3));
        assert_eq!(progbar.step, 4);
        assert_eq!(progbar.next(), Some(4));
        assert_eq!(progbar.step, 5);
        assert_eq!(progbar.next(), None);
    }

    #[test]
    fn test_percentage_calculation() {
        let mut progbar = (0..10).progbar().with_bounds('[', ']');
        progbar.next();
        progbar.calculate_percentage();
        assert_eq!(progbar.bound.percentage.get(), 10);

        for _ in 0..4 {
            progbar.next();
        }
        progbar.calculate_percentage();
        assert_eq!(progbar.bound.percentage.get(), 50);

        for _ in 0..5 {
            progbar.next();
        }
        progbar.calculate_percentage();
        assert_eq!(progbar.bound.percentage.get(), 100);
    }

    #[test]
    fn test_custom_bar_style() {
        let progbar = (0..10).progbar().bar_style("=").with_bounds('[', ']');
        assert_eq!(progbar.bar.to_string(), "=");
        assert_eq!(progbar.bound.len, 10);
    }

    #[test]
    fn test_unbounded_progbar() {
        let mut progbar = (0..).progbar().bar_style("-");
        assert_eq!(progbar.bar.to_string(), "-");
        for _ in 0..5 {
            progbar.next();
        }
        assert_eq!(progbar.step, 5);
    }

    #[test]
    fn test_progress_display() {
        let mut progbar = (0..10).progbar().with_bounds('[', ']');

        for i in 0..=10 {
            progbar.next();
            assert_eq!(progbar.bound.percentage.get(), i * 10);
        }
    }
}
