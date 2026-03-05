---
title: "Understanding Paxos: The Foundation of Distributed Consensus"
date: 2026-03-06
tags:
  - engineering
  - distributed-systems
  - consensus
---

# Understanding Paxos: The Foundation of Distributed Consensus

Paxos is often described as one of the most difficult protocols to understand in computer science. Yet, it remains the bedrock of distributed systems, providing a way for a collection of unreliable processors to agree on a single value.

## The Problem: Consensus in an Unreliable World

In a distributed system, we want multiple nodes to act as a single, coherent unit. This requires consensus:
1. **Agreement**: All non-faulty nodes must decide on the same value.
2. **Validity**: The decided value must have been proposed by some node.
3. **Termination**: All non-faulty nodes eventually decide on a value.

The challenge? Nodes can crash, network packets can be delayed, lost, or reordered. Paxos solves this for "Asynchronous Consensus with Crash Failures."

## The Roles

Paxos defines three logical roles (a single node often plays multiple roles):
- **Proposers**: Suggest values to be agreed upon.
- **Acceptors**: The "memory" of the protocol. They form a quorum to decide which proposal wins.
- **Learners**: Act on the decided value once consensus is reached.

## The Basic Algorithm (Single-Decree Paxos)

Paxos operates in two phases to ensure that once a value is chosen, it is never changed.

### Phase 1: Prepare
1. **Proposer** chooses a unique proposal number `n` and sends a `prepare(n)` request to a majority of **Acceptors**.
2. **Acceptor** receives `prepare(n)`. If `n` is greater than any proposal number it has ever seen, it responds with a `promise` not to accept any more proposals numbered less than `n`. If it has already accepted a proposal, it must include that proposal's number and value in its response.

### Phase 2: Accept
1. If the **Proposer** receives responses from a majority of **Acceptors**, it sends an `accept(n, v)` request to those acceptors. 
   - What is `v`? If any acceptor reported an already-accepted value in Phase 1, the proposer *must* use the value from the highest-numbered proposal reported. Otherwise, it can choose its own value.
2. **Acceptor** receives `accept(n, v)`. It accepts the proposal unless it has already promised (in Phase 1) to only consider proposals higher than `n`.

## Why It Works

The magic of Paxos lies in the **Majority Quorum**. Any two majorities overlap by at least one node. 

If a majority of Acceptors have accepted a value `v` with number `n`, then any subsequent successful `prepare` request (with number `m > n`) will hit at least one node that has already accepted `v`. That node will force the new proposer to use `v`, ensuring the consensus remains stable.

## Beyond Single-Decree: Multi-Paxos

In practice, we don't just want to agree on one value; we want to agree on a *sequence* of values (a log). This is **Multi-Paxos**. 

By electing a "Distinguished Proposer" (a Leader), we can skip Phase 1 for most proposals, drastically improving performance. This is the pattern used by systems like Google's Chubby and many distributed databases.

## Conclusion

While protocols like Raft have gained popularity for being "understandable," Paxos remains the pure, mathematical heart of consensus. Understanding Paxos isn't just an academic exercise; it's about understanding how we build reliable worlds out of unreliable parts.
