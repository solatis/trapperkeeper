#!/usr/bin/env python3
"""
Markdown AST Parser and Section Indexer

This module provides markdown document parsing, section extraction, and content
analysis using markdown-it-py for AST generation.

Key Features:
- Parse markdown to AST using markdown-it-py
- Extract all H2+ headings with content
- Count paragraphs and sentences in sections
- Extract list items for pattern matching
- Cache parsed AST for performance

Performance Target: <100ms to parse 1000-line document
"""

import re
from typing import Dict, List, Optional, Any
from dataclasses import dataclass
from markdown_it import MarkdownIt
from markdown_it.token import Token


@dataclass
class SectionInfo:
    """
    Information about a markdown section.

    Attributes:
        name: Section heading text
        level: Heading level (2 for H2, 3 for H3, etc.)
        content: Raw text content of the section
        line_start: Starting line number (0-based)
        line_end: Ending line number (0-based)
        tokens: AST tokens for this section
    """
    name: str
    level: int
    content: str
    line_start: int
    line_end: int
    tokens: List[Token]


class MarkdownParser:
    """
    Markdown document parser with AST caching.

    Provides efficient parsing and section extraction with caching to avoid
    re-parsing the same document multiple times.
    """

    def __init__(self):
        """Initialize parser with markdown-it-py instance."""
        self._md = MarkdownIt()
        self._cache: Dict[str, List[Token]] = {}

    def parse_markdown(self, text: str, cache_key: Optional[str] = None) -> List[Token]:
        """
        Parse markdown text to AST tokens.

        Args:
            text: Markdown text to parse
            cache_key: Optional key for caching parsed AST

        Returns:
            List of markdown-it-py tokens representing the AST

        Example:
            >>> parser = MarkdownParser()
            >>> tokens = parser.parse_markdown("# Title\\n\\nParagraph text.")
            >>> len(tokens) > 0
            True
        """
        if cache_key and cache_key in self._cache:
            return self._cache[cache_key]

        tokens = self._md.parse(text)

        if cache_key:
            self._cache[cache_key] = tokens

        return tokens

    def clear_cache(self):
        """Clear the AST cache."""
        self._cache.clear()


# Global parser instance for module-level functions
_parser = MarkdownParser()


def parse_markdown(text: str, cache_key: Optional[str] = None) -> List[Token]:
    """
    Parse markdown text to AST tokens (module-level function).

    Args:
        text: Markdown text to parse
        cache_key: Optional key for caching parsed AST

    Returns:
        List of markdown-it-py tokens representing the AST

    Example:
        >>> tokens = parse_markdown("# Title\\n\\nParagraph text.")
        >>> len(tokens) > 0
        True
    """
    return _parser.parse_markdown(text, cache_key)


def extract_sections(tokens: List[Token], min_level: int = 2) -> Dict[str, SectionInfo]:
    """
    Extract all sections from markdown AST.

    Sections are identified by H2+ headings (configurable via min_level).
    Each section includes all content until the next heading of equal or higher level.

    Args:
        tokens: Markdown AST tokens from parse_markdown()
        min_level: Minimum heading level to extract (default: 2 for H2+)

    Returns:
        Dictionary mapping section names to SectionInfo objects

    Example:
        >>> text = "## Section One\\n\\nParagraph.\\n\\n## Section Two\\n\\nMore text."
        >>> tokens = parse_markdown(text)
        >>> sections = extract_sections(tokens)
        >>> "Section One" in sections
        True
        >>> "Section Two" in sections
        True
    """
    sections: Dict[str, SectionInfo] = {}
    current_section: Optional[str] = None
    current_level: Optional[int] = None
    current_tokens: List[Token] = []
    current_line_start: Optional[int] = None

    in_heading = False
    pending_heading_level = None

    for token in tokens:
        # Check if this is a heading token
        if token.type == "heading_open":
            # Extract heading level (h2 -> 2, h3 -> 3, etc.)
            level = int(token.tag[1])

            if level >= min_level:
                # Save previous section if it exists
                if current_section is not None:
                    sections[current_section] = _create_section_info(
                        current_section,
                        current_level,
                        current_tokens,
                        current_line_start,
                        token.map[0] - 1 if token.map else 0
                    )

                # Start new section
                in_heading = True
                pending_heading_level = level
                current_line_start = token.map[0] if token.map else 0
                current_section = None  # Will be set by next inline token
                current_tokens = []
            elif current_section is not None:
                # Heading is higher level (H1), close current section
                sections[current_section] = _create_section_info(
                    current_section,
                    current_level,
                    current_tokens,
                    current_line_start,
                    token.map[0] - 1 if token.map else 0
                )
                current_section = None
                current_tokens = []

        elif token.type == "inline" and in_heading:
            # This is the heading text
            current_section = token.content
            current_level = pending_heading_level
            in_heading = False
            pending_heading_level = None

        elif token.type == "heading_close":
            # End of heading, continue
            pass

        elif current_section is not None:
            # Add token to current section
            current_tokens.append(token)

    # Save final section if exists
    if current_section is not None:
        # Use large line number for end if no map available
        end_line = current_tokens[-1].map[1] if current_tokens and current_tokens[-1].map else 999999
        sections[current_section] = _create_section_info(
            current_section,
            current_level,
            current_tokens,
            current_line_start,
            end_line
        )

    return sections


def _create_section_info(
    name: str,
    level: int,
    tokens: List[Token],
    line_start: int,
    line_end: int
) -> SectionInfo:
    """
    Create SectionInfo from tokens.

    Args:
        name: Section heading text
        level: Heading level
        tokens: Section content tokens
        line_start: Starting line number
        line_end: Ending line number

    Returns:
        SectionInfo object
    """
    # Extract text content from tokens
    content_parts = []
    for token in tokens:
        if token.type == "inline":
            content_parts.append(token.content)
        elif token.type in ("fence", "code_block"):
            content_parts.append(token.content)

    content = "\n".join(content_parts)

    return SectionInfo(
        name=name,
        level=level,
        content=content,
        line_start=line_start,
        line_end=line_end,
        tokens=tokens
    )


def get_section(tokens: List[Token], name: str) -> Optional[SectionInfo]:
    """
    Retrieve section by name.

    Args:
        tokens: Markdown AST tokens
        name: Section name to search for (case-sensitive)

    Returns:
        SectionInfo if found, None otherwise

    Example:
        >>> text = "## Files\\n\\nSome content."
        >>> tokens = parse_markdown(text)
        >>> section = get_section(tokens, "Files")
        >>> section.name
        'Files'
    """
    sections = extract_sections(tokens)
    return sections.get(name)


def count_paragraphs(section: SectionInfo) -> int:
    """
    Count paragraph nodes in a section.

    Args:
        section: SectionInfo object

    Returns:
        Number of paragraph nodes

    Example:
        >>> text = "## Test\\n\\nPara 1.\\n\\nPara 2.\\n\\nPara 3."
        >>> tokens = parse_markdown(text)
        >>> section = get_section(tokens, "Test")
        >>> count_paragraphs(section)
        3
    """
    count = 0
    for token in section.tokens:
        # Count only non-hidden paragraphs (excludes list item paragraphs)
        if token.type == "paragraph_open" and not token.hidden:
            count += 1
    return count


def count_sentences(section: SectionInfo) -> int:
    """
    Count sentences in a section.

    Handles common edge cases:
    - Abbreviations (Dr., Mr., etc.)
    - Decimal numbers (3.14)
    - Quoted text
    - Multiple punctuation (?!, !?)

    Args:
        section: SectionInfo object

    Returns:
        Number of sentences

    Example:
        >>> text = "## Test\\n\\nFirst sentence. Second sentence! Third?"
        >>> tokens = parse_markdown(text)
        >>> section = get_section(tokens, "Test")
        >>> count_sentences(section)
        3
    """
    text = section.content

    # Split on sentence boundaries: . ! ?
    # But handle common abbreviations and edge cases

    # Replace common abbreviations to avoid false splits
    # Note: Need to escape the period in the pattern but not in the replacement
    text = re.sub(r'\b(Dr|Mr|Mrs|Ms|Prof|Sr|Jr|vs|etc|e\.g|i\.e)\.', r'\1<PERIOD>', text, flags=re.IGNORECASE)

    # Replace decimal numbers to avoid false splits
    text = re.sub(r'(\d)\.(\d)', r'\1<PERIOD>\2', text)

    # Split on sentence-ending punctuation followed by whitespace or end of string
    sentences = re.split(r'[.!?]+(?:\s+|$)', text)

    # Filter out empty strings and whitespace-only strings
    sentences = [s.strip() for s in sentences if s.strip()]

    return len(sentences)


def extract_list_items(section: SectionInfo) -> List[str]:
    """
    Extract list items from a section.

    Extracts both bulleted and numbered list items for pattern matching.

    Args:
        section: SectionInfo object

    Returns:
        List of item text strings

    Example:
        >>> text = "## Test\\n\\n- Item 1\\n- Item 2\\n- Item 3"
        >>> tokens = parse_markdown(text)
        >>> section = get_section(tokens, "Test")
        >>> items = extract_list_items(section)
        >>> len(items)
        3
        >>> "Item 1" in items
        True
    """
    items = []
    in_list_item = False

    for token in section.tokens:
        if token.type == "list_item_open":
            in_list_item = True
        elif token.type == "list_item_close":
            in_list_item = False
        elif token.type == "inline" and in_list_item:
            items.append(token.content)

    return items


def section_matches_pattern(section: SectionInfo, pattern: str) -> bool:
    """
    Check if section content matches a regex pattern.

    Args:
        section: SectionInfo object
        pattern: Regular expression pattern

    Returns:
        True if pattern matches section content, False otherwise

    Example:
        >>> text = "## Test\\n\\nThis contains foo and bar."
        >>> tokens = parse_markdown(text)
        >>> section = get_section(tokens, "Test")
        >>> section_matches_pattern(section, r"foo.*bar")
        True
        >>> section_matches_pattern(section, r"baz")
        False
    """
    return re.search(pattern, section.content) is not None


def section_present(tokens: List[Token], name: str) -> bool:
    """
    Check if a section exists in the document.

    This is the main integration function used by predicates.section_present().

    Args:
        tokens: Markdown AST tokens
        name: Section name to search for (case-sensitive)

    Returns:
        True if section exists, False otherwise

    Example:
        >>> text = "## Files\\n\\nContent.\\n\\n## Summary\\n\\nMore content."
        >>> tokens = parse_markdown(text)
        >>> section_present(tokens, "Files")
        True
        >>> section_present(tokens, "Missing")
        False
    """
    sections = extract_sections(tokens)
    return name in sections


if __name__ == '__main__':
    # Example usage
    import sys

    if len(sys.argv) < 2:
        print("Usage: markdown_parser.py <markdown_file>")
        print("\nParses markdown file and displays section information.")
        sys.exit(1)

    file_path = sys.argv[1]

    with open(file_path, 'r') as f:
        text = f.read()

    tokens = parse_markdown(text)
    sections = extract_sections(tokens)

    print(f"Found {len(sections)} sections:\n")

    for name, info in sections.items():
        print(f"Section: {name}")
        print(f"  Level: H{info.level}")
        print(f"  Lines: {info.line_start}-{info.line_end}")
        print(f"  Paragraphs: {count_paragraphs(info)}")
        print(f"  Sentences: {count_sentences(info)}")
        print(f"  List items: {len(extract_list_items(info))}")
        print()
