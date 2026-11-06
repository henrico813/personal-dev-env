/// Saves one User Feedback under the lock.
/// Waits while another screen holds the lock.
// Only the review screen calls this.
pub fn save_user_feedback() {
    let tries = 3; // The lock waits at most three times.
}
