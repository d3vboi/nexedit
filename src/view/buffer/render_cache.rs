use std::collections::HashMap;

pub trait RenderCache {
    fn invalidate_from(&mut self, _: usize) {}
}

impl<T> RenderCache for HashMap<usize, T> {
    fn invalidate_from(&mut self, limit: usize) {
        self.retain(|&k, _| k < limit)
    }
}

#[cfg(test)]
mod tests {
    use super::RenderCache;
    use std::collections::HashMap;

    #[test]
    fn invalidate_from_clears_entries_starting_from_specified_index() {
        let mut cache = HashMap::new();
        cache.insert(100, String::new());
        cache.insert(200, String::new());
        cache.insert(300, String::new());
        cache.invalidate_from(200);

        let mut expected_cache = HashMap::new();
        expected_cache.insert(100, String::new());

        assert_eq!(cache, expected_cache);
    }
}
