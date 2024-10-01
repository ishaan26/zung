//! A custom string splitting utility targeting lifetimes in rust following  [Crust of Rust:
//! Lifetime Annotations](https://www.youtube.com/watch?v=rAl-9HwD858)
//!
//! ## Overview
//!
//! This module provides the [`StrsplitExt`] trait, which adds a
//! [`strsplit()`](StrsplitExt::strsplit()) method to both [`String`] and [`&str`]. This method
//! returns an iterator ([`Strsplit`]) that yields the portions of the original string that appear
//! between occurrences of the specified delimiter. The splitting is done in a lazy fashion,
//! meaning that substrings are produced one at a time as needed, without pre-allocating memory for
//! all results.
//!
//! The `Strsplit` iterator can also be collected into a `Vec<&str>` for cases where the entire
//! result is needed upfront.
//!
//! ### Features
//! - Supports splitting both owned `String` and borrowed `&str`.
//! - Returns an iterator that can be used lazily, or fully collected.
//!
//! ### Example
//!
//! ```rust
//! use zung_mini::strsplit::StrsplitExt;
//!
//! let haystack = "a,b,c,d,e";
//! let split = haystack.strsplit(",").into_vec();
//! assert_eq!(split, vec!["a", "b", "c", "d", "e"]);
//! ```

/// A trait to extend string types with the `strsplit` method.
/// and returns a `Strsplit` iterator over the resulting substrings.
/// This method allows for splitting a string by a specified delimiter (needle)
///
/// # Example
///
/// ```
/// use zung_mini::strsplit::StrsplitExt;
///
/// let haystack = "this is an example";
/// let split: Vec<&str> = haystack.strsplit(" ").collect();
/// assert_eq!(split, vec!["this", "is", "an", "example"]);
/// ```
pub trait StrsplitExt<'a, 'b>
where
    'b: 'a,
{
    /// Splits the string by cosuming it with the given `needle`, returning a `Strsplit` iterator.
    ///
    /// # Arguments
    ///
    /// * `needle` - The substring used as the delimiter for splitting.
    ///
    /// # Panics
    ///
    /// Panics if `needle` is empty.
    ///
    /// # Example
    ///
    /// ```
    /// use zung_mini::strsplit::StrsplitExt;
    ///
    /// let haystack = "this is an example";
    /// let split: Vec<&str> = haystack.strsplit(" ").collect();
    /// assert_eq!(split, vec!["this", "is", "an", "example"]);
    /// ```
    fn strsplit(&'a self, needle: &'b str) -> Strsplit;
}

impl<'a, 'b> StrsplitExt<'a, 'b> for String
where
    'b: 'a,
{
    fn strsplit(&'a self, needle: &'b str) -> Strsplit<'a> {
        Strsplit::new(self, needle)
    }
}

impl<'a, 'b> StrsplitExt<'a, 'b> for &str
where
    'b: 'a,
{
    fn strsplit(&'a self, needle: &'b str) -> Strsplit<'a> {
        Strsplit::new(self, needle)
    }
}

/// An iterator over substrings separated by a specified delimiter (`needle`).
/// The iterator yields the portions of the original string that appear between
/// occurrences of the delimiter.
///
/// This type is constructed by the [`strsplit()`](StrsplitExt::strsplit()) method.
pub struct Strsplit<'a> {
    remainder: Option<&'a str>,
    needle: &'a str,
}

impl<'a> Strsplit<'a> {
    fn new(haystack: &'a str, needle: &'a str) -> Self {
        assert!(!needle.is_empty(), "Empty needle is not allowed");
        Self {
            remainder: Some(haystack),
            needle,
        }
    }

    /// Consumes the [`Strsplit`] and constructs and returns a vector.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use zung_mini::strsplit::StrsplitExt;
    ///
    /// let haystack = "this is an example";
    /// let split = haystack.strsplit(" ").into_vec();
    ///
    /// // Will print in some order
    /// for s in split {
    ///     println!("{s}");
    /// }
    /// ```
    pub fn into_vec(self) -> Vec<&'a str> {
        self.collect()
    }
}

impl<'a> Iterator for Strsplit<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let remainder = self.remainder.as_mut()?;

        if let Some((start, end)) = find_needle(self.needle, remainder) {
            let before_needle = &remainder[..start];
            *remainder = &remainder[end..];
            Some(before_needle)
        } else {
            self.remainder.take()
        }
    }
}

fn find_needle(needle: &str, haystack: &str) -> Option<(usize, usize)> {
    haystack
        .find(needle)
        .map(|index| (index, index + needle.len()))
}

mod tests {
    #[cfg(test)]
    use super::*;

    #[test]
    fn strsplit_works() {
        let a = "a b c d e f";
        assert_eq!(
            a.strsplit(" ").into_vec(),
            vec!["a", "b", "c", "d", "e", "f"]
        );
    }

    #[test]
    #[should_panic(expected = "Empty needle is not allowed")]
    fn empty_needle() {
        let a = "a b c d e f";
        assert_eq!(
            a.strsplit("").into_vec(),
            vec!["a", "b", "c", "d", "e", "f"]
        );
    }

    #[test]
    fn strsplit_trailing_space_works() {
        let a = "a b c d e ";
        assert_eq!(
            a.strsplit(" ").into_vec(),
            vec!["a", "b", "c", "d", "e", ""]
        );
    }

    #[test]
    fn strsplit_with_comma_works() {
        let a = "a b c, d e f";
        assert_eq!(a.strsplit(",").into_vec(), vec!["a b c", " d e f"]);
    }
}
