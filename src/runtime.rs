#[cfg(feature = "tokio")]
pub use tokio::time::sleep;

#[cfg(feature = "async-std")]
pub use async_std::task::sleep;

#[cfg(not(any(feature = "tokio", feature = "async-std")))]
compile_error!("Either 'tokio' or 'async-std' feature must be enabled");
