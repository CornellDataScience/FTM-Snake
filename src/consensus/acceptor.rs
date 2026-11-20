//! The acceptor's ballot and accepted p-values, owned once by PaxosServer.
//! Memory-only, crash-stop: restart cannot safely rejoin the old log. Durable
//! writes would be required BEFORE P1b/P2b replies for restart recovery.
use std::collections::BTreeMap;

use super::types::{AcceptedHistory, Ballot, PValue, Slot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptorError {
    BallotMismatch { current: Option<Ballot> },
    ConflictingCommand,
}

#[derive(Default)]
pub struct AcceptorState {
    ballot: Option<Ballot>,
    accepted: BTreeMap<Slot, PValue>,
}

impl AcceptorState {
    pub fn ballot(&self) -> Option<Ballot> {
        self.ballot
    }

    /// P1a: adopt a higher ballot, or repeat the reply for the same ballot.
    /// Return accepted p-values for P1b. Lower ballots cannot change this state.
    pub fn on_p1a(&mut self, ballot: Ballot) -> Result<AcceptedHistory, AcceptorError> {
        if self.ballot.is_some_and(|current| ballot < current) {
            return Err(AcceptorError::BallotMismatch {
                current: self.ballot,
            });
        }
        self.ballot = Some(ballot);
        Ok(AcceptedHistory::Complete(
            self.accepted.values().cloned().collect(),
        ))
    }

    /// P2a: accept only when its ballot EQUALS the adopted ballot (Section 4,
    /// slide 34). A higher-ballot P2a must not implicitly perform phase 1.
    /// Same-ballot/slot retries must also carry the same command.
    pub fn on_p2a(&mut self, pvalue: PValue) -> Result<(), AcceptorError> {
        if self.ballot != Some(pvalue.ballot) {
            return Err(AcceptorError::BallotMismatch {
                current: self.ballot,
            });
        }
        if let Some(old) = self.accepted.get(&pvalue.slot)
            && old.ballot == pvalue.ballot
            && old.command != pvalue.command
        {
            return Err(AcceptorError::ConflictingCommand);
        }
        self.accepted.insert(pvalue.slot, pvalue);
        Ok(())
    }

    pub fn accepted(&self, slot: Slot) -> Option<&PValue> {
        self.accepted.get(&slot)
    }
}
