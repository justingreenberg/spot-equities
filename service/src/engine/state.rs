use crate::db::models::RequestStatus;

/// Validates that a state transition is allowed.
pub fn is_valid_transition(from: &RequestStatus, to: &RequestStatus) -> bool {
    matches!(
        (from, to),
        (RequestStatus::Detected, RequestStatus::Pending)
            | (RequestStatus::Pending, RequestStatus::Processing)
            | (RequestStatus::Pending, RequestStatus::Failed)
            | (RequestStatus::Processing, RequestStatus::DinariCompleted)
            | (RequestStatus::Processing, RequestStatus::Failed)
            | (RequestStatus::DinariCompleted, RequestStatus::ReadyToFulfill)
            | (RequestStatus::ReadyToFulfill, RequestStatus::Fulfilled)
            | (RequestStatus::ReadyToFulfill, RequestStatus::FulfillmentFailed)
            | (RequestStatus::FulfillmentFailed, RequestStatus::ReadyToFulfill)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        assert!(is_valid_transition(&RequestStatus::Detected, &RequestStatus::Pending));
        assert!(is_valid_transition(&RequestStatus::Pending, &RequestStatus::Processing));
        assert!(is_valid_transition(&RequestStatus::Processing, &RequestStatus::DinariCompleted));
        assert!(is_valid_transition(&RequestStatus::DinariCompleted, &RequestStatus::ReadyToFulfill));
        assert!(is_valid_transition(&RequestStatus::ReadyToFulfill, &RequestStatus::Fulfilled));
        assert!(is_valid_transition(&RequestStatus::ReadyToFulfill, &RequestStatus::FulfillmentFailed));
        assert!(is_valid_transition(&RequestStatus::FulfillmentFailed, &RequestStatus::ReadyToFulfill));
    }

    #[test]
    fn test_invalid_transitions() {
        assert!(!is_valid_transition(&RequestStatus::Detected, &RequestStatus::Fulfilled));
        assert!(!is_valid_transition(&RequestStatus::Fulfilled, &RequestStatus::Pending));
        assert!(!is_valid_transition(&RequestStatus::Failed, &RequestStatus::Pending));
    }
}
