use std::{thread::sleep, time::Duration};

use crate::{Gcr, GcrRequestError};

#[test]
fn test_request() {
    let mut rate: Gcr = Gcr::new(100, Duration::from_millis(100), Some(500))
        .expect("Failed to create GCR instance");

    // Make sure we can't request more than the max burst, even if we wait
    sleep(Duration::from_millis(100));
    assert!(matches!(
        rate.request(501),
        Err(GcrRequestError::RequestTooLarge)
    ));
    assert!(rate.capacity() == 500);

    // Make sure we can request up to the burst
    rate.request(500).expect("Failed to request burst");
    assert!(rate.capacity() == 0 && rate.request(1).is_err());
    assert!(rate.allow_at.elapsed().as_secs() == 0);

    // Make sure the rate is consistent
    sleep(Duration::from_millis(100));
    assert!(rate.capacity() / 10 == 10);

    // Make sure we are denied for the correct amount of time
    sleep(Duration::from_millis(100));
    let Err(GcrRequestError::DeniedFor(duration)) = rate.request(500) else {
        panic!("Expected a denied for error");
    };
    assert!(
        duration.as_millis() / 10 == 29
            || duration.as_millis() / 10 == 30
            || duration.as_millis() / 10 == 28
    );
}

#[test]
fn test_adjust() {
    let mut rate: Gcr = Gcr::new(100, Duration::from_millis(100), Some(500))
        .expect("Failed to create GCR instance");

    // Make sure the capacity stays the same when we adjust the parameters
    let mut rate2 = rate.clone();
    rate2
        .adjust(200, Duration::from_millis(100), Some(1000))
        .expect("Failed to adjust GCR");
    assert!(rate.capacity() == rate2.capacity());

    // Make sure we respect the new rate and burst
    rate.request(200).expect("Failed to request 200 units");
    rate.adjust(200, Duration::from_millis(100), Some(1000))
        .expect("Failed to adjust GCR");
    assert!(rate.capacity() == 300);
    sleep(Duration::from_millis(200));
    assert!(rate.capacity() / 100 == 7);
}
