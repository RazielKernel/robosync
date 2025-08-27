//! Smart buffer sizing for 10GbE saturation
//! Simplified from buffer_sizing.rs

use parking_lot::Mutex;

pub struct BufferSizer {
    max_buffer_size: usize,
    min_buffer_size: usize,
    cached_available_memory: Mutex<Option<u64>>,
}

impl BufferSizer {
    pub fn new() -> Self {
        // Optimized for 10GbE: large buffers to minimize syscalls
        BufferSizer {
            max_buffer_size: 16 * 1024 * 1024, // 16MB max
            min_buffer_size: 1024 * 1024,      // 1MB min
            cached_available_memory: Mutex::new(None),
        }
    }

    /// Get available memory (conservative fallback to avoid extra dependencies)
    fn get_available_memory() -> u64 {
        // Use a fixed, conservative value to bound buffer sizing without OS calls.
        // This keeps behavior consistent across platforms and avoids Windows-only deps.
        4 * 1024 * 1024 * 1024 // 4GB fallback
    }

    /// Calculate optimal buffer size based on file size and available memory
    pub fn calculate_buffer_size(&self, file_size: u64, is_network: bool) -> usize {
        // Get or cache available memory
        let available_memory = {
            let mut cached = self.cached_available_memory.lock();
            if let Some(mem) = *cached {
                mem
            } else {
                let mem = Self::get_available_memory();
                *cached = Some(mem);
                mem
            }
        };

        // For network transfers, use larger buffers
        let base_size = if is_network {
            8 * 1024 * 1024 // 8MB for network
        } else {
            4 * 1024 * 1024 // 4MB for local
        };

        // Scale based on file size
        let optimal_size = if file_size < 10 * 1024 * 1024 {
            // Small file: smaller buffer
            self.min_buffer_size
        } else if file_size < 100 * 1024 * 1024 {
            // Medium file: medium buffer
            base_size
        } else {
            // Large file: max buffer
            self.max_buffer_size
        };

        // Ensure we don't use more than 10% of available memory
        let memory_limit = (available_memory / 10) as usize;
        optimal_size.min(memory_limit).max(self.min_buffer_size)
    }

    /// Get buffer size for parallel operations (smaller to avoid memory pressure)
    pub fn calculate_parallel_buffer_size(&self, thread_count: usize, is_network: bool) -> usize {
        let single_buffer = self.calculate_buffer_size(100 * 1024 * 1024, is_network);
        // Avoid division by zero by treating 0 threads as 1
        let threads = thread_count.max(1);

        // Divide by thread count but maintain minimum
        (single_buffer / threads).max(256 * 1024) // 256KB minimum
    }
}

impl Default for BufferSizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_buffer_size_handles_zero_threads() {
        let sizer = BufferSizer::new();
        let expected = sizer.calculate_buffer_size(100 * 1024 * 1024, false);
        assert_eq!(sizer.calculate_parallel_buffer_size(0, false), expected);
    }
}
