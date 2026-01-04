// internal/rules/evaluate.go
package rules

import (
	"crypto/rand"
	"encoding/binary"
	"encoding/json"

	"github.com/solatis/trapperkeeper/internal/types"
)

/*
 * Rule evaluation orchestration.
 *
 * Evaluates CompiledRule against JSON payload with DNF semantics (OR of AND groups).
 * Implements short-circuit optimization, sample rate filtering, and match diagnostics.
 *
 * Evaluation flow:
 *   1. Sample rate check (fast-path for 0.0/1.0, crypto/rand for other values)
 *   2. OR groups evaluation (short-circuit on first match)
 *   3. AND conditions evaluation (short-circuit on first non-match, cost-ordered)
 *   4. Per-condition: resolve path -> coerce type -> compare operator
 *   5. Apply on_missing_field and on_coercion_fail policies
 *   6. Record matched_field, matched_value, matched_condition for diagnostics
 *
 * Policy handling:
 *   - Null/missing field: defers to on_missing_field (skip/match/fail)
 *   - Coercion failure: defers to on_coercion_fail (skip/match/error)
 *   - Decision Log: null vs coercion failure use separate policies
 *
 * Sample rate implementation: crypto/rand provides secure randomness for
 * consistent sampling. Rate 0.0 never evaluates (fast-path), 1.0 always
 * evaluates (no RNG call), intermediate values use RNG with fail-safe on error.
 *
 * Short-circuit semantics: First matching OR group stops evaluation. Within
 * AND group, first non-matching condition stops group evaluation. Cost ordering
 * from compilation maximizes short-circuit benefit.
 */

// MatchResult contains the outcome of rule evaluation.
type MatchResult struct {
	Matched          bool
	MatchedField     []types.PathSegment
	MatchedValue     any
	MatchedCondition []any
	Action           Action
	RuleID           types.RuleID
	RuleName         string
}

// Evaluate checks if the rule matches the given payload.
func Evaluate(rule *CompiledRule, payload json.RawMessage) (MatchResult, error) {
	result := MatchResult{
		RuleID:   rule.RuleID,
		RuleName: rule.Name,
		Action:   rule.Action,
	}

	if rule.SampleRate == 0.0 {
		return result, nil
	}
	if rule.SampleRate < 1.0 {
		if !shouldSample(rule.SampleRate) {
			return result, nil
		}
	}

	for groupIdx, group := range rule.OrGroups {
		matched, field, value, err := evaluateGroup(group, payload)
		if err != nil {
			return result, err
		}
		if matched {
			result.Matched = true
			result.MatchedField = field
			result.MatchedValue = value
			result.MatchedCondition = []any{"any", groupIdx, "all"}
			return result, nil
		}
	}

	return result, nil
}

// evaluateGroup evaluates AND group (all conditions must match).
// Short-circuits on first non-match. Returns matched field/value from first condition.
func evaluateGroup(group CompiledOrGroup, payload json.RawMessage) (bool, []types.PathSegment, any, error) {
	var firstField []types.PathSegment
	var firstValue any

	for i, cond := range group.Conditions {
		matched, field, value, err := evaluateCondition(cond, payload)
		if err != nil {
			return false, nil, nil, err
		}
		if !matched {
			return false, nil, nil, nil
		}
		if i == 0 {
			firstField = field
			firstValue = value
		}
	}

	return true, firstField, firstValue, nil
}

// evaluateCondition evaluates a single condition against payload.
// Orchestrates: resolve path -> coerce type -> compare operator.
// Applies on_missing_field and on_coercion_fail policies.
func evaluateCondition(cond CompiledCondition, payload json.RawMessage) (bool, []types.PathSegment, any, error) {
	resolved, err := Resolve(cond.Path, payload)
	if err != nil {
		if err == types.ErrFieldNotFound {
			return applyMissingPolicy(cond.OnMissing), nil, nil, nil
		}
		return false, nil, nil, err
	}

	if !resolved.Found {
		return applyMissingPolicy(cond.OnMissing), nil, nil, nil
	}

	coerced, err := Coerce(resolved.Value, cond.FieldType)
	if err != nil {
		if err == types.ErrCoercionFailed {
			return applyCoercionPolicy(cond.OnCoercion), resolved.ResolvedPath, resolved.Value, nil
		}
		return false, nil, nil, err
	}

	if coerced.IsNull {
		return applyMissingPolicy(cond.OnMissing), resolved.ResolvedPath, nil, nil
	}

	var target any
	if len(cond.FieldRef) > 0 {
		refResolved, err := Resolve(cond.FieldRef, payload)
		if err != nil || !refResolved.Found {
			return applyMissingPolicy(cond.OnMissing), resolved.ResolvedPath, coerced.Value, nil
		}
		refCoerced, err := Coerce(refResolved.Value, cond.FieldType)
		if err != nil || refCoerced.IsNull {
			return applyMissingPolicy(cond.OnMissing), resolved.ResolvedPath, coerced.Value, nil
		}
		target = refCoerced.Value
	} else if cond.Operator == OpIn {
		target = cond.Values
	} else {
		target = cond.Value
	}

	matched := Compare(cond.Operator, coerced.Value, target)
	return matched, resolved.ResolvedPath, coerced.Value, nil
}

// applyMissingPolicy converts OnMissingField policy to boolean match result.
// SKIP/FAIL -> false, MATCH -> true. Used for null values and missing fields.
func applyMissingPolicy(policy OnMissingField) bool {
	switch policy {
	case OnMissingMatch:
		return true
	case OnMissingFail:
		return false
	default:
		return false
	}
}

// applyCoercionPolicy converts OnCoercionPolicy policy to boolean match result.
// MATCH -> true, SKIP/ERROR -> false. Used when type coercion fails.
func applyCoercionPolicy(policy OnCoercionPolicy) bool {
	switch policy {
	case OnCoercionMatch:
		return true
	default:
		return false
	}
}

// shouldSample determines if event should be evaluated based on sample rate.
// Uses crypto/rand for secure uniform distribution. Fail-safe returns false on RNG error.
func shouldSample(rate float64) bool {
	var buf [8]byte
	if _, err := rand.Read(buf[:]); err != nil {
		return false
	}
	n := binary.BigEndian.Uint64(buf[:])
	f := float64(n) / float64(1<<64)
	return f < rate
}
