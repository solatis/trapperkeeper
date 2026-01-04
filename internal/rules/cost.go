// internal/rules/cost.go
package rules

import "github.com/solatis/trapperkeeper/internal/types"

/*
 * Cost model for condition evaluation.
 *
 * Defines canonical cost constants from doc/05-performance/cost-model.md and
 * provides CalculateConditionCost for priority-based rule ordering.
 *
 * Cost formula: lookup_cost + (operator_cost * type_multiplier * 8^wildcards)
 *
 * Why cost-based ordering: Evaluating cheaper conditions first enables
 * short-circuit optimization. For non-matching events, average evaluation
 * time decreases significantly when exists (cost 1) runs before prefix (cost 10).
 *
 * Wildcard execution multiplier: 8^n reflects worst-case fanout per wildcard.
 * With MaxNestedWildcards=2, ceiling is 64x cost. Compile-time enforcement
 * prevents unbounded performance degradation.
 *
 * Constants defined here are single source of truth; referenced in
 * doc/05-performance/cost-model.md for alignment.
 */

// Canonical cost constants from doc/05-performance/cost-model.md
const (
	// Operator base costs
	CostExists = 1
	CostIsNull = 1
	CostEq     = 5
	CostNeq    = 5
	CostLt     = 7
	CostLte    = 7
	CostGt     = 7
	CostGte    = 7
	CostIn     = 8
	CostPrefix = 10
	CostSuffix = 10

	// Field lookup cost per string component
	CostLookupPerSegment = 128

	// Field type multipliers
	MultiplierInt    = 1
	MultiplierBool   = 1
	MultiplierFloat  = 4
	MultiplierString = 48
	MultiplierAny    = 128

	// Base priority offset
	BasePriority = 1000
)

// CalculateConditionCost computes cost for a single condition.
// cost = lookup_cost + (operator_cost * field_type_multiplier * 8^wildcards)
func CalculateConditionCost(path []types.PathSegment, op Operator, fieldType FieldType) int {
	lookupCost := 0
	wildcardCount := 0
	for _, seg := range path {
		if seg.Key != "" {
			lookupCost += CostLookupPerSegment
		}
		if seg.Wildcard {
			wildcardCount++
		}
	}

	opCost := operatorCost(op)
	typeMult := typeMultiplier(fieldType)

	// Execution multiplier: 8^n for n wildcards
	execMult := 1
	for i := 0; i < wildcardCount; i++ {
		execMult *= 8
	}

	return lookupCost + (opCost * typeMult * execMult)
}

// operatorCost returns base cost for operator execution.
// Maps operator to canonical cost constant per doc/05-performance/cost-model.md.
func operatorCost(op Operator) int {
	switch op {
	case OpExists, OpIsNull:
		return CostExists
	case OpEq, OpNeq:
		return CostEq
	case OpLt, OpLte, OpGt, OpGte:
		return CostLt
	case OpIn:
		return CostIn
	case OpPrefix, OpSuffix:
		return CostPrefix
	default:
		return CostEq
	}
}

// typeMultiplier returns cost multiplier based on field type complexity.
// String/Any types more expensive than numeric/boolean due to comparison overhead.
func typeMultiplier(ft FieldType) int {
	switch ft {
	case FieldTypeNumeric:
		return MultiplierFloat // conservative: assume float
	case FieldTypeBoolean:
		return MultiplierBool
	case FieldTypeText:
		return MultiplierString
	case FieldTypeAny:
		return MultiplierAny
	default:
		return MultiplierAny
	}
}
