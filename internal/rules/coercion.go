// internal/rules/coercion.go
package rules

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/solatis/trapperkeeper/internal/types"
)

/*
 * Type coercion for rule evaluation.
 *
 * Implements 5-type system (NUMERIC, TEXT, BOOLEAN, ANY, UNSPECIFIED) with
 * strict and lenient modes per doc/04-rule-engine/type-system-coercion.md.
 *
 * Key distinction: Null values vs coercion failures trigger different policies.
 * Null/nil defers to on_missing_field. Coercion failure (e.g., "abc" to numeric)
 * triggers on_coercion_fail. This separation enables distinct recovery strategies.
 *
 * Type modes:
 *   - NUMERIC: Strict - coerce strings to float64, reject booleans
 *   - TEXT: Lenient - auto-coerce all types to string
 *   - BOOLEAN: Strict - boolean only, reject strings/numbers
 *   - ANY: Lenient - preserve original type, cross-type comparison allowed
 *
 * Performance: String trimming for NUMERIC per spec (whitespace-only strings
 * are not valid numbers).
 */

// FieldType mirrors the protobuf enum for field type specification.
type FieldType int

const (
	FieldTypeUnspecified FieldType = iota
	FieldTypeNumeric
	FieldTypeText
	FieldTypeBoolean
	FieldTypeAny
)

// CoercionResult holds the coerced value or indicates null.
type CoercionResult struct {
	Value  any  // coerced value (valid only if !IsNull)
	IsNull bool // true if input was nil/null
}

// Coerce attempts to convert value to the expected field type.
// Returns CoercionResult with IsNull=true for nil input.
// Returns ErrCoercionFailed for impossible coercions.
func Coerce(value any, fieldType FieldType) (CoercionResult, error) {
	if value == nil {
		return CoercionResult{IsNull: true}, nil
	}

	switch fieldType {
	case FieldTypeNumeric:
		return coerceNumeric(value)
	case FieldTypeText:
		return coerceText(value)
	case FieldTypeBoolean:
		return coerceBoolean(value)
	case FieldTypeAny:
		return coerceAny(value)
	case FieldTypeUnspecified:
		// Treat unspecified as ANY
		return coerceAny(value)
	default:
		return CoercionResult{}, types.ErrCoercionFailed
	}
}

// coerceNumeric converts value to float64 for numeric comparison.
// Accepts float64, int, int64, and numeric strings. Rejects booleans per strict mode.
// Whitespace-only strings return ErrCoercionFailed.
func coerceNumeric(value any) (CoercionResult, error) {
	switch v := value.(type) {
	case float64:
		return CoercionResult{Value: v}, nil
	case int:
		return CoercionResult{Value: float64(v)}, nil
	case int64:
		return CoercionResult{Value: float64(v)}, nil
	case string:
		// Trim whitespace per doc/04-rule-engine/type-system-coercion.md:451
		v = strings.TrimSpace(v)
		if v == "" {
			// Empty/whitespace-only strings are not valid numbers
			return CoercionResult{}, types.ErrCoercionFailed
		}
		// ParseFloat handles both integer and decimal string representations
		f, err := strconv.ParseFloat(v, 64)
		if err != nil {
			return CoercionResult{}, types.ErrCoercionFailed
		}
		return CoercionResult{Value: f}, nil
	case bool:
		// Strict mode: reject boolean-to-numeric coercion
		return CoercionResult{}, types.ErrCoercionFailed
	default:
		return CoercionResult{}, types.ErrCoercionFailed
	}
}

// coerceText converts all types to string representation for text comparison.
// Lenient mode: accepts any type and converts to string.
func coerceText(value any) (CoercionResult, error) {
	// TEXT auto-coerces all types to string
	switch v := value.(type) {
	case string:
		return CoercionResult{Value: v}, nil
	case float64:
		return CoercionResult{Value: strconv.FormatFloat(v, 'f', -1, 64)}, nil
	case int:
		return CoercionResult{Value: strconv.Itoa(v)}, nil
	case int64:
		return CoercionResult{Value: strconv.FormatInt(v, 10)}, nil
	case bool:
		if v {
			return CoercionResult{Value: "true"}, nil
		}
		return CoercionResult{Value: "false"}, nil
	default:
		return CoercionResult{Value: fmt.Sprintf("%v", v)}, nil
	}
}

// coerceBoolean validates value is boolean type for boolean comparison.
// Strict mode: rejects strings and numbers to avoid "true" vs 1 ambiguity.
func coerceBoolean(value any) (CoercionResult, error) {
	switch v := value.(type) {
	case bool:
		return CoercionResult{Value: v}, nil
	default:
		// Strict mode: no string-to-boolean coercion (avoids "true" vs 1 ambiguity)
		return CoercionResult{}, types.ErrCoercionFailed
	}
}

// coerceAny preserves original type for lenient cross-type comparison.
// Allows numeric/string mixing; comparison logic handled by Compare() operators.
func coerceAny(value any) (CoercionResult, error) {
	// Lenient mode: preserve original type, allow cross-type comparison
	// Numeric/string comparison handled by Compare() operator logic
	return CoercionResult{Value: value}, nil
}
