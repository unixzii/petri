pub trait VecExt {
    type Item;

    fn split_off_with<F>(&mut self, pred: F) -> Option<Vec<Self::Item>>
    where
        F: FnMut(&Self::Item) -> bool;
}

impl<T> VecExt for Vec<T> {
    type Item = T;

    fn split_off_with<F>(&mut self, mut pred: F) -> Option<Vec<T>>
    where
        F: FnMut(&Self::Item) -> bool,
    {
        for i in (0..self.len()).rev() {
            if pred(&self[i]) {
                let split_len = i + 1;
                self.rotate_left(split_len);
                return Some(self.split_off(self.len() - split_len));
            }
        }
        None
    }
}
