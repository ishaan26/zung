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
    fn strsplit<P>(&'a self, needle: P) -> Strsplit<'a, P>
    where
        P: 'b + AsRef<str>;

    /// Returns the substring before the first occurrence of the given `needle`
    /// without scanning the entire string.
    ///
    /// This function splits the string using the provided `needle` and immediately
    /// returns the portion of the string before the first occurrence of the `needle`.
    /// It stops searching once the `needle` is found, making it efficient as it
    /// avoids iterating over the entire string unnecessarily. If the `needle` is not found,
    /// the function returns the entire string.
    ///
    /// # Example
    ///
    /// ```rust
    /// use zung_mini::strsplit::StrsplitExt;
    /// let text = "hello world";
    ///
    /// let result = text.till_needle(" ");
    /// assert_eq!(result, "hello");
    /// ```
    fn till_needle<P>(&'a self, needle: P) -> &str
    where
        P: 'b + AsRef<str> + Clone,
    {
        let mut splitter = self.strsplit(needle.clone());
        splitter.next().unwrap()
    }
}

impl<'a, 'b> StrsplitExt<'a, 'b> for String
where
    'b: 'a,
{
    fn strsplit<P>(&'a self, needle: P) -> Strsplit<'a, P>
    where
        P: 'b + AsRef<str>,
    {
        Strsplit::new(self, needle)
    }
}

impl<'a, 'b> StrsplitExt<'a, 'b> for &str
where
    'b: 'a,
{
    fn strsplit<P>(&'a self, needle: P) -> Strsplit<'a, P>
    where
        P: 'b + AsRef<str>,
    {
        Strsplit::new(self, needle)
    }
}

/// An iterator over substrings separated by a specified delimiter (`needle`).
/// The iterator yields the portions of the original string that appear between
/// occurrences of the delimiter.
///
/// This type is constructed by the [`strsplit()`](StrsplitExt::strsplit()) method.
#[derive(Debug, Clone, Copy)]
pub struct Strsplit<'a, N> {
    remainder: Option<&'a str>,
    needle: N,
}

impl<'a, N> Strsplit<'a, N>
where
    N: 'a + AsRef<str>,
{
    fn new(haystack: &'a str, needle: N) -> Self {
        assert!(!needle.as_ref().is_empty(), "Empty needle is not allowed");
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

impl<'a, N> Iterator for Strsplit<'a, N>
where
    N: 'a + AsRef<str>,
{
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let remainder = self.remainder.as_mut()?;

        if let Some((start, end)) = find_needle(self.needle.as_ref(), remainder) {
            let before_needle = &remainder[..start];
            *remainder = &remainder[end..];
            Some(before_needle)
        } else {
            self.remainder.take()
        }
    }
}

impl<'a, N> From<Strsplit<'a, N>> for Vec<&'a str>
where
    N: 'a + AsRef<str>,
{
    fn from(value: Strsplit<'a, N>) -> Self {
        value.into_vec()
    }
}

fn find_needle(needle: &str, haystack: &str) -> Option<(usize, usize)> {
    haystack
        .find(needle)
        .map(|index| (index, index + needle.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let a = "a b c d e f".strsplit(" ");
        let b = Strsplit {
            remainder: Some("a b c d e f"),
            needle: " ",
        };
        assert_eq!(a.remainder, b.remainder);
        assert_eq!(a.needle, b.needle);
    }

    #[test]
    fn strsplit_works() {
        let a = "a b c d e f";
        assert_eq!(
            a.strsplit(" ").into_vec(),
            vec!["a", "b", "c", "d", "e", "f"]
        );
    }

    #[test]
    fn strsplit_works_with_string() {
        let a = "a b c d e f";
        assert_eq!(
            a.strsplit(String::from(" ")).into_vec(),
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

    #[test]
    fn till_needle_finds_substring() {
        let text = "hello world";
        let result = text.till_needle(" ");
        assert_eq!(result, "hello");
    }

    #[test]
    fn till_needle_returns_entire_string_if_needle_not_found() {
        let text = "hello";
        let result = text.till_needle(",");
        assert_eq!(result, "hello");
    }

    #[test]
    fn till_needle_with_multiple_occurrences() {
        let text = "apple,banana,orange";
        let result = text.till_needle(",");
        assert_eq!(result, "apple"); // Stops at first occurrence
    }

    #[test]
    fn till_needle_returns_none_for_empty_string() {
        let text = "";
        let result = text.till_needle(",");
        assert_eq!(result, "");
    }

    #[test]
    #[should_panic(expected = "Empty needle is not allowed")]
    fn till_needle_empty_needle_panics() {
        let text = "example";
        let _ = text.till_needle(""); // Should panic due to empty needle
    }

    #[test]
    fn till_needle_handles_special_characters() {
        let text = "foo@bar.com";
        let result = text.till_needle("@");
        assert_eq!(result, "foo");
    }

    #[test]
    fn till_needle_works_with_longer_needle() {
        let text = "this is a test string";
        let result = text.till_needle("is");
        assert_eq!(result, "th");
    }

    #[test]
    fn test_from_and_into() {
        let strsplit = "a b c d e f".strsplit(" ");
        let vec1 = Vec::from(strsplit);
        let vec2: Vec<&str> = strsplit.into();

        assert_eq!(vec1, vec2)
    }
}
