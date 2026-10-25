//! Small helpers the original's code relies on.

/// ScummVM's `RandomSource`: numbers from 0 to `max` inclusive.
#[derive(Debug, Clone)]
pub struct Rng {
    seed: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self { seed }
    }

    /// `getRandomNumber(max)`: 0 to `max` inclusive.
    pub fn get(&mut self, max: u32) -> u32 {
        self.seed = 0xdead_bf03u32.wrapping_mul(self.seed.wrapping_add(1));
        self.seed = self.seed.rotate_right(13);
        if max == u32::MAX {
            self.seed
        } else {
            self.seed % (max + 1)
        }
    }

    /// `getRandomNumberRng(min, max)`.
    pub fn range(&mut self, min: i32, max: i32) -> i32 {
        // getRandomNumberRng always draws, also for an empty range.
        min + self.get((max - min).max(0) as u32) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_in_range() {
        let mut rng = Rng::new(42);
        for _ in 0..1000 {
            assert!(rng.get(5) <= 5);
            let r = rng.range(3, 7);
            assert!((3..=7).contains(&r));
        }
    }
}
