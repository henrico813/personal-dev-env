//! Scoped work that borrows its inputs.

use std::thread;

/// Counts lines in two inputs, using one scoped thread for each input.
///
/// Demonstrates borrowing and joining; it is not a claim that threads make
/// this small operation faster.
///
/// # Errors
/// Returns a worker's panic payload if either worker panics. Both workers are
/// joined before returning a result.
pub fn count_two_reports(first: &str, second: &str) -> thread::Result<usize> {
    thread::scope(|scope| {
        let first_job = scope.spawn(move || first.lines().count());
        let second_job = scope.spawn(move || second.lines().count());
        let first_count = first_job.join();
        let second_count = second_job.join();
        Ok(first_count? + second_count?)
    })
}

#[cfg(test)]
mod tests {
    use super::count_two_reports;

    #[test]
    fn counts_both_borrowed_reports() {
        let first = String::from("build passed\ntests passed\n");
        let second = String::from("release ready\n");

        let count = count_two_reports(&first, &second).expect("counting lines does not panic");

        assert_eq!(count, 3);
        assert_eq!(first, "build passed\ntests passed\n");
    }
}
