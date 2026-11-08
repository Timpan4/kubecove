//! A fixed-size set of row indices, one bit per row.

pub(crate) struct Bits(Vec<u64>);

impl Bits {
    pub(crate) fn new(len: usize) -> Self {
        Self(vec![0; len.div_ceil(64)])
    }

    pub(crate) fn insert(&mut self, index: usize) {
        self.0[index / 64] |= 1 << (index % 64);
    }

    pub(crate) fn contains(&self, index: usize) -> bool {
        self.0[index / 64] & (1 << (index % 64)) != 0
    }
}
