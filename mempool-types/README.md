# Mempool Types

**Experimental**: This crate is currently experimental and does not yet have dedicated integration tests.

Shared, transport-agnostic types and deserializers for the [mempool.space API](https://mempool.space/docs/api/rest).

This crate extends [`esplora-types`](../esplora-types) with mempool.space specific types for the `/api/v1/*` endpoints, while re-exporting all standard Esplora types for convenience.

This crate has no HTTP client dependency. See [`mempool-client`](../mempool-client) for a blocking and async HTTP client built on these types.
