use indicatif::{ProgressBar, ProgressStyle};

use crate::orst::Sorter;

/// An implementation of [Insertion Sort](https://en.wikipedia.org/wiki/Insertion_sort)
///
/// # Explanation
///
/// Insertion sort is a simple sorting algorithm that builds the final sorted array (or list) one
/// item at a time
///
/// Insertion sort iterates, consuming one input element each repetition, and grows a sorted output
/// list. At each iteration, insertion sort removes one element from the input data, finds the
/// location it belongs within the sorted list, and inserts it there. It repeats until no input
/// elements remain.
///
/// Sorting is typically done in-place, by iterating up the array, growing the sorted list behind
/// it. At each array-position, it checks the value there against the largest value in the sorted
/// list (which happens to be next to it, in the previous array- position checked). If larger, it
/// leaves the element in place and moves to the next. If smaller, it finds the correct position
/// within the sorted list, shifts all the larger values up to make a space, and inserts into that
/// correct position.
///
/// # Usage
///```
/// use zung_mini::orst::{InsertionSorter, Sorter};
///
/// let mut slice = [1, 5, 4, 2, 3];
/// InsertionSorter{ smart: true }.sort(&mut slice);
/// assert_eq!(slice, [1, 2, 3, 4, 5]);
///```
pub struct InsertionSorter {
    pub smart: bool,
}

impl<T> Sorter<T> for InsertionSorter
where
    T: Ord,
{
    #[inline]
    fn sort(&self, slice: &mut [T]) {
        let pb = ProgressBar::new(slice.len() as u64);
        pb.set_style(
            ProgressStyle::with_template(
                "Insertion Sort -> {spinner:.green} [{elapsed_precise}] [{bar:50.cyan/blue}] On Slice: ({pos}/{len}, ETA: {eta})",
            )
            .unwrap(),
        );

        for unsorted in 1..slice.len() {
            if !self.smart {
                let mut i = unsorted;
                while i > 0 && slice[i - 1] > slice[i] {
                    slice.swap(i - 1, i);
                    i -= 1;
                }
            } else {
                let i = match slice[..unsorted].binary_search(&slice[unsorted]) {
                    Ok(i) => i,
                    Err(i) => i,
                };
                slice[i..=unsorted].rotate_right(1);
            }
            pb.inc(1);
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn arbitrary_array_smart() {
        let mut slice = [1, 5, 4, 2, 3];
        InsertionSorter { smart: true }.sort(&mut slice);
        assert_eq!(slice, [1, 2, 3, 4, 5]);
    }

    #[test]
    fn arbitrary_array_lame() {
        let mut slice = [1, 5, 4, 2, 3];
        InsertionSorter { smart: false }.sort(&mut slice);
        assert_eq!(slice, [1, 2, 3, 4, 5]);
    }

    #[test]
    fn sorted_array_smart() {
        let mut slice = (1..10).collect::<Vec<_>>();
        InsertionSorter { smart: true }.sort(&mut slice);
        assert_eq!(slice, (1..10).collect::<Vec<_>>());
    }

    #[test]
    fn sorted_array_lame() {
        let mut slice = (1..10).collect::<Vec<_>>();
        InsertionSorter { smart: false }.sort(&mut slice);
        assert_eq!(slice, (1..10).collect::<Vec<_>>());
    }

    #[test]
    fn very_unsorted_smart() {
        let mut slice = (1..1000).rev().collect::<Vec<_>>();
        InsertionSorter { smart: true }.sort(&mut slice);
        assert_eq!(slice, (1..1000).collect::<Vec<_>>());
    }

    #[test]
    fn very_unsorted_lame() {
        let mut slice = (1..1000).rev().collect::<Vec<_>>();
        InsertionSorter { smart: false }.sort(&mut slice);
        assert_eq!(slice, (1..1000).collect::<Vec<_>>());
    }

    #[test]
    fn simple_edge_cases_smart() {
        let mut one = vec![1];
        InsertionSorter { smart: true }.sort(&mut one);
        assert_eq!(one, vec![1]);

        let mut two = vec![1, 2];
        InsertionSorter { smart: true }.sort(&mut two);
        assert_eq!(two, vec![1, 2]);

        let mut two = vec![2, 1];
        InsertionSorter { smart: true }.sort(&mut two);
        assert_eq!(two, vec![1, 2]);

        let mut three = vec![3, 1, 2];
        InsertionSorter { smart: true }.sort(&mut three);
        assert_eq!(three, vec![1, 2, 3]);
    }

    #[test]
    fn simple_edge_cases_lame() {
        let mut one = vec![1];
        InsertionSorter { smart: false }.sort(&mut one);
        assert_eq!(one, vec![1]);

        let mut two = vec![1, 2];
        InsertionSorter { smart: false }.sort(&mut two);
        assert_eq!(two, vec![1, 2]);

        let mut two = vec![2, 1];
        InsertionSorter { smart: false }.sort(&mut two);
        assert_eq!(two, vec![1, 2]);

        let mut three = vec![3, 1, 2];
        InsertionSorter { smart: false }.sort(&mut three);
        assert_eq!(three, vec![1, 2, 3]);
    }
}
