use colored::Colorize;
use rand::{self, Rng};
use std::{cell::Cell, rc::Rc, time::Instant};

use prettytable::{row, Table};

use super::{BubbleSorter, InsertionSorter, QuickSorter, SelectionSorter, Sorter};

const ZERO: usize = 0;
const ONE: usize = 1;
const HUNDRED: usize = 100;
const TEN_THOUSAND: usize = 10_000;
const HUNDRED_THOUSAND: usize = 100_000;
const MILLION: usize = 1_000_000;
const HUNDRED_MILLION: usize = 100_000_000;

// In this the `elem` will be compared and the `comparison_counter` will be ignored.
#[derive(Clone)]
struct SortEvaluator<T> {
    // For making the comparisons
    elem: T,
    // This counter will update every time the `elem` is compared.
    // Therefore, obviously this has to be a mutable value.
    // Therefore, it is rapped in reference counter and a cell
    comparison_counter: Rc<Cell<usize>>,
}

impl<T> SortEvaluator<T> {
    fn new(elem: T, comparison_counter: Rc<Cell<usize>>) -> Self {
        Self {
            elem,
            comparison_counter,
        }
    }
}

// Trait for equality comparisons which are equivalence relations.
//
// This means, that in addition to a == b and a != b being strict inverses, the equality
// must be (for all a, b and c):
//
//     reflexive: a == a;
//     symmetric: a == b implies b == a; and
//     transitive: a == b and b == c implies a == c.
//
// This property cannot be checked by the compiler, and therefore Eq implies PartialEq,
// and has no extra methods.
impl<T: Eq> Eq for SortEvaluator<T> {}

// This trait allows for partial equality,
// for types that do not have a full equivalence relation.
// For example, in floating point numbers NaN != NaN,
// so floating point types implement PartialEq but not Eq.
// Formally speaking, when Rhs == Self,
// this trait corresponds to a partial equivalence relation.
impl<T: PartialEq> PartialEq for SortEvaluator<T> {
    fn eq(&self, other: &Self) -> bool {
        self.comparison_counter
            .set(self.comparison_counter.get() + 1);
        self.elem == other.elem
    }
}

// Trait for types that form a partial order.
//
// The lt, le, gt, and ge methods of this trait can be called using the <, <=, >, and >= operators, respectively.
//
// The methods of this trait must be consistent with each other and with those of PartialEq. The following conditions must hold:
//
//    1. a == b if and only if partial_cmp(a, b) == Some(Equal).
//    2. a < b if and only if partial_cmp(a, b) == Some(Less)
//    3. a > b if and only if partial_cmp(a, b) == Some(Greater)
//    4. a <= b if and only if a < b || a == b
//    5. a >= b if and only if a > b || a == b
//    6. a != b if and only if !(a == b).
//
impl<T: PartialOrd> PartialOrd for SortEvaluator<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.comparison_counter
            .set(self.comparison_counter.get() + 1);
        self.elem.partial_cmp(&other.elem)
    }
}

// Trait for types that form a total order.
//
// Implementations must be consistent with the PartialOrd implementation,
// and ensure max, min, and clamp are consistent with cmp:
//
//     partial_cmp(a, b) == Some(cmp(a, b)).
//     max(a, b) == max_by(a, b, cmp) (ensured by the default implementation).
//     min(a, b) == min_by(a, b, cmp) (ensured by the default implementation).
//     For a.clamp(min, max), see the method docs (ensured by the default implementation).
//
// It’s easy to accidentally make cmp and partial_cmp disagree by deriving
// some of the traits and manually implementing others.
impl<T: Ord> Ord for SortEvaluator<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.comparison_counter
            .set(self.comparison_counter.get() + 1);
        self.elem.cmp(&other.elem)
    }
}

fn run_bench<T, S>(
    sorter: S,
    values: &mut [SortEvaluator<T>],
    comparisons: Rc<Cell<usize>>,
) -> usize
where
    T: Ord + Eq + Clone,
    S: Sorter<SortEvaluator<T>>,
{
    comparisons.set(0);
    sorter.sort(values);

    comparisons.get()
}

pub fn run_orst() {
    let mut random = rand::thread_rng();
    let counter = Rc::new(Cell::new(0));
    for &n in &[
        ZERO,
        ONE,
        HUNDRED,
        TEN_THOUSAND,
        HUNDRED_THOUSAND,
        MILLION,
        HUNDRED_MILLION,
    ] {
        let mut values = Vec::with_capacity(n);
        for _ in 0..n {
            values.push(SortEvaluator::new(random.gen::<i32>(), counter.clone()));
        }

        println!(
            "{} {}",
            "List Size -> ".bold().underline().blue(),
            n.to_string().bold()
        );

        let mut table = Table::new();
        table.add_row(row![
            "Sorter".bold(),
            "Comparisons Made".bold(),
            "Time Taken".bold()
        ]);

        if n <= HUNDRED_THOUSAND {
            let now = Instant::now();
            let took = run_bench(BubbleSorter, &mut values, counter.clone());
            table.add_row(row![
                "Bubble Sort",
                took.to_string(),
                format!("{:?}", now.elapsed())
            ]);
        } else {
            table.add_row(row!["Bubble Sort", "Not Doing It".red(), "It is Stupid"]);
        }

        if n <= HUNDRED_THOUSAND {
            let now = Instant::now();
            let took = run_bench(
                InsertionSorter { smart: true },
                &mut values,
                counter.clone(),
            );

            table.add_row(row![
                "Insertion Sort",
                took.to_string(),
                format!("{:?}", now.elapsed())
            ]);

            let now = Instant::now();
            let took = run_bench(
                InsertionSorter { smart: false },
                &mut values,
                counter.clone(),
            );

            table.add_row(row![
                "Insertion Sort (not smart)",
                took.to_string(),
                format!("{:?}", now.elapsed())
            ]);
        } else {
            table.add_row(row!["Insertion Sort", "Not Doing It".red(), "It is Stupid"]);
        }

        if n <= HUNDRED_THOUSAND {
            let now = Instant::now();
            let took = run_bench(SelectionSorter, &mut values, counter.clone());
            table.add_row(row![
                "Selection Sort",
                took.to_string(),
                format!("{:?}", now.elapsed())
            ]);
        } else {
            table.add_row(row!["Selection Sort", "Not Doing It".red(), "It is Stupid"]);
        }

        let now = Instant::now();
        let took = run_bench(QuickSorter, &mut values, counter.clone());

        table.add_row(row![
            "Quick Sort",
            took.to_string(),
            format!("{:?}", now.elapsed())
        ]);

        // TODO: Implement this.
        //
        // let now = Instant::now();
        // let took = run_bench(StdSorter { stable: true }, &mut values, counter.clone());
        // table.add_row(row![
        //     "Standard Library Sort Stable",
        //     took.to_string(),
        //     format!("{:?}", now.elapsed())
        // ]);
        //
        // let now = Instant::now();
        // let took = run_bench(StdSorter { stable: false }, &mut values, counter.clone());
        // table.add_row(row![
        //     "Standart Library Sort Unstable",
        //     took.to_string(),
        //     format!("{:?}", now.elapsed())
        // ]);

        table.printstd();
        println!();
    }
}
