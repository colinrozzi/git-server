/// Safe logging function that works in both runtime and test environments
pub fn safe_log(message: &str) {
    // Only log in non-test environments
    if !cfg!(test) {
        crate::bindings::theater::simple::runtime::log(message);
    }
}
