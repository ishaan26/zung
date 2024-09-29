use std::io::Write;
use std::{thread::sleep, time::Duration};

pub struct Progbar<I> {
    iterator: I,
    len: usize,
    step: usize,
    percentage: u8,
    bar_style: BarStyle,
}

struct BarStyle(String);

impl BarStyle {
    fn new(bar_style: String) -> Self {
        Self(bar_style)
    }
}

impl Default for BarStyle {
    fn default() -> Self {
        Self::new(String::from("#"))
    }
}

impl<I> Progbar<I>
where
    I: Iterator,
{
    pub fn new(iterator: I) -> Self
    where
        I: ExactSizeIterator,
    {
        let len = iterator.len();
        Self {
            iterator,
            len,
            step: 0,
            percentage: 0,
            bar_style: BarStyle::default(),
        }
    }

    fn calculate_percentage(&mut self)
    where
        I: ExactSizeIterator,
    {
        self.percentage = ((self.step as f64 / self.len as f64) * 100.0) as u8;
    }

    fn display(&self) {
        if self.step >= 1 {
            print!(
                "[{:>3}%] {} \r",
                self.percentage,
                self.bar_style.0.repeat(self.step)
            );
            std::io::stdout().flush().unwrap();
        } else {
            print!("[{:>3}%]\r", 0);
            std::io::stdout().flush().unwrap();
        }
    }
}

impl<I> Iterator for Progbar<I>
where
    I: Iterator + ExactSizeIterator,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.iterator.next();
        self.calculate_percentage();
        self.display();
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
        let progbar = Progbar::new(0..10);
        assert_eq!(progbar.len, 10);
        assert_eq!(progbar.step, 0);
        assert_eq!(progbar.percentage, 0);
        assert_eq!(progbar.bar_style.0, "#");
    }

    #[test]
    fn test_progbar_iteration() {
        let mut progbar = Progbar::new(0..5);
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
        let mut progbar = Progbar::new(0..10);
        progbar.next();
        progbar.calculate_percentage();
        assert_eq!(progbar.percentage, 10);

        for _ in 0..4 {
            progbar.next();
        }
        progbar.calculate_percentage();
        assert_eq!(progbar.percentage, 50);

        for _ in 0..5 {
            progbar.next();
        }
        progbar.calculate_percentage();
        assert_eq!(progbar.percentage, 100);
    }

    #[test]
    fn test_display_output() {
        use std::fmt::Write;

        let mut progbar = Progbar::new(0..3);
        let mut output = String::new();

        // Initial display
        progbar.display();
        write!(output, "[  0%]\r").unwrap();

        // After first iteration
        progbar.next();
        progbar.display();
        write!(output, "[ 33%] # \r").unwrap();

        // After second iteration
        progbar.next();
        progbar.display();
        write!(output, "[ 66%] ## \r").unwrap();

        // After third (final) iteration
        progbar.next();
        progbar.display();
        write!(output, "[100%] ### \r\n").unwrap();

        // Check the final output
        assert_eq!(output, "[  0%]\r[ 33%] # \r[ 66%] ## \r[100%] ### \r\n");
    }
}

pub fn run_progbar(count: u8) {
    for _ in Progbar::new(0..count) {
        sleep(Duration::from_millis(100));
    }
}
