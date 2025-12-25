use crate::generators::traits::ValueGenerator;
use rand::distr::Alphanumeric;
use rand::Rng;

pub struct AlphanumericGenerator {
    length: usize,
}

impl AlphanumericGenerator {
    pub fn new(length: usize) -> Self {
        Self { length }
    }
}

impl ValueGenerator for AlphanumericGenerator {
    fn generate(&self) -> String {
        rand::rng()
            .sample_iter(&Alphanumeric)
            .take(self.length)
            .map(char::from)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck_macros::quickcheck;

    #[quickcheck]
    fn generates_correct_length(len: u8) -> bool {
        let len = (len as usize).max(1);
        let generator = AlphanumericGenerator::new(len);
        generator.generate().len() == len
    }

    #[quickcheck]
    fn generates_alphanumeric_only(len: u8) -> bool {
        let len = (len as usize).max(1);
        let generator = AlphanumericGenerator::new(len);
        generator.generate().chars().all(|c| c.is_ascii_alphanumeric())
    }

    #[quickcheck]
    fn generates_unique_values_with_sufficient_length(seed: u8) -> bool {
        let _ = seed;
        let generator = AlphanumericGenerator::new(32);
        let a = generator.generate();
        let b = generator.generate();
        a != b
    }
}
