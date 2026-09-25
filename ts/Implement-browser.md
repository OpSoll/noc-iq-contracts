 Implement browser local storage cache provider for SDK read queries
Repo Avatar
OpSoll/noc-iq-contracts
Problem
Repeated SDK queries for static contract config parameters cause unnecessary RPC traffic.

Proposed change
Incorporate an optional local storage caching layer in SDK for read-only contract query methods.

Acceptance criteria
Caches static contract configuration queries in browser local storage
Applies configurable TTL (e.g. 5 minutes) to cached responses
Provides clearCache() method to force fresh RPC queries
Unit test verifies cache hit and expiration behavior
Metadata
Suggested labels: enhancement, smart-contracts
Affected contract or module: sdk/src/client.ts
Dependencies: None

