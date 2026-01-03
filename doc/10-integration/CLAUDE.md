# Integration Guide for LLM Agents

## Purpose

Go module architecture with internal/ package organization, monorepo structure,
and clean separation between client (SDK) and server concerns.

## Hub

**`README.md`** - Read when understanding integration strategy, Go module
architecture, or package organization patterns

## Files

**`monorepo-structure.md`** - Read when understanding repository layout (go.mod,
internal/, cmd/, sdks/), directory organization, or build architecture with go
build

**`package-separation.md`** - Read when understanding package boundaries
(internal/types, internal/protobuf, internal/rules), implementing clean package
separation between SDK and server, or managing internal/ package visibility

**`dependency-management.md`** - Read when understanding Go/Python/Java
versioning policies, vendoring strategy, or dependency upgrade decisions
