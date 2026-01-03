package types

import "errors"

// Sentinel errors for TrapperKeeper operations.
var (
	// ErrPayloadTooLarge indicates the event payload exceeds MaxPayloadSize.
	ErrPayloadTooLarge = errors.New("payload exceeds maximum size")

	// ErrMetadataTooLarge indicates metadata exceeds size limits.
	ErrMetadataTooLarge = errors.New("metadata exceeds maximum size")

	// ErrTooManyMetadataPairs indicates too many metadata key-value pairs.
	ErrTooManyMetadataPairs = errors.New("too many metadata pairs")

	// ErrMetadataKeyTooLong indicates a metadata key exceeds MaxMetadataKeyLength.
	ErrMetadataKeyTooLong = errors.New("metadata key too long")

	// ErrMetadataValueTooLong indicates a metadata value exceeds MaxMetadataValueLength.
	ErrMetadataValueTooLong = errors.New("metadata value too long")

	// ErrReservedMetadataKey indicates use of reserved $tk.* namespace.
	ErrReservedMetadataKey = errors.New("metadata key uses reserved $tk.* namespace")

	// ErrPathTooDeep indicates a field path exceeds MaxPathDepth.
	ErrPathTooDeep = errors.New("field path exceeds maximum depth")

	// ErrTooManyWildcards indicates a field path exceeds MaxNestedWildcards.
	ErrTooManyWildcards = errors.New("field path has too many wildcards")

	// ErrWildcardInFieldRef indicates a wildcard in a field_ref path.
	ErrWildcardInFieldRef = errors.New("wildcards not allowed in field_ref")

	// ErrTooManyInValues indicates an IN operator exceeds MaxInOperatorValues.
	ErrTooManyInValues = errors.New("IN operator has too many values")

	// ErrEmptyExpression indicates a rule has no conditions.
	ErrEmptyExpression = errors.New("rule expression is empty")

	// ErrInvalidOperator indicates an unknown or incompatible operator.
	ErrInvalidOperator = errors.New("invalid operator for field type")

	// ErrCoercionFailed indicates type coercion failed.
	ErrCoercionFailed = errors.New("type coercion failed")

	// ErrFieldNotFound indicates a field path could not be resolved.
	ErrFieldNotFound = errors.New("field not found")
)
