use crate::orst::Sorter;
use indicatif::{ProgressBar, ProgressStyle};

/// An implementation of [Bubble Sort](https://en.wikipedia.org/wiki/Bubble_sort)
///
/// # Usage
///```
/// use zung_mini::orst::{BubbleSorter, Sorter};
///
/// let mut slice = [1, 5, 4, 2, 3];
/// BubbleSorter.sort(&mut slice);
/// assert_eq!(slice, [1, 2, 3, 4, 5]);
///```
/// # Explanation
///
/// Bubble sort, sometimes referred to as sinking sort,
/// is a simple sorting algorithm that repeatedly steps
/// through the list, compares adjacent elements and swaps
/// them if they are in the wrong order. The pass through
/// the list is repeated until the list is sorted. The
/// algorithm, which is a comparison sort, is named for the
/// way smaller or larger elements "bubble" to the top of the list.
///
///
/// # Algorithm
///
/// ```
/// let mut slice = vec![1, 3, 2, 5, 4];
///
/// let mut swapped = true;
///
///     while swapped {
///         swapped = false;
///         for i in 0..(slice.len() - 1) {
///         // swap the elements at index if the current element is
///         // bigger that the next element.
///             if slice[i] > slice[i + 1] {
///                 slice.swap(i, i + 1);
///                 swapped = true;
///             }
///         }
///     }
/// ```
#[derive(Default)]
pub struct BubbleSorter;

impl<T> Sorter<T> for BubbleSorter
where
    T: Ord,
{
    #[inline]
    fn sort(&self, slice: &mut [T]) {
        let pb = ProgressBar::new(slice.len() as u64);
        pb.set_style(
            ProgressStyle::with_template(
                "Bubble Sort -> {spinner:.green} [{elapsed_precise}] [{bar:50.cyan/blue}] On Slice: ({pos}/{len}, ETA: {eta})",
            )
            .unwrap(),
        );

        let mut swapped = true;

        while swapped {
            swapped = false;
            for i in 1..slice.len() {
                if slice[i - 1] > slice[i] {
                    slice.swap(i - 1, i);
                    swapped = true;
                }
            }
            pb.inc(1);
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn arbitrary_array() {
        let mut slice = [1, 5, 4, 2, 3];
        BubbleSorter.sort(&mut slice);
        assert_eq!(slice, [1, 2, 3, 4, 5]);
    }

    #[test]
    fn sorted_array() {
        let mut slice = (1..10).collect::<Vec<_>>();
        BubbleSorter.sort(&mut slice);
        assert_eq!(slice, (1..10).collect::<Vec<_>>());
    }

    #[test]
    fn very_unsorted() {
        let mut slice = (1..1000).rev().collect::<Vec<_>>();
        BubbleSorter.sort(&mut slice);
        assert_eq!(slice, (1..1000).collect::<Vec<_>>());
    }

    #[test]
    fn simple_edge_cases() {
        let mut one = vec![1];
        BubbleSorter.sort(&mut one);
        assert_eq!(one, vec![1]);

        let mut two = vec![1, 2];
        BubbleSorter.sort(&mut two);
        assert_eq!(two, vec![1, 2]);

        let mut two = vec![2, 1];
        BubbleSorter.sort(&mut two);
        assert_eq!(two, vec![1, 2]);

        let mut three = vec![3, 1, 2];
        BubbleSorter.sort(&mut three);
        assert_eq!(three, vec![1, 2, 3]);
    }
}
