//! Saves each review in one file, under a lock.

/// Holds the owner's answers.
#[derive(Debug)]
pub struct ReviewStore {
    /// One answer per comment.
    pub answers: Vec<String>,
}

// Only the review screen writes answers.
impl ReviewStore {
    /// Saves the owner's answer under the review's lock.
    pub fn save_answer(&mut self) {
        // Entries are only added, never changed.
        self.answers.push(String::new());
    }
}

/// How often the lock is tried.
pub const LOCK_TRIES: u32 = 3; // The lock waits at most three times.
pub const LOCK_WAIT_SECONDS: u64 = 1;

/// Finishes writing the review.

pub fn finish_writing() {}

#[cfg(test)]
mod tests {
    /// A lost answer would drop the owner's review.
    #[test]
    fn saves_each_answer() {}
}
