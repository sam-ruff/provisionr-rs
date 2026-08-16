use crate::storage::models::HashingAlgorithm;
use sha_crypt::{PasswordHasher as _, ShaCrypt};
use yescrypt::Yescrypt;

pub trait PasswordHasher: Send + Sync {
    fn hash(&self, password: &str) -> String;
}

pub struct Sha512Hasher;

impl PasswordHasher for Sha512Hasher {
    fn hash(&self, password: &str) -> String {
        ShaCrypt::SHA512
            .hash_password(password.as_bytes())
            .expect("SHA-512 hashing failed")
            .to_string()
    }
}

pub struct YescryptHasher;

impl PasswordHasher for YescryptHasher {
    fn hash(&self, password: &str) -> String {
        Yescrypt::default()
            .hash_password(password.as_bytes())
            .expect("Yescrypt hashing failed").to_string()
    }
}

pub struct NoOpHasher;

impl PasswordHasher for NoOpHasher {
    fn hash(&self, password: &str) -> String {
        password.to_string()
    }
}

pub fn create_hasher(algorithm: &HashingAlgorithm) -> Box<dyn PasswordHasher> {
    match algorithm {
        HashingAlgorithm::None => Box::new(NoOpHasher),
        HashingAlgorithm::Sha512 => Box::new(Sha512Hasher),
        HashingAlgorithm::Yescrypt => Box::new(YescryptHasher),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_op_hasher_returns_original() {
        let hasher = NoOpHasher;
        assert_eq!(hasher.hash("password123"), "password123");
    }

    #[test]
    fn sha512_hasher_produces_crypt_format() {
        let hasher = Sha512Hasher;
        let result = hasher.hash("testpassword");
        assert!(result.starts_with("$6$"), "SHA-512 hash should start with $6$");
    }

    #[test]
    fn yescrypt_hasher_produces_yescrypt_format() {
        let hasher = YescryptHasher;
        let result = hasher.hash("testpassword");
        assert!(result.starts_with("$y$"), "Yescrypt hash should start with $y$");
    }

    #[test]
    fn sha512_hash_uses_default_rounds_and_verifies() {
        use sha_crypt::PasswordVerifier;

        let result = Sha512Hasher.hash("testpassword");
        assert!(result.starts_with("$6$rounds=5000$"), "got {result}");
        assert!(ShaCrypt::SHA512.verify_password(b"testpassword", result.as_str()).is_ok());
        assert!(ShaCrypt::SHA512.verify_password(b"wrong", result.as_str()).is_err());
    }

    #[test]
    fn yescrypt_hash_verifies() {
        use yescrypt::PasswordVerifier;

        let result = YescryptHasher.hash("testpassword");
        assert!(Yescrypt::default().verify_password(b"testpassword", result.as_str()).is_ok());
        assert!(Yescrypt::default().verify_password(b"wrong", result.as_str()).is_err());
    }

    #[test]
    fn hashes_are_salted() {
        assert_ne!(Sha512Hasher.hash("same"), Sha512Hasher.hash("same"));
        assert_ne!(YescryptHasher.hash("same"), YescryptHasher.hash("same"));
    }

    #[test]
    fn create_hasher_returns_correct_type() {
        let hasher = create_hasher(&HashingAlgorithm::None);
        assert_eq!(hasher.hash("test"), "test");

        let hasher = create_hasher(&HashingAlgorithm::Sha512);
        assert!(hasher.hash("test").starts_with("$6$"));

        let hasher = create_hasher(&HashingAlgorithm::Yescrypt);
        assert!(hasher.hash("test").starts_with("$y$"));
    }
}
