# Rule Engine Architecture Guide for LLM Agents

## Purpose

Rule expression evaluation system including DNF schema, field path resolution, type coercion, schema evolution handling, and operational lifecycle controls for sub-millisecond evaluation.

## Hub

**`README.md`** - Read when understanding rule engine philosophy, DNF expression structure, validation domain boundaries, or overall rule evaluation strategy

## Files

**`expression-language.md`** - Read when implementing DNF schema, understanding condition structure, operator semantics, or rule compilation logic

**`field-path-resolution.md`** - Read when implementing field path resolution, understanding wildcard semantics, nested path handling, or cross-field comparisons

**`type-system-coercion.md`** - Read when implementing type coercion, understanding field type multipliers, null value semantics, or coercion failure handling

**`schema-evolution.md`** - Read when implementing missing field policies, understanding on_missing_field modes, or handling schema drift in incoming data

**`lifecycle.md`** - Read when implementing dry-run mode, emergency pause, enable/disable controls, or understanding rule propagation timing
