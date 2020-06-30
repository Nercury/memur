pub struct EmptyIfDeadIter<K, I: ExactSizeIterator<Item=K>> {
    pub is_alive: bool,
    pub inner: I,
}

impl<K, I: ExactSizeIterator<Item=K>> ExactSizeIterator for EmptyIfDeadIter<K, I> {
    fn len(&self) -> usize {
        if self.is_alive {
            0
        } else {
            self.inner.len()
        }
    }
}

impl<K, I: ExactSizeIterator<Item=K>> Iterator for EmptyIfDeadIter<K, I> {
    type Item = K;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_alive {
            self.inner.next()
        } else {
            None
        }
    }
}