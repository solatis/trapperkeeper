// internal/types/rules.go
package types

/*
 * Domain types for rule evaluation.
 *
 * Provides Rule, OrGroup, Condition, and PathSegment structures used by
 * internal/rules for compilation and evaluation. These types are wire-format
 * agnostic - proto-to-types conversion happens at SDK/API boundary per
 * doc/10-integration/package-separation.md.
 *
 * Key types:
 *   - Rule: Complete rule definition with DNF structure
 *   - OrGroup: AND group (all conditions must match)
 *   - Condition: Single comparison with field path and operator
 *   - PathSegment: One component of a JSON path (key, index, or wildcard)
 *
 * Dependencies: None (zero external dependencies, encoding/json only)
 */

// PathSegment represents one component of a field path.
// String for object keys, int for array indices, wildcard for array expansion.
type PathSegment struct {
	Key      string // object key (mutually exclusive with Index/Wildcard)
	Index    int    // array index (mutually exclusive with Key/Wildcard)
	IsIndex  bool   // disambiguates Index=0 from unset
	Wildcard bool   // true = wildcard segment
}

// Condition represents a single condition in a rule expression.
type Condition struct {
	FieldPath      []PathSegment // path to field in payload
	FieldRef       []PathSegment // path to comparison field (mutually exclusive with Value)
	Operator       int           // operator enum value
	FieldType      int           // field type enum value
	Value          any           // comparison value (nil for exists/is_null)
	Values         []any         // for IN operator
	OnMissingField int           // policy enum value
	OnCoercionFail int           // policy enum value
}

// OrGroup represents an AND group in DNF (all conditions must match).
type OrGroup struct {
	Conditions []Condition
}

// Rule represents a complete rule definition for compilation.
type Rule struct {
	RuleID     RuleID    // immutable identifier
	Name       string    // human-readable name
	SampleRate float64   // [0.0, 1.0] sampling rate
	OrGroups   []OrGroup // DNF: OR of AND groups
	Action     int       // action enum value
}
