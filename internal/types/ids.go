package types

import (
	"time"

	"github.com/google/uuid"
)

// NewEventID generates a UUIDv7 event identifier.
// Time-ordered IDs ensure sequential inserts cluster in B-tree pages.
// Panics on clock regression (uuid.Must); acceptable for ID generation.
func NewEventID() EventID {
	return EventID(uuid.Must(uuid.NewV7()).String())
}

// NewRuleID generates a UUIDv7 rule identifier.
// Panics on clock regression (uuid.Must); acceptable for ID generation.
func NewRuleID() RuleID {
	return RuleID(uuid.Must(uuid.NewV7()).String())
}

// ParseEventID validates and converts a string to EventID.
// Rejects malformed UUIDs to prevent invalid IDs from entering the system.
func ParseEventID(s string) (EventID, error) {
	_, err := uuid.Parse(s)
	if err != nil {
		return "", err
	}
	return EventID(s), nil
}

// ParseRuleID validates and converts a string to RuleID.
// Rejects malformed UUIDs to prevent invalid IDs from entering the system.
func ParseRuleID(s string) (RuleID, error) {
	_, err := uuid.Parse(s)
	if err != nil {
		return "", err
	}
	return RuleID(s), nil
}

// EventIDTime extracts the timestamp embedded in a UUIDv7 ID.
// Enables time-based queries without database lookup.
// Returns zero time for invalid UUIDs; caller should check IsZero().
func EventIDTime(id EventID) time.Time {
	u, err := uuid.Parse(string(id))
	if err != nil {
		return time.Time{}
	}
	sec, nsec := u.Time().UnixTime()
	return time.Unix(sec, nsec)
}
