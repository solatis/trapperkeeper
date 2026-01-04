// internal/rules/compile.go
package rules

import (
	"sort"

	"github.com/solatis/trapperkeeper/internal/types"
)

/*
 * Rule compilation and validation.
 *
 * Compiles types.Rule to CompiledRule with pre-ordered conditions, validated
 * resource limits, and calculated priority for cost-based evaluation.
 *
 * Compilation workflow:
 *   1. Validate resource limits (path depth, wildcards, IN values)
 *   2. Calculate condition costs using canonical cost model
 *   3. Order conditions by ascending cost (stable sort for determinism)
 *   4. Calculate rule priority from total cost + penalties
 *
 * Why compile-time validation: Enforcing limits during compilation moves
 * error detection to rule creation time rather than evaluation time. This
 * prevents unbounded evaluation costs in production.
 *
 * Why stable sort: Conditions with equal cost must maintain original order
 * to ensure deterministic matched_field reporting across identical inputs
 * (Decision Log: evaluation order stability).
 *
 * Field_ref constraint: Cross-field comparison paths cannot contain wildcards
 * because resolving both sides with wildcards creates N*M comparison matrix.
 */

// Operator mirrors the protobuf enum for condition operators.
type Operator int

const (
	OpUnspecified Operator = iota
	OpEq
	OpNeq
	OpLt
	OpLte
	OpGt
	OpGte
	OpPrefix
	OpSuffix
	OpIn
	OpExists
	OpIsNull
)

// OnMissingField policy for missing field handling.
type OnMissingField int

const (
	OnMissingSkip OnMissingField = iota
	OnMissingMatch
	OnMissingFail
)

// OnCoercionPolicy specifies behavior when type coercion fails.
type OnCoercionPolicy int

const (
	OnCoercionSkip OnCoercionPolicy = iota
	OnCoercionMatch
	OnCoercionError
)

// CompiledCondition is a pre-processed condition ready for evaluation.
type CompiledCondition struct {
	Path       []types.PathSegment
	Operator   Operator
	FieldType  FieldType
	Value      any   // comparison value (nil for exists/is_null)
	Values     []any // for IN operator
	FieldRef   []types.PathSegment // for cross-field comparison (mutually exclusive with Value)
	OnMissing  OnMissingField
	OnCoercion OnCoercionPolicy
	Cost       int
}

// CompiledOrGroup is a pre-processed AND group.
type CompiledOrGroup struct {
	Conditions []CompiledCondition // ordered by ascending cost
}

// CompiledRule is fully pre-processed and ready for evaluation.
type CompiledRule struct {
	RuleID     types.RuleID
	Name       string
	Action     Action
	SampleRate float64
	OrGroups   []CompiledOrGroup
	Priority   int // calculated from cost model
}

// Action mirrors the protobuf enum for rule actions.
type Action int

const (
	ActionUnspecified Action = iota
	ActionObserve
	ActionDrop
	ActionFail
)

// Compile validates and pre-processes a rule for efficient evaluation.
func Compile(rule *types.Rule) (*CompiledRule, error) {
	compiled := &CompiledRule{
		RuleID:     rule.RuleID,
		Name:       rule.Name,
		Action:     Action(rule.Action),
		SampleRate: rule.SampleRate,
		OrGroups:   make([]CompiledOrGroup, 0, len(rule.OrGroups)),
	}

	totalCost := 0

	for _, group := range rule.OrGroups {
		compiledGroup := CompiledOrGroup{
			Conditions: make([]CompiledCondition, 0, len(group.Conditions)),
		}

		for _, cond := range group.Conditions {
			cc, err := compileCondition(cond)
			if err != nil {
				return nil, err
			}
			compiledGroup.Conditions = append(compiledGroup.Conditions, cc)
			totalCost += cc.Cost
		}

		// Stable sort: equal-cost conditions maintain original order (deterministic matched_field)
		sort.SliceStable(compiledGroup.Conditions, func(i, j int) bool {
			return compiledGroup.Conditions[i].Cost < compiledGroup.Conditions[j].Cost
		})

		compiled.OrGroups = append(compiled.OrGroups, compiledGroup)
	}

	// Priority calculation per doc/05-performance/cost-model.md
	orPenalty := len(rule.OrGroups) * 10
	samplePenalty := int((1.0 - rule.SampleRate) * 50)
	compiled.Priority = BasePriority + totalCost + orPenalty + samplePenalty

	return compiled, nil
}

// compileCondition validates and pre-processes a single condition for evaluation.
// Enforces path depth, wildcard, and IN value limits. Calculates cost for ordering.
// Validates field_ref paths contain no wildcards (prevents N*M comparison matrix).
func compileCondition(cond types.Condition) (CompiledCondition, error) {
	path := cond.FieldPath

	// Validate path depth
	if len(path) > types.MaxPathDepth {
		return CompiledCondition{}, types.ErrPathTooDeep
	}

	// Validate wildcard count
	wildcardCount := 0
	for _, seg := range path {
		if seg.Wildcard {
			wildcardCount++
		}
	}
	if wildcardCount > types.MaxNestedWildcards {
		return CompiledCondition{}, types.ErrTooManyWildcards
	}

	// Validate field_ref: no wildcards (prevents N*M comparison matrix)
	fieldRef := cond.FieldRef
	if len(fieldRef) > 0 {
		for _, seg := range fieldRef {
			if seg.Wildcard {
				return CompiledCondition{}, types.ErrWildcardInFieldRef
			}
		}
	}

	op := Operator(cond.Operator)
	ft := FieldType(cond.FieldType)

	// Validate IN operator values
	if op == OpIn && len(cond.Values) > types.MaxInOperatorValues {
		return CompiledCondition{}, types.ErrTooManyInValues
	}

	cost := CalculateConditionCost(path, op, ft)

	return CompiledCondition{
		Path:       path,
		Operator:   op,
		FieldType:  ft,
		Value:      cond.Value,
		Values:     cond.Values,
		FieldRef:   fieldRef,
		OnMissing:  OnMissingField(cond.OnMissingField),
		OnCoercion: OnCoercionPolicy(cond.OnCoercionFail),
		Cost:       cost,
	}, nil
}
