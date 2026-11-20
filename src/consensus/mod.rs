//! Multi-Paxos teaching interface using the CS5414 notes' terminology.
//! PaxosServer owns shared protocol state; phase1/phase2 are implementation areas
//! for the two groups. Distributed methods remain Unsupported.
//! LocalProposer is a test double. The playable --NONE runtime does not use it.
pub mod acceptor;
pub mod batcher;
pub mod paxos;
pub mod phase1;
pub mod phase2;
pub mod proposer;
pub mod replica;
pub mod session;
pub mod types;

#[cfg(test)]
mod tests;
