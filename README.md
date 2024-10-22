# Generic Cell Rate (GCR) algorithm
[![crates.io](https://img.shields.io/crates/v/gcr)](https://crates.io/crates/gcr)
[![docs.rs](https://img.shields.io/docsrs/gcr)](https://docs.rs/gcr)

A fast, simple, and small Generic Cell Rate (`GCR`) algorithm implementation with zero dependencies
that allows for dynamic rate adjustment.

# Usage

```rust
use gcr::Gcr;

let mut rate = Gcr::new(10, Duration::from_secs(1), Some(30)).unwrap();
    // 10 units allowed every second with a max burst of 30 units at once

rate.request(20).unwrap(); // Leftover capacity is now 10
rate.request(20).unwrap_err(); // Returns `DeniedFor(1 second)`
```

## Rate adjustment

[`Gcr::adjust`] can be used to change the rate of the limiter while preserving the current capacity.

It accepts the same parameters as [`Gcr::new`].

```rust
rate.adjust(20, Duration::from_secs(1), Some(30)).unwrap();
    // 20 units allowed every second with a max burst of 30 units at once
```

## Capacity

[`Gcr::capacity`] can be used to get the current capacity of the rate limiter without making a request.

```rust
rate.capacity();
```