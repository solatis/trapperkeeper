// internal/rules/fieldpath.go
package rules

import (
	"encoding/json"
	"sort"

	"github.com/solatis/trapperkeeper/internal/types"
)

/*
 * Field path resolution for JSON payloads.
 *
 * Resolves arbitrary paths through nested objects and arrays with wildcard
 * support. Implements ANY semantics for wildcards (first match wins) per
 * doc/04-rule-engine/field-path-resolution.md. Enforces MaxPathDepth (16)
 * and MaxNestedWildcards (2) at resolution time.
 *
 * Key functions:
 *   - Resolve: Traverses JSON following PathSegment chain
 *   - resolveRecursive: Internal recursive traversal
 *
 * Wildcard semantics: Returns first matching element, not all matches.
 * This enables short-circuit optimization while maintaining correctness.
 *
 * Performance: Wildcard on object requires sorted key iteration for
 * deterministic order (Decision Log: evaluation order stability).
 */

// ResolveResult contains the resolved value and the actual path taken.
type ResolveResult struct {
	Value        any                 // resolved value (nil if not found)
	ResolvedPath []types.PathSegment // path with wildcards replaced by actual indices
	Found        bool                // true if path resolved to a value
}

// Resolve traverses data following path segments.
// Returns ErrPathTooDeep if path exceeds MaxPathDepth.
// Returns ErrTooManyWildcards if path contains > MaxNestedWildcards wildcards.
// Returns ErrFieldNotFound if path does not exist in data.
func Resolve(path []types.PathSegment, data json.RawMessage) (ResolveResult, error) {
	if len(path) > types.MaxPathDepth {
		return ResolveResult{}, types.ErrPathTooDeep
	}

	wildcardCount := 0
	for _, seg := range path {
		if seg.Wildcard {
			wildcardCount++
		}
	}
	if wildcardCount > types.MaxNestedWildcards {
		return ResolveResult{}, types.ErrTooManyWildcards
	}

	var parsed any
	if err := json.Unmarshal(data, &parsed); err != nil {
		return ResolveResult{}, err
	}

	return resolveRecursive(path, parsed, nil)
}

// resolveRecursive traverses nested JSON structures following path segments.
// Returns first match for wildcards (ANY semantics). Accumulates resolved path
// with actual indices/keys replacing wildcards for match diagnostics.
func resolveRecursive(path []types.PathSegment, current any, resolvedSoFar []types.PathSegment) (ResolveResult, error) {
	if len(path) == 0 {
		return ResolveResult{
			Value:        current,
			ResolvedPath: resolvedSoFar,
			Found:        true,
		}, nil
	}

	seg := path[0]
	remaining := path[1:]

	switch v := current.(type) {
	case map[string]any:
		if seg.Wildcard {
			// Sort keys for deterministic iteration order (stable evaluation invariant)
			keys := make([]string, 0, len(v))
			for k := range v {
				keys = append(keys, k)
			}
			sort.Strings(keys)
			for _, key := range keys {
				val := v[key]
				resolved := append(resolvedSoFar, types.PathSegment{Key: key})
				result, err := resolveRecursive(remaining, val, resolved)
				if err == nil && result.Found {
					return result, nil
				}
			}
			return ResolveResult{}, types.ErrFieldNotFound
		}
		if seg.IsIndex {
			// Cannot index into object with integer
			return ResolveResult{}, types.ErrFieldNotFound
		}
		val, ok := v[seg.Key]
		if !ok {
			return ResolveResult{}, types.ErrFieldNotFound
		}
		return resolveRecursive(remaining, val, append(resolvedSoFar, seg))

	case []any:
		if seg.Wildcard {
			if len(v) == 0 {
				// Empty array: all elements missing, defer to on_missing_field
				return ResolveResult{}, types.ErrFieldNotFound
			}
			// ANY semantics: return first match (short-circuit optimization)
			for i, elem := range v {
				resolved := append(resolvedSoFar, types.PathSegment{Index: i, IsIndex: true})
				result, err := resolveRecursive(remaining, elem, resolved)
				if err == nil && result.Found {
					return result, nil
				}
			}
			return ResolveResult{}, types.ErrFieldNotFound
		}
		if !seg.IsIndex {
			// Cannot use string key on array
			return ResolveResult{}, types.ErrFieldNotFound
		}
		if seg.Index < 0 || seg.Index >= len(v) {
			return ResolveResult{}, types.ErrFieldNotFound
		}
		return resolveRecursive(remaining, v[seg.Index], append(resolvedSoFar, seg))

	case nil:
		// Null value at intermediate position
		return ResolveResult{}, types.ErrFieldNotFound

	default:
		// Scalar value but path continues
		return ResolveResult{}, types.ErrFieldNotFound
	}
}
