// internal/rules/operators.go
package rules

import (
	"strings"
)

/*
 * Operator comparison logic.
 *
 * Implements 11 comparison operators with type-aware comparison rules.
 * Values should already be coerced via Coerce() before reaching Compare().
 *
 * Operators:
 *   - exists/is_null: Null checks (cost 1)
 *   - eq/neq: Equality with numeric tolerance (cost 5)
 *   - lt/lte/gt/gte: Numeric comparison only (cost 7)
 *   - prefix/suffix: String prefix/suffix matching (cost 10)
 *   - in: Membership test with equality semantics (cost 8)
 *
 * Numeric comparison: Handles float64/int/int64 mixing for JSON compatibility.
 * String comparison: Prefix/suffix operators reject non-string types (return false).
 *
 * Why function-based: User preference favors functional composition over
 * interface polymorphism. 11 operators via switch statement cleaner than
 * 11 interface implementations with minimal behavior variation.
 */

// Compare applies the operator to compare value against target.
// Both values should already be coerced to compatible types.
func Compare(op Operator, value, target any) bool {
	switch op {
	case OpExists:
		return value != nil
	case OpIsNull:
		return value == nil
	case OpEq:
		return compareEqual(value, target)
	case OpNeq:
		return !compareEqual(value, target)
	case OpLt:
		return compareNumeric(value, target) < 0
	case OpLte:
		return compareNumeric(value, target) <= 0
	case OpGt:
		return compareNumeric(value, target) > 0
	case OpGte:
		return compareNumeric(value, target) >= 0
	case OpPrefix:
		return comparePrefix(value, target)
	case OpSuffix:
		return compareSuffix(value, target)
	case OpIn:
		return compareIn(value, target)
	default:
		return false
	}
}

// compareEqual performs equality comparison with numeric type coercion.
// Handles float64/int/int64 mixing for JSON compatibility.
func compareEqual(a, b any) bool {
	if na, nb, ok := asNumbers(a, b); ok {
		return na == nb
	}
	return a == b
}

// compareNumeric performs three-way numeric comparison (-1/0/1).
// Returns 0 for incomparable types.
func compareNumeric(a, b any) int {
	na, nb, ok := asNumbers(a, b)
	if !ok {
		return 0
	}
	switch {
	case na < nb:
		return -1
	case na > nb:
		return 1
	default:
		return 0
	}
}

// asNumbers attempts to convert both values to float64 for numeric comparison.
// Returns converted values and success flag. Used by compareEqual and compareNumeric.
func asNumbers(a, b any) (float64, float64, bool) {
	na, oka := toFloat64(a)
	nb, okb := toFloat64(b)
	return na, nb, oka && okb
}

// toFloat64 converts value to float64 if it's a numeric type.
// Handles float64, int, int64 from JSON unmarshaling.
func toFloat64(v any) (float64, bool) {
	switch n := v.(type) {
	case float64:
		return n, true
	case int:
		return float64(n), true
	case int64:
		return float64(n), true
	default:
		return 0, false
	}
}

// comparePrefix checks if value starts with prefix (both must be strings).
// Returns false for non-string types.
func comparePrefix(value, prefix any) bool {
	vs, ok1 := value.(string)
	ps, ok2 := prefix.(string)
	if !ok1 || !ok2 {
		return false
	}
	return strings.HasPrefix(vs, ps)
}

// compareSuffix checks if value ends with suffix (both must be strings).
// Returns false for non-string types.
func compareSuffix(value, suffix any) bool {
	vs, ok1 := value.(string)
	ss, ok2 := suffix.(string)
	if !ok1 || !ok2 {
		return false
	}
	return strings.HasSuffix(vs, ss)
}

// compareIn checks if value exists in set using equality semantics.
// Set should be []any from IN operator values.
func compareIn(value, set any) bool {
	arr, ok := set.([]any)
	if !ok {
		return false
	}
	for _, elem := range arr {
		if compareEqual(value, elem) {
			return true
		}
	}
	return false
}
