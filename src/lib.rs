//! A fast, simple, and small Generic Cell Rate ([`Gcr`]) algorithm implementation with zero dependencies
//! that allows for dynamic rate adjustment.
//!
//! # Usage
//!
//! ```rust
//! use gcr::Gcr;
//! use std::time::Duration;
//!
//! let mut rate = Gcr::new(10, Duration::from_secs(1), Some(30)).unwrap();
//!     // 10 units allowed every second with a max burst of 30 units at once
//!
//! rate.request(20).unwrap(); // Leftover capacity is now 10
//! rate.request(20).unwrap_err(); // Returns `DeniedFor(1 second)`
//! ```
//!
//! ## Rate adjustment
//!
//! [`Gcr::adjust`] can be used to change the rate of the limiter while preserving the current capacity.
//!
//! It accepts the same parameters as [`Gcr::new`].
//!
//! ```rust
//! use gcr::Gcr;
//! use std::time::Duration;
//!
//! let mut rate = Gcr::new(10, Duration::from_secs(1), Some(30)).unwrap();
//! 
//! rate.adjust(20, Duration::from_secs(1), Some(30)).unwrap();
//!     // Double the allowed rate while preserving the current capacity
//! ```
//!
//! ## Capacity
//!
//! [`Gcr::capacity`] can be used to get the current capacity of the rate limiter without making a request.

use core::fmt;
use std::{
    cmp::{self, max},
    fmt::Display,
    time::{Duration, Instant},
};

#[cfg(test)]
mod test;

/// Errors encountered when creating a new [`Gcr`] instance
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum GcrCreationError {
    ParametersOutOfRange(String),
}

/// Display implementation for [`GcrCreationError`]
impl Display for GcrCreationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParametersOutOfRange(msg) => write!(f, "Parameters out of range: {}", msg),
        }
    }
}

/// Errors encountered when requesting units from a [`Gcr`] instance
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum GcrRequestError {
    DeniedFor(Duration),
    RequestTooLarge,
    ParametersOutOfRange(String),
}

/// Display implementation for [`GcrRequestError`]
impl Display for GcrRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeniedFor(duration) => write!(f, "Request denied for {:?}", duration),
            Self::RequestTooLarge => write!(f, "Request was too large to ever be allowed"),
            Self::ParametersOutOfRange(msg) => write!(f, "Parameters out of range: {}", msg),
        }
    }
}

/// A generic cell rate (GCR) algorithm instance
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gcr {
    /// The "refill" rate
    emission_interval: Duration,
    delay_tolerance: Duration,
    /// The theoretical arrival time of the next unit
    theoretical_arrival_time: Instant,
    /// The time at which the next unit is allowed
    allow_at: Instant,
    /// The maximum number of units to allow in a single request
    max_burst: u32,
}

impl Gcr {
    /// Create a new [`Gcr`] instance.
    ///
    /// * `rate` - The number of units to "refill" per `period`
    /// * `period` - The amount of time between each "refill"
    /// * `max_burst` - The maximum number of units to allow in a single request. If
    /// not specified, this will be set to the rate.
    ///
    /// Returns a new [`Gcr`] instance on success.
    ///
    /// # Errors
    /// - [`GcrCreationError::ParametersOutOfRange`] - if the parameters are out of range
    pub fn new(
        rate: u32,
        period: Duration,
        max_burst: Option<u32>,
    ) -> Result<Self, GcrCreationError> {
        // The emission interval is the "refill" rate
        let emission_interval =
            period
                .checked_div(rate)
                .ok_or(GcrCreationError::ParametersOutOfRange(
                    "duration division failed: supplied rate was zero".to_string(),
                ))?;

        // If not set, the max burst is the rate
        let max_burst = max_burst.unwrap_or(rate);

        // The delay tolerance is the time between the theoretical arrival time and the
        // allow at time
        let delay_tolerance = emission_interval * max_burst;

        // This is set to the current time so we can instantly have our full burst
        let theoretical_arrival_time = Instant::now();

        // The allow_at time is the theoretical arrival time minus the delay tolerance
        let allow_at = theoretical_arrival_time
            .checked_sub(delay_tolerance)
            .ok_or(GcrCreationError::ParametersOutOfRange(
                "interval subtraction failed: max_burst * (period / rate) was too large"
                    .to_string(),
            ))?;

        Ok(Self {
            max_burst,
            emission_interval,
            delay_tolerance,
            theoretical_arrival_time,
            allow_at,
        })
    }

    /// Get the capacity of the rate limiter at a given time.
    /// 
    /// Note: this function calculates the capacity on the fly
    fn capacity_at(&self, now: Instant) -> u32 {
        // Get the duration since the allow at time
        let Some(time_since) = now.checked_duration_since(self.allow_at) else {
            return 0;
        };

        // Return the min of the number of emission intervals that have passed (units allowed)
        // and the max burst
        cmp::min(
            time_since.div_duration_f64(self.emission_interval) as u32,
            self.max_burst,
        )
    }

    /// Get the current capacity of the rate limiter
    /// 
    /// Note: this function calculates the capacity on the fly
    pub fn capacity(&self) -> u32 {
        self.capacity_at(Instant::now())
    }

    /// Request `n` units from the rate limiter.
    ///
    /// If the request was allowed through, this will return `Ok(())`. If not, it will return an error with the reason.
    ///
    /// # Errors
    /// - [`GcrRequestError::DeniedFor`] - if the request was denied. Includes the duration until the next successful request of the same size can be made.
    /// - [`GcrRequestError::RequestTooLarge`] - if the request was too large to ever be allowed. This happens if the request size is greater than the maximum burst (or the `rate` if it was not set)
    /// - [`GcrRequestError::ParametersOutOfRange`] - if the [`Gcr`] parameters are out of range
    pub fn request(&mut self, n: u32) -> Result<(), GcrRequestError> {
        // If the request is greater than the maximum request size, deny it with an error
        if n > self.max_burst {
            return Err(GcrRequestError::RequestTooLarge);
        }

        // This is the canonical request time
        let now = Instant::now();

        // If the request exceeds capacity, deny it
        if n > self.capacity_at(now) {
            // If we are not past the virtual theoretical arrival time, disallow the request

            // Calculate the time at which all units would have been allowed
            let allow_time = self.allow_at + (n * self.emission_interval);

            // See how far it is from the current time
            let denied_for = allow_time.checked_duration_since(now);
            if let Some(denied_for) = denied_for {
                return Err(GcrRequestError::DeniedFor(denied_for));
            }
        }

        // We are past the virtual theoretical arrival time, so allow the request

        // Update the theoretical arrival time to account for the new units consumed
        self.theoretical_arrival_time =
            max(self.theoretical_arrival_time, now) + (n * self.emission_interval);

        // Update the `allow_at` time to account for the new units consumed
        self.allow_at = self
            .theoretical_arrival_time
            .checked_sub(self.delay_tolerance)
            .ok_or(GcrRequestError::ParametersOutOfRange(
                "interval subtraction failed: delay_tolerance was too large".to_string(),
            ))?;

        Ok(())
    }

    /// Adjust the parameters of the rate limiter while preserving the current capacity.
    ///
    /// # Errors
    /// - [`GcrCreationError::ParametersOutOfRange`] - if the parameters are out of range
    pub fn adjust(
        &mut self,
        rate: u32,
        period: Duration,
        max_burst: Option<u32>,
    ) -> Result<(), GcrCreationError> {
        // Create a new `Gcr` with the new rate, period, and max burst
        let mut new_rate = Gcr::new(rate, period, max_burst)?;

        // This is the canonical request time
        let now = Instant::now();

        // Get the duration since the allow at time
        if let Some(time_since) = now.checked_duration_since(self.allow_at) {
            // Update the allow at time to account for the new rate
            new_rate.allow_at = now
                .checked_sub(
                    time_since.div_duration_f64(self.emission_interval) as u32
                        * new_rate.emission_interval,
                )
                .ok_or(GcrCreationError::ParametersOutOfRange(
                    "interval subtraction failed: emission_interval was too large".to_string(),
                ))?;

            // Update the theoretical arrival time to account for the new rate
            new_rate.theoretical_arrival_time = new_rate.allow_at + new_rate.delay_tolerance;
        }

        // Replace ourselves with the new rate
        *self = new_rate;

        Ok(())
    }
}
