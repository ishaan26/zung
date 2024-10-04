use indicatif::{ProgressBar, ProgressStyle};
use rand::Rng;

use crate::orst::Sorter;

/// An implementation of [Quick Sort](https://en.wikipedia.org/wiki/Quicksort)
///
/// # Usage
///```
/// use zung_mini::orst::{QuickSorter, Sorter};
///
/// let mut slice = [1, 5, 4, 2, 3];
/// QuickSorter.sort(&mut slice);
/// assert_eq!(slice, [1, 2, 3, 4, 5]);
///```
///
/// # Explanation
///
/// Quicksort is an in-place sorting algorithm. Developed
/// by British computer scientist Tony Hoare in 1959 and published
/// in 1961 it is still a commonly used algorithm for
/// sorting. When implemented well, it can be somewhat
/// faster than merge sort and about two or three times
/// faster than heapsort.
///
/// # Algorithm
///
/// Quicksort is a divide-and-conquer algorithm.
/// It works by selecting a 'pivot' element from
/// the array and partitioning the other elements into two sub
/// -arrays, according to whether they are less than
/// or greater than the pivot. For this reason,
/// it is sometimes called partition-exchange sort.
/// The sub-arrays are then sorted recursively.
/// This can be done in-place, requiring small
/// additional amounts of memory to perform the sorting.
pub struct QuickSorter;

fn quicksort<T: Ord>(slice: &mut [T]) {
    const INSERTION_THRESHOLD: usize = 10;

    let pb = ProgressBar::new(slice.len() as u64);
    pb.set_style(
            ProgressStyle::with_template(
                "Quick Sort -> {spinner:.green} [{elapsed_precise}] {bar:50.cyan/blue} On Slice: {pos}/{len}, ETA: {eta}",
            )
            .unwrap(),
        );

    // Define a closure to encapsulate the counter
    let quicksort_with_pb = |slice: &mut [T]| {
        fn inner_quicksort<T: Ord>(slice: &mut [T], pb: &ProgressBar) {
            if slice.len() <= INSERTION_THRESHOLD {
                slice.sort();
                return;
            }

            let pivot_index = rand::thread_rng().gen_range(0..slice.len());
            slice.swap(0, pivot_index);

            let (pivot, rest) = slice.split_first_mut().expect("Unexpected empty slice");
            let mut left = 0;
            let mut right = rest.len() - 1;

            while left <= right {
                if &rest[left] <= pivot {
                    left += 1;
                } else if &rest[right] > pivot {
                    if right == 0 {
                        break;
                    }
                    right -= 1;
                } else {
                    rest.swap(left, right);
                    left += 1;
                    if right == 0 {
                        break;
                    }
                    right -= 1;
                }
            }

            let left = left + 1;
            slice.swap(0, left - 1);

            let (left_part, right_part) = slice.split_at_mut(left - 1);
            assert!(left_part.last() <= right_part.first());

            pb.inc(1);
            inner_quicksort(left_part, pb);
            inner_quicksort(&mut right_part[1..], pb);
        }

        // Call the inner recursive function
        inner_quicksort(slice, &pb);
    };

    quicksort_with_pb(slice);
}

impl<T> Sorter<T> for QuickSorter
where
    T: Ord,
{
    #[inline]
    fn sort(&self, slice: &mut [T]) {
        quicksort(slice)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn arbitrary_array() {
        let mut slice = [1, 5, 4, 2, 3];
        QuickSorter.sort(&mut slice);
        assert_eq!(slice, [1, 2, 3, 4, 5]);
    }

    #[test]
    fn sorted_array() {
        let mut slice = (1..10).collect::<Vec<_>>();
        QuickSorter.sort(&mut slice);
        assert_eq!(slice, (1..10).collect::<Vec<_>>());
    }

    #[test]
    fn very_unsorted() {
        let mut slice = (1..1000).rev().collect::<Vec<_>>();
        QuickSorter.sort(&mut slice);
        assert_eq!(slice, (1..1000).collect::<Vec<_>>());
    }

    #[test]
    fn simple_edge_cases() {
        let mut one = vec![1];
        QuickSorter.sort(&mut one);
        assert_eq!(one, vec![1]);

        let mut two = vec![1, 2];
        QuickSorter.sort(&mut two);
        assert_eq!(two, vec![1, 2]);

        let mut two = vec![2, 1];
        QuickSorter.sort(&mut two);
        assert_eq!(two, vec![1, 2]);

        let mut three = vec![3, 1, 2];
        QuickSorter.sort(&mut three);
        assert_eq!(three, vec![1, 2, 3]);
    }
}
