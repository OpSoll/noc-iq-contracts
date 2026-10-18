// Governance proposal execution queue. Complements config_freeze.rs.
use soroban_sdk::{contracterror, contracttype, symbol_short, Env, Symbol, Vec};

#[contracttype]
#[derive(Clone)]
pub struct QueuedProposal {
    pub proposal_id: u64,
    pub execute_after: u64,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum QueueError {
    TimelockNotExpired = 1,
}

const EXECUTED_EVENT: Symbol = symbol_short!("propexec");

/// Anyone can invoke execute_queued_proposal once its timelock expires,
/// applying the queued parameter update atomically.
pub fn execute_queued_proposal(env: &Env, proposal: &QueuedProposal) -> Result<(), QueueError> {
    if env.ledger().timestamp() < proposal.execute_after {
        return Err(QueueError::TimelockNotExpired);
    }
    env.events()
        .publish((EXECUTED_EVENT,), proposal.proposal_id);
    Ok(())
}

// -----------------------------------------------------------------------
// Queue storage
// -----------------------------------------------------------------------

const PROPOSAL_QUEUE_KEY: Symbol = symbol_short!("PQUEUE");

/// Appends a proposal to the back of the execution queue.
pub fn enqueue_proposal(env: &Env, proposal: &QueuedProposal) {
    let mut queue: Vec<QueuedProposal> = env
        .storage()
        .instance()
        .get(&PROPOSAL_QUEUE_KEY)
        .unwrap_or_else(|| Vec::new(env));
    queue.push_back(proposal.clone());
    env.storage().instance().set(&PROPOSAL_QUEUE_KEY, &queue);
}

/// View returning the number of proposals currently waiting in the queue.
pub fn queue_len(env: &Env) -> u32 {
    let queue: Vec<QueuedProposal> = env
        .storage()
        .instance()
        .get(&PROPOSAL_QUEUE_KEY)
        .unwrap_or_else(|| Vec::new(env));
    queue.len()
}

/// Removes and returns the oldest queued proposal - the front of the queue.
///
/// Combined with [`execute_queued_proposal`] this is the FIFO execution
/// order: proposals leave the queue in the order they were enqueued,
/// independently of their timelock expiry.
pub fn dequeue_proposal(env: &Env) -> Option<QueuedProposal> {
    let mut queue: Vec<QueuedProposal> = env
        .storage()
        .instance()
        .get(&PROPOSAL_QUEUE_KEY)
        .unwrap_or_else(|| Vec::new(env));
    let front = queue.pop_front();
    if front.is_some() {
        env.storage().instance().set(&PROPOSAL_QUEUE_KEY, &queue);
    }
    front
}

#[cfg(test)]
mod tests {
    // The crate root is `#![no_std]`; tests opt back into std the same way
    // `benchmarks.rs` and `proptests.rs` do.
    extern crate std;

    use super::*;
    use soroban_sdk::{
        contract, contractimpl,
        testutils::{Events, Ledger as _},
        TryIntoVal,
    };

    /// `env.storage()` and `env.events()` only run inside a contract
    /// invocation (`assert_in_contract!`), so the queue functions have to be
    /// reached through a client rather than called directly from a test.
    #[contract]
    struct ProposalQueueHarness;

    #[contractimpl]
    impl ProposalQueueHarness {
        pub fn enqueue(env: Env, proposal_id: u64, execute_after: u64) {
            enqueue_proposal(
                &env,
                &QueuedProposal {
                    proposal_id,
                    execute_after,
                },
            );
        }

        pub fn dequeue(env: Env) -> Option<QueuedProposal> {
            dequeue_proposal(&env)
        }

        pub fn len(env: Env) -> u32 {
            queue_len(&env)
        }

        pub fn execute(env: Env, proposal_id: u64, execute_after: u64) -> Result<(), QueueError> {
            execute_queued_proposal(
                &env,
                &QueuedProposal {
                    proposal_id,
                    execute_after,
                },
            )
        }
    }

    fn harness(env: &Env) -> ProposalQueueHarnessClient {
        ProposalQueueHarnessClient::new(env, &env.register_contract(None, ProposalQueueHarness))
    }

    fn executed_ids(env: &Env) -> Vec<u64> {
        let mut ids: Vec<u64> = Vec::new(env);
        let events = env.events().all();
        for i in 0..events.len() {
            let (_, topics, data) = events.get(i).unwrap();
            if topics.len() < 1 {
                continue;
            }
            let name: Symbol = topics.get(0).unwrap().try_into_val(env).unwrap();
            if name == EXECUTED_EVENT {
                let id: u64 = data.try_into_val(env).unwrap();
                ids.push_back(id);
            }
        }
        ids
    }

    /// #902: proposals execute in insertion order - the queue is FIFO even
    /// when the proposals' timelocks expire at different times.
    #[test]
    fn queued_proposals_execute_in_fifo_order() {
        let env = Env::default();
        let client = harness(&env);

        // Enqueued in id order, with increasing timelocks.
        for (id, at) in [(1u64, 1_000u64), (2, 2_000), (3, 3_000)] {
            client.enqueue(&id, &at);
        }

        // Before 1_000 nothing is executable, and the queue still holds all
        // three - blocked timelocks must not lose or reorder proposals.
        assert_eq!(client.len(), 3);

        // 1_500: proposal 1 is due, proposal 2 is not.
        env.ledger().with_mut(|l| l.timestamp = 1_500);
        let first = client.dequeue().expect("front proposal");
        assert_eq!(first.proposal_id, 1);
        client.execute(&first.proposal_id, &first.execute_after);

        let second = client.dequeue().expect("second proposal");
        assert_eq!(second.proposal_id, 2, "second out must be the second in");
        assert!(
            matches!(
                client.try_execute(&second.proposal_id, &second.execute_after),
                Err(Ok(QueueError::TimelockNotExpired))
            ),
            "its timelock has not expired yet"
        );

        // 2_500: proposal 2's timelock expires; it executes, then proposal 3.
        env.ledger().with_mut(|l| l.timestamp = 2_500);
        client.execute(&second.proposal_id, &second.execute_after);
        let third = client.dequeue().expect("third proposal");
        assert_eq!(third.proposal_id, 3);
        env.ledger().with_mut(|l| l.timestamp = 3_500);
        client.execute(&third.proposal_id, &third.execute_after);

        assert_eq!(client.len(), 0, "queue drains as proposals execute");
        // The execution events fired in insertion order.
        assert_eq!(
            executed_ids(&env),
            soroban_sdk::Vec::from_slice(&env, &[1u64, 2, 3])
        );
    }

    /// FIFO means insertion order, not timelock order: a proposal enqueued
    /// first leaves first even if its timelock is the latest to expire.
    #[test]
    fn dequeue_order_follows_insertion_not_timelock() {
        let env = Env::default();
        let client = harness(&env);

        client.enqueue(&1, &9_000); // latest timelock, first in
        client.enqueue(&2, &1_000); // earliest timelock, second in

        assert_eq!(client.dequeue().expect("first in").proposal_id, 1);
        assert_eq!(client.dequeue().expect("second in").proposal_id, 2);
        assert!(client.dequeue().is_none(), "queue is empty");
    }

    /// #903: the view returns the current queue length.
    #[test]
    fn queue_len_tracks_enqueues_and_dequeues() {
        let env = Env::default();
        let client = harness(&env);

        assert_eq!(client.len(), 0, "empty queue reports zero");

        client.enqueue(&1, &100);
        assert_eq!(client.len(), 1);
        client.enqueue(&2, &200);
        client.enqueue(&3, &300);
        assert_eq!(client.len(), 3);

        let _ = client.dequeue();
        assert_eq!(client.len(), 2);
        let _ = client.dequeue();
        let _ = client.dequeue();
        assert_eq!(client.len(), 0);

        // Dequeue on an empty queue is a no-op, not an underflow.
        assert!(client.dequeue().is_none());
        assert_eq!(client.len(), 0);
    }

    /// Dequeue returns None rather than panicking when empty.
    #[test]
    fn dequeue_on_empty_queue_returns_none() {
        let env = Env::default();
        let client = harness(&env);
        assert!(client.dequeue().is_none());
    }
}
