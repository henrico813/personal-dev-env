//! Copying bytes from a buffer supplied by foreign code.

/// Copies a foreign buffer into a Rust-owned vector.
///
/// An empty buffer may use a null pointer. This does not free the foreign buffer.
///
/// # Safety
/// For a nonzero `len`, `data` must be non-null and point to `len` initialized
/// bytes within one live allocation. The range must be valid for reads, fit
/// within `isize::MAX` bytes, and not wrap the address space. No one may mutate
/// or free the bytes while this function reads them. The caller remains
/// responsible for any cleanup required by the foreign allocator.
pub unsafe fn copy_foreign_bytes(data: *const u8, len: usize) -> Vec<u8> {
    if len == 0 {
        return Vec::new();
    }

    // SAFETY: The caller guarantees a readable, initialized, single-allocation
    // range for this call, with no concurrent mutation. We copy it before returning
    // and do not let a reference to the foreign allocation escape.
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    bytes.to_vec()
}

#[cfg(test)]
mod tests {
    use super::copy_foreign_bytes;

    #[test]
    fn copies_foreign_bytes() {
        let input = [1_u8, 2, 3];

        // SAFETY: The live array provides all three initialized bytes and remains
        // unchanged for the duration of the copy.
        let copied = unsafe { copy_foreign_bytes(input.as_ptr(), input.len()) };

        assert_eq!(copied, input);
    }

    #[test]
    fn accepts_empty_null_buffer() {
        // SAFETY: This function explicitly permits null for an empty input.
        let copied = unsafe { copy_foreign_bytes(std::ptr::null(), 0) };

        assert!(copied.is_empty());
    }
}
