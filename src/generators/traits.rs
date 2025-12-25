#[cfg_attr(test, mockall::automock)]
pub trait ValueGenerator: Send + Sync {
    fn generate(&self) -> String;
}
