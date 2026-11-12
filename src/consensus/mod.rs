//! Teachable consensus boundary. Only `LocalAgreement` is implemented end to end.
//! See docs/consensus.md for ownership, recovery, and failure-model contracts.
pub mod acceptor;
pub mod agreement;
pub mod application;
pub mod batcher;
pub mod leadership;
pub mod session;
pub mod types;

#[cfg(test)]
mod tests;
