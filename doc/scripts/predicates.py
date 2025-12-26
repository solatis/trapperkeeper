#!/usr/bin/env python3
"""
Predicate Engine for Conditional Validation Rules

This module provides predicate evaluation capabilities for checking filesystem
and document state to enable conditional validation rules like:
"require Files section if .md files exist"

Supported Predicates:
- file_exists(path): Check if file exists relative to document directory
- md_files_exist(exclude=[]): Check for markdown files excluding specified files
- section_present(name): Check if section exists in target document (requires AST)
- subdirs_exist(): Check if subdirectories exist in document directory
"""

import re
from pathlib import Path
from typing import Any, Optional, List


class PredicateContext:
    """
    Context object for predicate evaluation containing document metadata.

    Attributes:
        doc_path: Path to the document being validated
        doc_dir: Directory containing the document
        doc_ast: Optional document AST for section_present predicate (M3 dependency)
    """

    def __init__(self, doc_path: str | Path, doc_ast: Optional[Any] = None):
        """
        Initialize predicate context.

        Args:
            doc_path: Absolute or relative path to document
            doc_ast: Optional document AST structure for section queries
        """
        self.doc_path = Path(doc_path)
        self.doc_dir = self.doc_path.parent
        self.doc_ast = doc_ast


class PredicateError(Exception):
    """Base exception for predicate evaluation errors."""
    pass


class PredicateParseError(PredicateError):
    """Exception raised when predicate expression cannot be parsed."""
    pass


class PredicateEvaluationError(PredicateError):
    """Exception raised when predicate evaluation fails."""
    pass


# Predicate function implementations

def file_exists(path: str, ctx: PredicateContext) -> bool:
    """
    Check if a file exists relative to document directory.

    Args:
        path: Relative file path to check
        ctx: Predicate context with document location

    Returns:
        True if file exists, False otherwise

    Example:
        >>> ctx = PredicateContext("/path/to/doc/CLAUDE.md")
        >>> file_exists("README.md", ctx)
        True
    """
    target_path = ctx.doc_dir / path
    return target_path.is_file()


def md_files_exist(exclude: Optional[List[str]] = None, ctx: Optional[PredicateContext] = None) -> bool:
    """
    Check if markdown files exist in document directory, excluding specified files.

    Args:
        exclude: Optional list of filenames to exclude from check
        ctx: Predicate context with document location

    Returns:
        True if any .md files found (after exclusions), False otherwise

    Example:
        >>> ctx = PredicateContext("/path/to/doc/CLAUDE.md")
        >>> md_files_exist(exclude=["README.md", "CLAUDE.md"], ctx=ctx)
        True
    """
    if ctx is None:
        raise PredicateEvaluationError("md_files_exist requires context parameter")

    exclude_set = set(exclude or [])

    # Find all .md files in document directory
    md_files = [f for f in ctx.doc_dir.glob("*.md") if f.is_file()]

    # Filter out excluded files
    remaining = [f for f in md_files if f.name not in exclude_set]

    return len(remaining) > 0


def section_present(name: str, ctx: PredicateContext) -> bool:
    """
    Check if a section exists in the target document.

    Uses markdown_parser module to parse document and check for section presence.

    Args:
        name: Section name to search for (case-sensitive)
        ctx: Predicate context with document AST

    Returns:
        True if section exists, False otherwise

    Raises:
        PredicateEvaluationError: If doc_ast not provided in context

    Example:
        >>> ctx = PredicateContext("/path/to/doc/CLAUDE.md", doc_ast=parsed_ast)
        >>> section_present("Files", ctx)
        True
    """
    if ctx.doc_ast is None:
        raise PredicateEvaluationError(
            "section_present requires doc_ast in context"
        )

    # Use markdown_parser to check section presence
    try:
        from markdown_parser import section_present as mp_section_present
        return mp_section_present(ctx.doc_ast, name)
    except ImportError:
        raise PredicateEvaluationError(
            "markdown_parser module not available - install markdown-it-py"
        )
    except Exception as e:
        raise PredicateEvaluationError(
            f"Failed to check section presence: {e}"
        )


def subdirs_exist(ctx: PredicateContext) -> bool:
    """
    Check if subdirectories exist in document directory.

    Args:
        ctx: Predicate context with document location

    Returns:
        True if any subdirectories exist, False otherwise

    Example:
        >>> ctx = PredicateContext("/path/to/doc/CLAUDE.md")
        >>> subdirs_exist(ctx)
        True
    """
    for item in ctx.doc_dir.iterdir():
        if item.is_dir():
            return True
    return False


# Predicate registry mapping function names to implementations
PREDICATES = {
    'file_exists': file_exists,
    'md_files_exist': md_files_exist,
    'section_present': section_present,
    'subdirs_exist': subdirs_exist,
}


def parse_predicate_expression(expression: str) -> tuple[str, dict[str, Any]]:
    """
    Parse a predicate expression string into function name and arguments.

    Supports:
    - Simple function calls: func_name("arg")
    - Multiple arguments: func_name("arg1", "arg2")
    - Keyword arguments: func_name(exclude=["file1.md", "file2.md"])

    Args:
        expression: Predicate expression string

    Returns:
        Tuple of (function_name, kwargs_dict)

    Raises:
        PredicateParseError: If expression is malformed

    Example:
        >>> parse_predicate_expression('file_exists("README.md")')
        ('file_exists', {'path': 'README.md'})
        >>> parse_predicate_expression('md_files_exist(exclude=["a.md", "b.md"])')
        ('md_files_exist', {'exclude': ['a.md', 'b.md']})
    """
    expression = expression.strip()

    # Match function call pattern: func_name(args)
    match = re.match(r'^(\w+)\((.*)\)$', expression)
    if not match:
        raise PredicateParseError(
            f"Invalid predicate expression format: {expression}. "
            "Expected format: function_name(args)"
        )

    func_name = match.group(1)
    args_str = match.group(2).strip()

    if func_name not in PREDICATES:
        raise PredicateParseError(
            f"Unknown predicate function: {func_name}. "
            f"Available predicates: {', '.join(PREDICATES.keys())}"
        )

    # Parse arguments
    kwargs = {}

    if not args_str:
        # No arguments case (e.g., subdirs_exist())
        return func_name, kwargs

    # Check if this is a keyword argument pattern
    if '=' in args_str:
        # Parse keyword arguments
        kwargs = _parse_keyword_args(args_str, expression)
    else:
        # Parse positional arguments
        kwargs = _parse_positional_args(func_name, args_str, expression)

    return func_name, kwargs


def _parse_keyword_args(args_str: str, original_expr: str) -> dict[str, Any]:
    """Parse keyword argument format: key=value."""
    kwargs = {}

    # Match keyword=value patterns
    # Supports: key="value", key=["item1", "item2"]
    kwarg_pattern = r'(\w+)=(\[[^\]]*\]|"[^"]*")'

    matches = re.finditer(kwarg_pattern, args_str)
    found_any = False

    for match in matches:
        found_any = True
        key = match.group(1)
        value_str = match.group(2)

        # Parse the value
        value = _parse_value(value_str, original_expr)
        kwargs[key] = value

    if not found_any:
        raise PredicateParseError(
            f"Failed to parse keyword arguments in: {original_expr}"
        )

    return kwargs


def _parse_positional_args(func_name: str, args_str: str, original_expr: str) -> dict[str, Any]:
    """Parse positional arguments and map to parameter names."""
    # For positional args, we need to know the parameter name
    # Map function names to their first positional parameter
    positional_params = {
        'file_exists': 'path',
        'section_present': 'name',
    }

    if func_name not in positional_params:
        raise PredicateParseError(
            f"Function {func_name} does not accept positional arguments. "
            f"Use keyword arguments instead."
        )

    param_name = positional_params[func_name]

    # Parse the value (should be a quoted string)
    value = _parse_value(args_str, original_expr)

    return {param_name: value}


def _parse_value(value_str: str, original_expr: str) -> Any:
    """
    Parse a value string into appropriate Python type.

    Supports:
    - Quoted strings: "value"
    - Lists: ["item1", "item2"]
    """
    value_str = value_str.strip()

    # List pattern
    if value_str.startswith('[') and value_str.endswith(']'):
        return _parse_list(value_str, original_expr)

    # String pattern
    if value_str.startswith('"') and value_str.endswith('"'):
        return value_str[1:-1]  # Remove quotes

    raise PredicateParseError(
        f"Invalid value format in: {original_expr}. "
        f"Expected quoted string or list, got: {value_str}"
    )


def _parse_list(list_str: str, original_expr: str) -> List[str]:
    """Parse a list literal: ["item1", "item2"]."""
    # Remove brackets
    inner = list_str[1:-1].strip()

    if not inner:
        return []

    # Match quoted strings
    items = []
    pattern = r'"([^"]*)"'

    for match in re.finditer(pattern, inner):
        items.append(match.group(1))

    return items


def evaluate_predicate(expression: str, ctx: PredicateContext) -> bool:
    """
    Evaluate a predicate expression with given context.

    Args:
        expression: Predicate expression string to evaluate
        ctx: Context containing document path and metadata

    Returns:
        Boolean result of predicate evaluation

    Raises:
        PredicateParseError: If expression is malformed
        PredicateEvaluationError: If evaluation fails

    Example:
        >>> ctx = PredicateContext("/path/to/doc/CLAUDE.md")
        >>> evaluate_predicate('file_exists("README.md")', ctx)
        True
        >>> evaluate_predicate('md_files_exist(exclude=["CLAUDE.md"])', ctx)
        False
    """
    try:
        func_name, kwargs = parse_predicate_expression(expression)
        predicate_func = PREDICATES[func_name]

        # Add context to kwargs
        kwargs['ctx'] = ctx

        # Call predicate function
        result = predicate_func(**kwargs)

        if not isinstance(result, bool):
            raise PredicateEvaluationError(
                f"Predicate {func_name} returned non-boolean value: {result}"
            )

        return result

    except PredicateParseError:
        raise
    except PredicateEvaluationError:
        raise
    except TypeError as e:
        raise PredicateEvaluationError(
            f"Invalid arguments for predicate {func_name}: {e}"
        )
    except Exception as e:
        raise PredicateEvaluationError(
            f"Failed to evaluate predicate '{expression}': {e}"
        )


if __name__ == '__main__':
    # Example usage
    import sys

    if len(sys.argv) < 3:
        print("Usage: predicates.py <doc_path> <predicate_expression>")
        print("\nExamples:")
        print('  predicates.py /path/to/CLAUDE.md \'file_exists("README.md")\'')
        print('  predicates.py /path/to/CLAUDE.md \'md_files_exist(exclude=["CLAUDE.md"])\'')
        print('  predicates.py /path/to/CLAUDE.md \'subdirs_exist()\'')
        sys.exit(1)

    doc_path = sys.argv[1]
    expression = sys.argv[2]

    try:
        ctx = PredicateContext(doc_path)
        result = evaluate_predicate(expression, ctx)
        print(f"Result: {result}")
        sys.exit(0 if result else 1)
    except PredicateError as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(2)
