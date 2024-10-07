use indicatif::{ProgressBar, ProgressStyle};

use crate::orst::Sorter;

/// An implementation of [Selection Sort](https://en.wikipedia.org/wiki/Selection_sort)
///
/// # Usage
///```
/// use zung_mini::orst::{SelectionSorter, Sorter};
///
/// let mut slice = [1, 5, 4, 2, 3];
/// SelectionSorter.sort(&mut slice);
/// assert_eq!(slice, [1, 2, 3, 4, 5]);
///```
/// # Explanation
///
/// Selection sort is an in-place comparison sorting
/// algorithm. It has an O(n2) time complexity, which
/// makes it inefficient on large  lists, and generally
/// performs worse than the similar insertion sort. Selection sort is noted for its
/// simplicity and has performance advantages over more complicated algorithms
/// in certain situations, particularly where auxiliary memory is
/// limited.
///
/// # Algorithm
///
/// The algorithm divides the input list into two parts:
/// a sorted sublist of items which is built
/// up from left to right at the front (
/// left) of the list and a sublist of
/// the remaining unsorted items that occupy the rest of
/// the list. Initially, the sorted sublist is
/// empty and the unsorted sublist is the entire input
/// list. The algorithm proceeds by finding the smallest
/// (or largest, depending on sorting order)
/// element in the unsorted sublist, exchanging (swapping
/// ) it with the leftmost unsorted element (putting
/// it in sorted order), and moving the sublist
/// boundaries one element to the right.
pub struct SelectionSorter;

impl<T> Sorter<T> for SelectionSorter
where
    T: Ord,
{
    fn sort(&self, slice: &mut [T]) {
        let pb = ProgressBar::new(slice.len() as u64);
        pb.set_style(
            ProgressStyle::with_template(
                "Selection Sort -> {spinner:.green} [{elapsed_precise}] [{bar:50.cyan/blue}] On Slice: ({pos}/{len}, ETA: {eta})",
            )
            .unwrap(),
        );
        for unsorted in 0..slice.len() {
            let mut smallest_in_rest = unsorted;
            for i in (unsorted + 1)..slice.len() {
                if slice[i] < slice[smallest_in_rest] {
                    smallest_in_rest = i;
                }
            }
            if unsorted != smallest_in_rest {
                slice.swap(unsorted, smallest_in_rest);
            }
            pb.inc(1);
        }
    }
}

#[test]
fn works() {
    let mut things = vec![4, 2, 3, 5, 1];
    SelectionSorter.sort(&mut things);
    assert_eq!(things, &[1, 2, 3, 4, 5])
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn arbitrary_array_smart() {
        let mut slice = [1, 5, 4, 2, 3];
        SelectionSorter.sort(&mut slice);
        assert_eq!(slice, [1, 2, 3, 4, 5]);
    }

    #[test]
    fn arbitrary_array_lame() {
        let mut slice = [1, 5, 4, 2, 3];
        SelectionSorter.sort(&mut slice);
        assert_eq!(slice, [1, 2, 3, 4, 5]);
    }

    #[test]
    fn sorted_array_smart() {
        let mut slice = (1..10).collect::<Vec<_>>();
        SelectionSorter.sort(&mut slice);
        assert_eq!(slice, (1..10).collect::<Vec<_>>());
    }

    #[test]
    fn sorted_array_lame() {
        let mut slice = (1..10).collect::<Vec<_>>();
        SelectionSorter.sort(&mut slice);
        assert_eq!(slice, (1..10).collect::<Vec<_>>());
    }

    #[test]
    fn very_unsorted_smart() {
        let mut slice = (1..1000).rev().collect::<Vec<_>>();
        SelectionSorter.sort(&mut slice);
        assert_eq!(slice, (1..1000).collect::<Vec<_>>());
    }

    #[test]
    fn very_unsorted_lame() {
        let mut slice = (1..1000).rev().collect::<Vec<_>>();
        SelectionSorter.sort(&mut slice);
        assert_eq!(slice, (1..1000).collect::<Vec<_>>());
    }

    #[test]
    fn simple_edge_cases_smart() {
        let mut one = vec![1];
        SelectionSorter.sort(&mut one);
        assert_eq!(one, vec![1]);

        let mut two = vec![1, 2];
        SelectionSorter.sort(&mut two);
        assert_eq!(two, vec![1, 2]);

        let mut two = vec![2, 1];
        SelectionSorter.sort(&mut two);
        assert_eq!(two, vec![1, 2]);

        let mut three = vec![3, 1, 2];
        SelectionSorter.sort(&mut three);
        assert_eq!(three, vec![1, 2, 3]);
    }

    #[test]
    fn simple_edge_cases_lame() {
        let mut one = vec![1];
        SelectionSorter.sort(&mut one);
        assert_eq!(one, vec![1]);

        let mut two = vec![1, 2];
        SelectionSorter.sort(&mut two);
        assert_eq!(two, vec![1, 2]);

        let mut two = vec![2, 1];
        SelectionSorter.sort(&mut two);
        assert_eq!(two, vec![1, 2]);

        let mut three = vec![3, 1, 2];
        SelectionSorter.sort(&mut three);
        assert_eq!(three, vec![1, 2, 3]);
    }
}
