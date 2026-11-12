//! ONE shared state instance per acceptor, borrowed by both protocol groups.
//! This memory-only scaffold assumes crash-stop: a lost instance must never
//! rejoin the same log under the same identity. Restart recovery requires durable
//! promise/accept writes BEFORE sending Promise/Accepted responses.
use std::collections::BTreeMap;

use super::types::{AcceptedEntry, AcceptedHistory, Ballot, LogSlot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptorError {
    Preempted(Ballot),
    ConflictingValue,
}

#[derive(Default)]
pub struct AcceptorState {
    promised: Option<Ballot>,
    accepted: BTreeMap<LogSlot, AcceptedEntry>,
}

impl AcceptorState {
    pub fn promised(&self) -> Option<Ballot> {
        self.promised
    }

    /// Prepare contract: reject lower ballots; persist the promise before reply.
    /// Equal-ballot retries are idempotent. This implementation has no pagination.
    pub fn promise(&mut self, ballot: Ballot) -> Result<AcceptedHistory, AcceptorError> {
        self.check_ballot(ballot)?;
        self.promised = Some(ballot);
        Ok(AcceptedHistory::Complete(
            self.accepted.values().cloned().collect(),
        ))
    }

    /// Accept contract: ballot >= promise, and no different value for the SAME
    /// ballot+slot. A higher ballot may replace the value only because its leader
    /// obeys Phase 1 recovery. Update BOTH promise and accepted state before ack.
    pub fn accept(&mut self, entry: AcceptedEntry) -> Result<(), AcceptorError> {
        self.check_ballot(entry.ballot)?;
        if let Some(old) = self.accepted.get(&entry.slot)
            && old.ballot == entry.ballot
            && old.value != entry.value
        {
            return Err(AcceptorError::ConflictingValue);
        }
        self.promised = Some(entry.ballot);
        self.accepted.insert(entry.slot, entry);
        Ok(())
    }

    pub fn accepted(&self, slot: LogSlot) -> Option<&AcceptedEntry> {
        self.accepted.get(&slot)
    }

    fn check_ballot(&self, ballot: Ballot) -> Result<(), AcceptorError> {
        if let Some(promised) = self.promised
            && ballot < promised
        {
            return Err(AcceptorError::Preempted(promised));
        }
        Ok(())
    }
}
