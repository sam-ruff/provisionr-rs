use crate::generators::traits::ValueGenerator;
use rand::seq::IndexedRandom;

const WORDLIST: &str = include_str!("../assets/eff_short_wordlist.txt");

pub struct PassphraseGenerator {
    word_count: usize,
    words: Vec<&'static str>,
}

impl PassphraseGenerator {
    pub fn new(word_count: usize) -> Self {
        let words: Vec<&'static str> = WORDLIST
            .lines()
            .filter(|l| !l.is_empty() && !l.contains('-'))
            .collect();
        Self { word_count, words }
    }
}

impl ValueGenerator for PassphraseGenerator {
    fn generate(&self) -> String {
        let mut rng = rand::rng();
        (0..self.word_count)
            .map(|_| *self.words.choose(&mut rng).unwrap_or(&"word"))
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
        let generator = PassphraseGenerator::new(4);
        assert!(!generator.words.is_empty());
        assert!(generator.words.len() > 1000);
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
