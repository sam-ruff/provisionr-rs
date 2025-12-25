use once_cell::sync::Lazy;
use tokio_util::sync::CancellationToken;

// A single global token; every clone refers to the same cancellation state.
static GLOBAL_CANCEL_TOKEN: Lazy<CancellationToken> = Lazy::new(CancellationToken::new);

/// Call this to request shutdown of the whole program.
pub fn request_shutdown() {
    GLOBAL_CANCEL_TOKEN.cancel();
}

/// Get a clone of the global cancellation token.
///
/// Clones are cheap and all observe the same cancellation.
pub fn global_cancellation_token() -> CancellationToken {
    GLOBAL_CANCEL_TOKEN.clone()
}
