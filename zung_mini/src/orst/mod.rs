//! Implementation of sorting algorithms from [Crust of Rust: Sorting
//! Algorithms](https://www.youtube.com/watch?v=h4RkCyJyXmM)
//!
//! # Example
//!
//! ```
//! use zung_mini::orst::BubbleSorter;
//! use zung_mini::orst::Sorter;
//!
//! let mut slice = vec![1, 3, 2, 5, 4];
//! BubbleSorter.sort(&mut slice);
//! assert_eq!(vec![1, 2, 3, 4, 5], slice);
//! ```

pub mod benchmark;
mod sorters;

pub use sorters::bubble_sorter::BubbleSorter;
pub use sorters::insertion_sorter::InsertionSorter;
pub use sorters::quick_sorter::QuickSorter;
pub use sorters::selection_sorter::SelectionSorter;

/// The sorting algorithm must implement the trait `Sorter`.
pub trait Sorter<T>
where
    T: Ord,
{
    fn sort(&self, slice: &mut [T]);
}

pub trait Sort<T, S>
where
    S: Sorter<T>,
    T: Ord,
{
    fn orst(&mut self);
}
