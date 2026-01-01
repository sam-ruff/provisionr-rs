use crate::storage::models::HashingAlgorithm;
use sha_crypt::{sha512_simple, Sha512Params};
use yescrypt::{PasswordHasher as YescryptPasswordHasher, Yescrypt};

pub trait PasswordHasher: Send + Sync {
    fn hash(&self, password: &str) -> String;
}

pub struct Sha512Hasher;

impl PasswordHasher for Sha512Hasher {
    fn hash(&self, password: &str) -> String {
        let params = Sha512Params::new(5000).expect("Invalid SHA-512 rounds");
        sha512_simple(password, &params).expect("SHA-512 hashing failed")
    }
}

pub struct YescryptHasher;

impl PasswordHasher for YescryptHasher {
    fn hash(&self, password: &str) -> String {
        Yescrypt
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
    fn create_hasher_returns_correct_type() {
        let hasher = create_hasher(&HashingAlgorithm::None);
        assert_eq!(hasher.hash("test"), "test");

        let hasher = create_hasher(&HashingAlgorithm::Sha512);
        assert!(hasher.hash("test").starts_with("$6$"));

        let hasher = create_hasher(&HashingAlgorithm::Yescrypt);
        assert!(hasher.hash("test").starts_with("$y$"));
    }
}
