use crate::generators::traits::ValueGenerator;
use once_cell::sync::Lazy;
use rand::seq::IndexedRandom;

const WORDLIST: &str = include_str!("../assets/eff_short_wordlist.txt");

static FILTERED_WORDS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    WORDLIST
        .lines()
        .filter(|l| !l.is_empty() && !l.contains('-'))
        .collect()
});

pub struct PassphraseGenerator {
    word_count: usize,
}

impl PassphraseGenerator {
    pub fn new(word_count: usize) -> Self {
        Self { word_count }
    }
}

impl ValueGenerator for PassphraseGenerator {
    fn generate(&self) -> String {
        let mut rng = rand::rng();
        (0..self.word_count)
            .map(|_| *FILTERED_WORDS.choose(&mut rng).unwrap_or(&"word"))
            .collect::<Vec<_>>()
            .join("-")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck_macros::quickcheck;

    #[test]
    fn wordlist_loaded() {
        assert!(!FILTERED_WORDS.is_empty());
        assert!(FILTERED_WORDS.len() > 1000);
    }

    #[quickcheck]
    fn generates_correct_word_count(count: u8) -> bool {
        // Use % 9 + 1 to guarantee range 1-9 (avoids edge case of 0)
        let count = (count as usize % 9) + 1;
        let generator = PassphraseGenerator::new(count);
        generator.generate().split('-').count() == count
    }

    #[quickcheck]
    fn generates_unique_passphrases(seed: u8) -> bool {
        let _ = seed;
        let generator = PassphraseGenerator::new(6);
        let a = generator.generate();
        let b = generator.generate();
        a != b
    }

    #[quickcheck]
    fn words_are_lowercase_alpha(count: u8) -> bool {
        // Use % 9 + 1 to guarantee range 1-9
        let count = (count as usize % 9) + 1;
        let generator = PassphraseGenerator::new(count);
        let result = generator.generate();
        result
            .split('-')
            .all(|word| word.chars().all(|c| c.is_ascii_lowercase()))
    }
}
