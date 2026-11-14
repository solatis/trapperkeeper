#!/usr/bin/env python3
"""Check that the step numbers of walkthrough comments match their back-references.

A walkthrough is a comment block at the top of a function body that lists the
steps of the function as "1. ...", "2. ...", each in full sentences. Each step
has a one-line back-reference such as "// 2. run the query" at the first line
of its code. A note may follow the back-reference on the lines below it. The
check reads line comments, knows how each language writes strings and
comments, and finds the scope of a walkthrough from the indentation of
formatted code. It knows no grammar.

The check takes files and directories, the files that a git range changed, or
with --words the words of a person passed on unchanged. The words before the
first word that ends with a colon, such as "focus:", are the scope, and the
check ignores the rest. In the scope, a word that names an existing file or
directory is a path, a word BASE...REV is a git range, and the check ignores
any other word. It also ignores a range that git does not know and says so.

Accepted limits:
- Block comments (/* */, {- -}, --[[ ]]) are not checked.
- Code that is not formatted gives wrong scopes.
- In the brace, Lisp, Haskell and Erlang families a comment at column 0
  documents the file or an item, so a list at column 0 is not seen.
- A walkthrough directly above a nested Python def is read as a comment on
  the def.
- The check cannot tell whether a back-reference sits at the right line,
  whether a function that needs a walkthrough has one, or whether the text
  is true.
- Numbered items written as "1)" or with letters are not read.
- Lua long strings and strings that span lines in Lisp and Erlang are not
  tracked.
"""

import argparse
import os
import re
import subprocess
import sys
from collections import namedtuple

# ---------------------------------------------------------------------------
# Languages
# ---------------------------------------------------------------------------

# The strings that may span lines, as named regex groups. The group name says
# how the string closes.
LONG_STRINGS = {
    # Python, Java and TOML triple quotes, such as """text""".
    "triple": r"(?P<triple>\"\"\"|''')",
    # A C++ raw string opener such as R"sql( or u8R"(. The delimiter is at most
    # 16 characters, as the standard allows.
    "cpp-raw": r"(?P<cpp_raw>(?<!\w)(?:u8|[uUL])?R\"(?P<delim>[^()\\\s\"]{0,16})\()",
    # A Rust raw string opener such as r"  r#"  br##" or cr#".
    "rust-raw": r"(?P<rust_raw>(?<!\w)[bc]?r(?P<hashes>#*)\")",
    # A Go raw string or a JavaScript template string, `text`.
    "backtick": r"(?P<backtick>`)",
    # A shell heredoc opener such as <<EOF, <<-'END' or <<~"SQL". The tag must
    # be upper case, so that a << operator followed by a name opens nothing.
    "heredoc": r"(?P<heredoc>(?<!<)<<(?!<)[-~]?\s*(?P<hq>['\"]?)(?P<tag>[A-Z_][A-Z0-9_]*)(?P=hq))",
}

# A char literal in Rust, Go, the C family and Haskell: 'a', '"', '\'', '\n',
# '\u{1F600}'. A Rust lifetime such as 'a has no closing quote and stays code.
QUOTED_CHAR = r"'(?:\\.[^']*|[^\\'])'"
# A char literal or escaped character written with a backslash: \" in Clojure
# or in shell.
ESCAPED_CHAR = r"\\."
# An Erlang char literal: $" or $\n.
ERLANG_CHAR = r"\$\\?."
# A regex that matches no text, for a language without doc comments.
NEVER = r"(?!)"

# name: the language key. tokens: the regex that finds the next comment,
#   string or char literal on a line, built by make_lang.
# doc: a regex on the text after the comment marker that makes the line a doc
#   comment.
# decl: a regex on the line below a comment block that makes the block a
#   comment on a declaration, "c" for the C family test, or None.
# top_level_code: True where code runs at column 0, so a list at column 0 can
#   be a walkthrough. Elsewhere a comment at column 0 documents an item.
# block_close: the text that closes a block comment.
# multiline: True where a plain "..." string may span lines.
# outdented: a regex on a whole code line that the formatter outdents and that
#   therefore says nothing about scope, or None.
Lang = namedtuple(
    "Lang", "name tokens doc decl top_level_code block_close multiline outdented")


def make_lang(name, marker, doc, quotes, char=None, long_strings=(), block=None,
              decl=None, top_level_code=False, multiline=False, outdented=None):
    """Return the Lang of one language from its comment and string syntax."""
    parts = ["(?P<block>%s)" % re.escape(block[0])] if block else []
    parts.append("(?P<comment>%s)" % marker)
    parts.extend(LONG_STRINGS[k] for k in long_strings)
    if char:
        parts.append("(?P<char>%s)" % char)
    if quotes:
        parts.append("(?P<quote>[%s])" % quotes)
    # The leftmost token wins, and at one position the first alternative wins,
    # so a block opener goes before the line marker and triple quotes before ".
    return Lang(name, re.compile("|".join(parts)), re.compile(doc),
                re.compile(decl) if decl not in (None, "c") else decl,
                top_level_code, block[1] if block else None, multiline,
                re.compile(outdented) if outdented else None)


# A label alone on its line, such as "outer:" or "public:". gofmt and
# clang-format outdent it below the indentation of the body.
LABEL = r"[A-Za-z_]\w*\s*:"
# A C preprocessor line, such as "#ifdef X", which clang-format puts at column 0.
PREPROCESSOR = r"#.*"
# A shell or YAML "#" starts a comment only at the start of a word, which keeps
# "$#" and "a#b" as code.
WORD_HASH = r"(?<!\S)#+"
# A Haskell "--" followed by another symbol character is an operator such
# as "-->".
HASKELL_DASHES = r"--(?![!#$%&*+./<=>?@\\^|~:])"

# The C family test for a declaration below a comment block, used by
# is_doc_block: a keyword that starts a declaration, or an attribute.
C_DECL = re.compile(
    r"(template\s*<|class\b|struct\b|enum\b|union\b|namespace\b|interface\b"
    r"|typedef\b|public\b|private\b|protected\b|internal\b|virtual\b|inline\b"
    r"|explicit\b|export\b|function\b|fun\b|func\b|override\b|@|\[\[)"
)
# A function declaration such as `int Foo::bar(int x) const {`: a type, a name
# and a "(". The lookahead rejects statements that start with a keyword.
C_FUNCTION = re.compile(
    r"(?!(if|for|while|switch|return|else|do|case|throw|delete|new|goto"
    r"|co_return|co_await|co_yield)\b)[\w:<>,*&~\s]+?[\s*&]+[\w:~]+\s*\("
)

# C and C++ sources and headers share one syntax.
C_FAMILY = dict(marker=r"//", doc=r"[/!]", quotes='"', char=QUOTED_CHAR,
                long_strings=("cpp-raw",), block=("/*", "*/"), decl="c",
                multiline=True, outdented=PREPROCESSOR + "|" + LABEL)

# The doc regex [/!] matches the third character of "///" and "//!". For
# Python and shell the doc regex "!" makes a "#!" line a doc comment.
LANGS = {
    "rust": make_lang("rust", r"//", r"[/!]", '"', QUOTED_CHAR, ("rust-raw",),
                      ("/*", "*/"), multiline=True),
    "go": make_lang("go", r"//", r"[/!]", '"', QUOTED_CHAR, ("backtick",),
                    ("/*", "*/"), outdented=LABEL),
    "c": make_lang("c", **C_FAMILY),
    "c-header": make_lang("c-header", **C_FAMILY),
    "java": make_lang("java", r"//", r"[/!]", '"', QUOTED_CHAR, ("triple",),
                      ("/*", "*/"), decl="c", outdented=LABEL),
    "js": make_lang("js", r"//", r"[/!]", "\"'", None, ("backtick",),
                    ("/*", "*/"), decl="c", outdented=LABEL),
    # The decl regex matches a def, a class or a decorator.
    "python": make_lang("python", r"#+", r"!", "\"'", None, ("triple",),
                        decl=r"(async\s+def|def|class)\b|@", top_level_code=True),
    "shell": make_lang("shell", WORD_HASH, r"!", "\"'", ESCAPED_CHAR, ("heredoc",),
                       top_level_code=True),
    "config": make_lang("config", WORD_HASH, NEVER, "\"'", None, ("triple",),
                        top_level_code=True),
    # A Haskell doc comment starts with "-- |", "-- ^", "-- $", "-- *" or "---".
    "haskell": make_lang("haskell", HASKELL_DASHES, r"-|\s*[|^$*]", '"', QUOTED_CHAR,
                         block=("{-", "-}")),
    "sql": make_lang("sql", r"--", NEVER, "'\"", block=("/*", "*/"),
                     top_level_code=True),
    # A Lua "--[[" opens a block comment, and "---" starts a doc comment.
    "lua": make_lang("lua", r"--(?!\[\[)", r"-", "\"'", block=("--[[", "]]"),
                     top_level_code=True),
    "lisp": make_lang("lisp", r";+", NEVER, '"', ESCAPED_CHAR),
    # An Erlang doc comment holds an edoc tag such as "%% @doc".
    "erlang": make_lang("erlang", r"%+", r"\s*@\w+", "\"'", ERLANG_CHAR),
}

EXTENSIONS = {
    ".rs": "rust", ".go": "go",
    ".c": "c", ".h": "c-header", ".cpp": "c", ".hpp": "c-header", ".cc": "c",
    ".java": "java", ".kt": "java", ".swift": "java",
    ".js": "js", ".ts": "js",
    ".py": "python", ".sh": "shell",
    ".yml": "config", ".yaml": "config", ".toml": "config",
    ".hs": "haskell", ".sql": "sql", ".lua": "lua",
    ".clj": "lisp", ".cljs": "lisp", ".cljc": "lisp", ".edn": "lisp",
    ".el": "lisp", ".lisp": "lisp",
    ".erl": "erlang", ".hrl": "erlang",
}

# The info strings of Markdown code fences that name a known language.
FENCE_LANGS = {
    "rust": "rust", "rs": "rust", "go": "go", "golang": "go",
    "python": "python", "py": "python", "python3": "python",
    "cpp": "c", "c++": "c", "cc": "c", "hpp": "c", "c": "c", "h": "c",
    "java": "java", "kotlin": "java", "swift": "java",
    "js": "js", "javascript": "js", "ts": "js", "typescript": "js",
    "clojure": "lisp", "clj": "lisp", "lisp": "lisp", "elisp": "lisp",
    "haskell": "haskell", "hs": "haskell", "erlang": "erlang", "erl": "erlang",
    "sh": "shell", "bash": "shell", "shell": "shell", "zsh": "shell",
    "lua": "lua", "sql": "sql",
    "yaml": "config", "yml": "config", "toml": "config",
}

SKIP_DIRS = {".git", "target", "node_modules", "vendor", "thirdparty",
             "third_party", "build", "dist", ".env", "venv"}


def language_of(path):
    """Return the language key of a path from its extension, or None."""
    return EXTENSIONS.get(os.path.splitext(path)[1].lower())


# ---------------------------------------------------------------------------
# Scanning one line
# ---------------------------------------------------------------------------

# A string or block comment that is still open. kind is "block", "plain" (a
# backslash escapes the closer), "raw" or "heredoc" (the closer is a tag alone
# on its line).
Open = namedtuple("Open", "kind closer")
# has_code: the line holds code outside comments. comment: the text after the
# first line-comment marker outside strings, or None. state: the Open at the
# end of the line, or None.
Scan = namedtuple("Scan", "has_code comment state")


def find_close(o, line, at):
    """Return the index after the text that closes `o` in `line`, searching from `at`, or -1."""
    if o.kind == "heredoc":
        return len(line) if line.strip() == o.closer else -1
    if o.kind != "plain":
        i = line.find(o.closer, at)
        return -1 if i < 0 else i + len(o.closer)
    i = at
    while i < len(line):
        if line[i] == "\\":
            # A backslash escapes the next character, which cannot close the string.
            i += 2
        elif line[i] == o.closer:
            return i + 1
        else:
            i += 1
    return -1


def opened_by(lang, m):
    """Return the Open that a token match starts, or None for a char literal."""
    kind, text = m.lastgroup, m.group(0)
    if kind == "block":
        return Open("block", lang.block_close)
    if kind == "quote":
        return Open("plain", text)
    if kind in ("triple", "backtick"):
        return Open("raw", text)
    if kind == "cpp_raw":
        return Open("raw", ")" + m.group("delim") + '"')
    if kind == "rust_raw":
        return Open("raw", '"' + m.group("hashes"))
    if kind == "heredoc":
        return Open("heredoc", m.group("tag"))
    return None


def scan_line(lang, line, state):
    """Return the Scan of `line`, given the Open at its start or None."""
    # The scan moves left to right through the line:
    #
    # 1. Inside a string or block comment, it jumps past the closer, or stops
    #    when the line does not hold it.
    # 2. Outside, it jumps to the next token of the language. Any other text
    #    before the token is code.
    # 3. A line comment ends the scan, because the rest of the line is its text.
    # 4. A string or block comment that opens becomes the state. A heredoc
    #    starts on the next line, so it waits until the end of the line. A char
    #    literal opens nothing, which keeps a quote inside it from opening a
    #    string.
    # 5. At the end of the line, a plain string closes unless the language lets
    #    it span lines. This keeps a stray quote from hiding the rest of a file.
    has_code, comment, heredoc, i = False, None, None, 0
    while i < len(line):
        # 1. jump past the closer
        if state is not None:
            end = find_close(state, line, i)
            if end < 0:
                break
            i, state = end, None
            continue
        # 2. find the next token
        m = lang.tokens.search(line, i)
        has_code = has_code or bool(line[i:m.start() if m else len(line)].strip())
        if m is None:
            break
        # 3. stop at a line comment
        if m.lastgroup == "comment":
            comment = line[m.end():]
            break
        # 4. open a string or block comment
        o = opened_by(lang, m)
        # A string or a char literal is code, and a block comment is not.
        has_code = has_code or o is None or o.kind != "block"
        if o is not None and o.kind == "heredoc":
            heredoc = o
        else:
            state = o
        i = m.end()
    # 5. close a plain string
    if state is not None and state.kind == "plain" and not lang.multiline:
        state = None
    return Scan(has_code, comment, state or heredoc)


# ---------------------------------------------------------------------------
# Lines
# ---------------------------------------------------------------------------

BLANK, CODE, IGNORED, COMMENT, DOC = "blank", "code", "ignored", "comment", "doc"

# no: 1-based line number. indent: visual column of the first character.
# text: for COMMENT and DOC the text after the marker, for CODE the stripped
# line. trailing: for CODE the text of a comment after the code, or None.
Line = namedtuple("Line", "no kind indent text trailing")


def visual_indent(raw, tab_width):
    """Return the column of the first non-space character of `raw`."""
    expanded = raw.expandtabs(tab_width)
    return len(expanded) - len(expanded.lstrip())


def classify_line(lang, no, raw, inside, s, tab_width):
    """Return the Line of one raw line from its Scan `s`.

    `inside` is True when the line starts inside a string or block comment.
    """
    indent = visual_indent(raw, tab_width)
    stripped = raw.strip()
    if inside:
        # Text inside a string or block comment neither comments nor ends a scope.
        return Line(no, IGNORED, indent, "", None)
    if not stripped:
        return Line(no, BLANK, indent, "", None)
    if s.comment is not None and not s.has_code:
        text = s.comment.expandtabs(tab_width)
        return Line(no, DOC if lang.doc.match(text) else COMMENT, indent, text, None)
    # A line of block comment only, or a label or preprocessor line that the
    # formatter outdents, says nothing about scope.
    if not s.has_code or (lang.outdented and lang.outdented.fullmatch(stripped)):
        return Line(no, IGNORED, indent, "", None)
    # A line that opens a string spanning lines may start at any column, as in
    # a macro argument at column 0, so it says nothing about scope either.
    if s.state is not None and s.state.kind != "block":
        return Line(no, IGNORED, indent, "", None)
    return Line(no, CODE, indent, stripped, s.comment)


def classify_lines(raw_lines, lang, tab_width):
    """Return one Line per raw line."""
    out, state = [], None
    for no, raw in enumerate(raw_lines, 1):
        s = scan_line(lang, raw, state)
        out.append(classify_line(lang, no, raw, state is not None, s, tab_width))
        state = s.state
    return out


# ---------------------------------------------------------------------------
# Comment blocks and their items
# ---------------------------------------------------------------------------

# A numbered item: digits, a period, then a space or the end of the text, as
# in "2. run the query" or "2.".
ITEM = re.compile(r"(\d+)\.(?:\s+(.*))?$")
# Two steps in one back-reference, such as "2. and 3.", "2., 3.", "2-3." or
# "2 & 3.".
COMBINED = re.compile(r"(\d{1,2})\.?\s*(?:,|&|-|/|\+|and\b|or\b|to\b)\s*(\d{1,2})\.(?=\s|$)")
# The line above a following item ends with one of these. A semicolon ends an
# item in a list of clauses.
ITEM_END = (".", "!", "?", ":", ";")

# number: the step number. no: the line of the number. col: the column of the
# number inside the comment text. text: the item's text over all its lines.
Item = namedtuple("Item", "number no col text")
# kind: "list", "title-list", "ref", "long-ref", "trailing", "stray-list" or
# "prose".
Block = namedtuple("Block", "first last indent lines kind items")


def comment_blocks(lines):
    """Return the runs of adjacent COMMENT lines at one indentation."""
    blocks, run = [], []
    for ln in lines:
        if ln.kind == COMMENT and run and run[-1].indent == ln.indent \
                and run[-1].no + 1 == ln.no:
            run.append(ln)
            continue
        if run:
            blocks.append(run)
        run = [ln] if ln.kind == COMMENT else []
    if run:
        blocks.append(run)
    return blocks


def leading_spaces(text):
    return len(text) - len(text.lstrip())


def starts_item(idx, prev, items, in_item, col, number, strict):
    """Return True when a line that starts with "N." is an item by rule R0.

    With `strict` False, a number also starts an item after a line that ends
    no sentence, when the line above belongs to no item or when the number
    follows the item above. This reads a list of titles.
    """
    if items and col > items[0].col:
        # A number indented further than the list is a nested sub-list, which
        # stays part of its item.
        return False
    if idx == 0 or prev == "" or prev.endswith(ITEM_END):
        return True
    return not strict and (not in_item or number == items[-1].number + 1)


def continues_item(item, in_item, prev, col):
    """Return True when a line continues `item`, the way Markdown continues a list item."""
    # Right below an item, any line at its column or deeper continues it.
    # After an empty line only a line indented further than the number does.
    return (in_item and prev != "" and col >= item.col) or col > item.col


def block_items(run, strict=True):
    """Return the numbered items of a comment block by rule R0, with `strict` as in starts_item."""
    items = []
    in_item = False  # the previous non-empty line belongs to items[-1]
    prev = None      # the previous line's stripped text, "" when it is empty
    for idx, ln in enumerate(run):
        s = ln.text.strip()
        if not s:
            prev = ""
            continue
        col = leading_spaces(ln.text)
        m = ITEM.match(s)
        if m and starts_item(idx, prev, items, in_item, col, int(m.group(1)), strict):
            items.append(Item(int(m.group(1)), ln.no, col, (m.group(2) or "").strip()))
            in_item = True
        elif items and continues_item(items[-1], in_item, prev, col):
            items[-1] = items[-1]._replace(text=(items[-1].text + " " + s).strip())
            in_item = True
        else:
            in_item = False
        prev = s
    return items


def is_list(items):
    return len(items) >= 2 and items[0].number == 1


def ends_sentence(text):
    return text.endswith((".", "!", "?"))


def numbered_one_to_n(items):
    return is_list(items) and [i.number for i in items] == list(range(1, len(items) + 1))


def has_note_below(run, item):
    """Return True when the lines below a back-reference on the first line of `run` are a note.

    A note starts at the column of the number or before it, and none of its
    lines is numbered.
    """
    rest = [ln.text for ln in run[1:] if ln.text.strip()]
    # A line indented further than the number continues the back-reference.
    if rest and leading_spaces(rest[0]) > item.col:
        return False
    return not any(ITEM.match(t.strip()) or COMBINED.match(t.strip()) for t in rest)


def make_block(run):
    """Return the Block of a comment run, with its items and its kind."""
    items = block_items(run)
    titles = block_items(run, strict=False)
    first = run[0].text.strip()
    combined = COMBINED.match(first)
    if combined and not (items and items[0].no == run[0].no):
        # "2-3. both" is no ITEM, but it is a back-reference to step 2 for N2.
        items = [Item(int(combined.group(1)), run[0].no, leading_spaces(run[0].text), "")] + items
    first_is_item = bool(items) and items[0].no == run[0].no
    if is_list(items):
        kind = "list"
    elif numbered_one_to_n(titles) and all(ends_sentence(i.text) for i in titles):
        # The items are full sentences numbered 1..n, so the line above item 1
        # is an introduction that ends without punctuation.
        kind, items = "list", titles
    elif numbered_one_to_n(titles) and not any(ends_sentence(i.text) for i in titles):
        # Titles numbered 1..n are a walkthrough without sentences. Reading it
        # as a walkthrough keeps its back-references from each giving N4.
        kind, items = "title-list", titles
    elif first_is_item and has_note_below(run, items[0]):
        # Only the first line is the back-reference, so its words leave out the note.
        words = ITEM.match(first)
        kind = "ref"
        items = [items[0]._replace(text=(words.group(2) or "").strip() if words else "")]
    elif first_is_item:
        kind = "long-ref"
    elif items:
        kind = "stray-list"
    else:
        kind = "prose"
    return Block(run[0].no, run[-1].no, run[0].indent, run, kind, items)


def trailing_block(ln):
    """Return a "trailing" Block for a numbered comment after code, or None for another comment."""
    text = ln.trailing.strip()
    if not (ITEM.match(text) or COMBINED.match(text)):
        return None
    b = make_block([Line(ln.no, COMMENT, ln.indent, ln.trailing, None)])
    return b._replace(kind="trailing")


# The header of a type, impl, namespace or module body, whose direct comments
# document members, such as "template <typename T> class Q {". Modifiers and a
# C++ template prefix may come first, as in "export default class A {".
CONTAINER = re.compile(
    r"^((pub(\([^)]*\))?|public|private|protected|internal|export|final|abstract"
    r"|static|unsafe|sealed|data|companion|open|inner|default|declare|partial"
    r"|template\s*<[^>]*>|type\s+\w+(\[[^\]]*\])?)\s*)*"
    r"(interface|struct|class|enum|union|trait|impl|object|record|protocol"
    r"|namespace|extension|module|extern)\b"
    r"[^=]*[{:]\s*$")


def enclosing_line(lines, at, indent):
    """Return the nearest code line above index `at` that is indented less than `indent`, or ""."""
    for ln in reversed(lines[:at]):
        if ln.kind == CODE and ln.indent < indent:
            return ln.text
    return ""


def is_c_declaration(lang, text):
    """Return True when a C family code line declares something."""
    if C_DECL.match(text):
        return True
    # In a source file `Foo bar(x);` is more often a statement than a
    # declaration. In a header it is a declaration.
    return bool(C_FUNCTION.match(text)) and "=" not in text.split("(")[0] \
        and (lang.name == "c-header" or not text.endswith(";"))


def is_doc_block(run, lines, lang, index_of):
    """Return True when a comment run documents a declaration or the file."""
    # The tests go from certain signs to guesses, and the first test that
    # applies decides:
    #
    # 1. A run at column 0 documents the file or an item, unless the language
    #    runs code at column 0.
    # 2. A run that continues a doc comment, or an Erlang run with an edoc tag,
    #    is documentation.
    # 3. A run of one line is kept, because a back-reference may sit above a
    #    nested declaration.
    # 4. A run directly inside a type, impl or namespace body documents a
    #    member. A run inside any other brace body is kept, because the line
    #    below it may be a statement that only looks like a declaration.
    # 5. A run directly above a declaration at its indentation documents it.
    first, last = run[0], run[-1]
    # 1. column 0
    if first.indent == 0 and not lang.top_level_code:
        return True
    # 2. doc comment or edoc tag
    at = index_of[first.no]
    if at > 0 and lines[at - 1].kind == DOC and lines[at - 1].indent == first.indent:
        return True
    if lang.name == "erlang" and any(re.match(r"\s*@\w+", ln.text) for ln in run):
        return True
    # 3. one line
    if len(run) == 1:
        return False
    # 4. the enclosing body
    enclosing = enclosing_line(lines, at, first.indent)
    if CONTAINER.search(enclosing):
        return True
    if enclosing.endswith("{"):
        return False
    # 5. the declaration below
    below_at = index_of[last.no] + 1
    if below_at >= len(lines):
        return False
    below = lines[below_at]
    if below.kind != CODE or below.indent != first.indent:
        return False
    if lang.decl == "c":
        return is_c_declaration(lang, below.text)
    return bool(lang.decl and lang.decl.match(below.text))


# ---------------------------------------------------------------------------
# Scopes and notes
# ---------------------------------------------------------------------------

def list_scope(block, lines, index_of, list_starts):
    """Return (first, end) line numbers of a list's scope, end exclusive.

    The scope ends at the first code line indented less than the list, at the
    next list in `list_starts` at the same or a smaller indentation, or at the
    end of the file.
    """
    for ln in lines[index_of[block.last] + 1:]:
        if ln.kind == CODE and ln.indent < block.indent:
            return block.last + 1, ln.no
        other = list_starts.get(ln.no)
        if other is not None and other.indent <= block.indent:
            return block.last + 1, ln.no
    return block.last + 1, (lines[-1].no + 1 if lines else block.last + 1)


def follows_numbered_comment(block, lines, index_of, numbered):
    """Return True when a numbered comment sits above `block` with no code between them."""
    for ln in reversed(lines[:index_of[block.first]]):
        if ln.kind not in (BLANK, COMMENT, DOC):
            return False
        if ln.no in numbered:
            return True
    return False


def sits_below_code_in_scope(block, lines, index_of, lists):
    """Return True when `block` sits right below code at its indentation in a list's scope."""
    at = index_of[block.first]
    above = lines[at - 1] if at > 0 else None
    if above is None or above.kind != CODE or above.indent != block.indent:
        return False
    starts = {b.first: b for b in lists}
    return any(b.indent <= block.indent
               and block.first < list_scope(b, lines, index_of, starts)[1]
               for b in lists)


def split_notes(blocks, lines, index_of):
    """Return (walkthroughs, notes) from the "list" blocks, in line order.

    A list is a note when a numbered comment sits above it with no code
    between, or when it sits right below code inside the scope of a
    walkthrough that comes before it.
    """
    numbered = {ln.no for b in blocks if b.kind != "prose" for ln in b.lines}
    lists, notes = [], []
    for b in (b for b in blocks if b.kind in ("list", "title-list")):
        if follows_numbered_comment(b, lines, index_of, numbered) \
                or sits_below_code_in_scope(b, lines, index_of, lists):
            notes.append(b)
        else:
            lists.append(b)
    return lists, notes


# ---------------------------------------------------------------------------
# Rules
# ---------------------------------------------------------------------------

RULES = {
    "N1": "a walkthrough list is numbered 1..n without gaps or repeats",
    "N2": "every step of a walkthrough has one back-reference in its scope",
    "N3": "the back-references of a walkthrough appear in ascending order",
    "N4": "a back-reference names a step of a walkthrough whose scope holds it",
    "N5": "a back-reference has words after its number",
    "N6": "a back-reference names one step",
    "N7": "numbered lines appear only in walkthroughs and one-line back-references",
    "N8": "a back-reference sits on its own line, not after code",
    "W1": "a back-reference has at most --warn-long words (8 by default)",
    "W2": "a walkthrough item ends a sentence and says more than its back-reference",
    "W3": "a back-reference shares a content word with its item",
    "D1": "no comment matches a --deny pattern",
}

Finding = namedtuple("Finding", "path line rule message")
# A back-reference: the block it is, the step numbers it names, its words.
Ref = namedtuple("Ref", "block numbers words")
# tab_width: columns per tab. deny: compiled --deny patterns. warn_long: the
# W1 word limit, or None when W1 is off. warn_text: W2 and W3 are on.
Options = namedtuple("Options", "tab_width deny warn_long warn_text")

STOPWORDS = {"the", "and", "for", "from", "into", "its", "are", "this", "that",
             "then", "else", "when", "which", "with", "not", "but", "has",
             "have", "was", "were", "will", "all", "any", "each"}


def make_ref(block):
    """Return the Ref of a back-reference block."""
    first = block.lines[0].text.strip()
    combined = COMBINED.match(first)
    if combined:
        numbers = (int(combined.group(1)), int(combined.group(2)))
        return Ref(block, numbers, first[combined.end():].strip())
    return Ref(block, (block.items[0].number,), block.items[0].text)


def owner_of(ref, lists, scopes):
    """Return (owner, around) for a ref.

    owner is the innermost list whose scope holds the ref and which has its
    step, or None. around holds every list whose scope holds the ref.
    """
    no = ref.block.first
    # A ref indented less than a list sits after the body that holds the list.
    around = [b for b in lists if scopes[b.first][0] <= no < scopes[b.first][1]
              and b.indent <= ref.block.indent]
    # Innermost means the deepest indentation, then the latest start.
    around.sort(key=lambda b: (b.indent, b.first), reverse=True)
    for b in around:
        if any(n in {i.number for i in b.items} for n in ref.numbers):
            return b, around
    return None, around


def content_words(text):
    """Return the lower-case words of three letters or more that are not stop words."""
    # A word starts with a letter, as "utf8" or "row_count".
    words = re.findall(r"[a-z][a-z0-9_]*", text.lower())
    return {w for w in words if len(w) >= 3 and w not in STOPWORDS}


def same_stem(a, b):
    """Return True when two words share four first letters, or one starts the other."""
    return len(os.path.commonprefix([a, b])) >= min(len(a), len(b), 4)


def normalized(text):
    return " ".join(text.lower().rstrip(".!?:;").split())


def check_ref_shape(path, ref, warn_long):
    """Return the N5, N6, N7, N8 and W1 findings of one back-reference."""
    out = []
    no = ref.block.first
    if len(ref.numbers) > 1:
        out.append(Finding(path, no, "N6", "back-reference names steps %s. Give each step its own"
                           % " and ".join(map(str, ref.numbers))))
    if not ref.words:
        out.append(Finding(path, no, "N5", "bare step number %d. needs a few words from its step"
                           % ref.numbers[0]))
    if ref.block.kind == "long-ref":
        out.append(Finding(path, no, "N7", "numbered comment of %d lines. A back-reference is one "
                           "line, and the text belongs in the walkthrough"
                           % len(ref.block.lines)))
    if ref.block.kind == "trailing":
        out.append(Finding(path, no, "N8", "back-reference to step %d after code. Put it on its "
                           "own line above the first line of the step" % ref.numbers[0]))
    if warn_long is not None and len(ref.words.split()) > warn_long:
        out.append(Finding(path, no, "W1", "back-reference has %d words, more than %d"
                           % (len(ref.words.split()), warn_long)))
    return out


def check_list_numbers(path, block):
    numbers = [i.number for i in block.items]
    if numbers == list(range(1, len(numbers) + 1)):
        return []
    return [Finding(path, block.items[0].no, "N1",
                    "walkthrough is numbered %s, expected 1..%d"
                    % (" ".join(map(str, numbers)), len(numbers)))]


def check_list_refs(path, block, refs, covered):
    """Return the N2 and N3 findings of one list against its refs in line order.

    `covered` holds the step numbers that a note inside the scope names. They
    count as present, so that a note is reported once as N7.
    """
    out = []
    seen = {}
    highest = 0
    for ref in refs:
        for n in ref.numbers:
            if n in seen:
                out.append(Finding(path, ref.block.first, "N2",
                                   "step %d already has a back-reference at line %d"
                                   % (n, seen[n])))
                continue
            seen[n] = ref.block.first
            if n < highest:
                out.append(Finding(path, ref.block.first, "N3",
                                   "step %d comes after step %d" % (n, highest)))
            highest = max(highest, n)
    for item in block.items:
        if item.number not in seen and item.number not in covered:
            out.append(Finding(path, item.no, "N2",
                               "step %d has no back-reference" % item.number))
    return out


def check_item_text(path, block, refs):
    """Return the W2 and W3 warnings of one list, given its refs."""
    out = []
    first_ref = {}
    for ref in refs:
        first_ref.setdefault(ref.numbers[0], ref)
    for item in block.items:
        ref = first_ref.get(item.number)
        # A sentence end is a ".", "!", "?" or ":" before a space or the end,
        # so the "." in "a.len()" is none.
        if not re.search(r"[.!?:](\s|$)", item.text):
            out.append(Finding(path, item.no, "W2", "step %d holds no sentence end. "
                               "Say what the step does and why" % item.number))
        elif ref is not None and normalized(item.text) == normalized(ref.words):
            out.append(Finding(path, item.no, "W2", "step %d says no more than its "
                               "back-reference" % item.number))
        if ref is None or not ref.words:
            continue
        ref_words, item_words = content_words(ref.words), content_words(item.text)
        if ref_words and not any(same_stem(a, b) for a in ref_words for b in item_words):
            out.append(Finding(path, ref.block.first, "W3", "back-reference shares no word "
                               "with step %d" % item.number))
    return out


def check_numbered_blocks(path, blocks, notes):
    """Return the N7 findings of notes, stray numbered items and walkthroughs of titles."""
    out = [Finding(path, b.items[0].no, "N7", "numbered list inside a note. Only a "
                   "walkthrough at the top of a body is numbered. Use bullets or prose")
           for b in notes]
    noted = {b.first for b in notes}
    for b in blocks:
        if b.kind == "stray-list":
            out.append(Finding(path, b.items[0].no, "N7",
                               "numbered items in a comment that is no walkthrough "
                               "(a walkthrough starts at 1 and has two items or more)"))
        if b.kind == "title-list" and b.first not in noted:
            out.append(Finding(path, b.items[0].no, "N7", "walkthrough items without "
                               "sentence ends. Each step is a full sentence that says "
                               "what the step does and why"))
    return out


def check_deny(path, lines, patterns):
    """Return a D1 finding for each comment line and trailing comment that matches a pattern."""
    texts = [(ln.no, ln.text if ln.kind in (COMMENT, DOC) else ln.trailing) for ln in lines]
    return [Finding(path, no, "D1", "comment matches %r" % p.pattern)
            for no, t in texts if t for p in patterns if p.search(t)]


def covered_steps(notes, lists, scopes):
    """Return, per walkthrough start, the step numbers that the notes in its scope name."""
    covered = {b.first: set() for b in lists}
    for b in notes:
        for item in b.items:
            owner, _ = owner_of(Ref(b, (item.number,), ""), lists, scopes)
            if owner is not None:
                covered[owner.first].add(item.number)
    return covered


def assign_refs(path, refs, lists, scopes):
    """Return (owned, findings) for the refs of a file.

    owned maps the start of each walkthrough to its refs, and findings holds
    an N4 for each ref that no walkthrough owns.
    """
    owned = {b.first: [] for b in lists}
    out = []
    for ref in refs:
        owner, around = owner_of(ref, lists, scopes)
        if owner is not None:
            owned[owner.first].append(ref)
        elif around:
            out.append(Finding(path, ref.block.first, "N4", "step %d is not in the walkthrough "
                               "at line %d" % (ref.numbers[0], around[0].first)))
        else:
            out.append(Finding(path, ref.block.first, "N4",
                               "step %d is outside the scope of any walkthrough"
                               % ref.numbers[0]))
    return owned, out


def check_walkthrough(path, block, refs, covered, warn_text):
    """Return the N1, N2, N3, W2 and W3 findings of one walkthrough, given its refs."""
    out = check_list_numbers(path, block)
    # A list of titles without back-references is reported once, as N7.
    if block.kind == "list" or refs:
        out.extend(check_list_refs(path, block, refs, covered))
    if warn_text:
        out.extend(check_item_text(path, block, refs))
    return out


def check_lines(path, raw_lines, lang, opts, line_offset=0):
    """Return the findings of one file or one Markdown code block."""
    # The check runs in four steps:
    #
    # 1. Classify the lines and group the comment lines into blocks.
    # 2. Drop the blocks that document a declaration or the file, and add a
    #    block for each numbered comment after code.
    # 3. Split the lists into walkthroughs and notes, then compute the scope of
    #    each walkthrough. A note does not end a scope, so the split runs first.
    # 4. Give each back-reference and each note to the walkthrough whose scope
    #    holds it, then check each walkthrough. A walkthrough is checked only
    #    once all its back-references are known.

    # 1. classify and group
    lines = classify_lines(raw_lines, lang, opts.tab_width)
    index_of = {ln.no: i for i, ln in enumerate(lines)}
    runs = comment_blocks(lines)

    # 2. drop doc blocks and add trailing ones
    blocks = [make_block(r) for r in runs if not is_doc_block(r, lines, lang, index_of)]
    trailing = [trailing_block(ln) for ln in lines if ln.kind == CODE and ln.trailing]
    refs = sorted([make_ref(b) for b in blocks if b.kind in ("ref", "long-ref")]
                  + [make_ref(b) for b in trailing if b is not None],
                  key=lambda r: r.block.first)

    # 3. split notes and compute scopes
    lists, notes = split_notes(blocks, lines, index_of)
    starts = {b.first: b for b in lists}
    scopes = {b.first: list_scope(b, lines, index_of, starts) for b in lists}

    # 4. assign and check
    covered = covered_steps(notes, lists, scopes)
    owned, out = assign_refs(path, refs, lists, scopes)
    out.extend(check_numbered_blocks(path, blocks, notes))
    out.extend(f for ref in refs for f in check_ref_shape(path, ref, opts.warn_long))
    out.extend(f for b in lists for f in check_walkthrough(
        path, b, owned[b.first], covered[b.first], opts.warn_text))
    out.extend(check_deny(path, lines, opts.deny))
    return [f._replace(line=f.line + line_offset) for f in out]


# ---------------------------------------------------------------------------
# Inputs
# ---------------------------------------------------------------------------

# A Markdown fence opener such as "```rust" or "  ~~~ python": the indentation,
# the fence and the info string.
FENCE = re.compile(r"^(\s*)(`{3,}|~{3,})\s*([\w+#-]*)")
# A source to check. markdown: check the fenced code blocks of `text`.
Source = namedtuple("Source", "path markdown text")


def markdown_blocks(raw_lines):
    """Yield (language key, line offset, lines) for each fenced block of a known language."""
    i = 0
    while i < len(raw_lines):
        m = FENCE.match(raw_lines[i])
        if not m:
            i += 1
            continue
        pad, fence, info = len(m.group(1)), m.group(2), m.group(3).lower()
        j = i + 1
        while j < len(raw_lines) and not raw_lines[j].strip().startswith(fence):
            j += 1
        # The body loses the fence's indentation, as Markdown renders it.
        body = [ln[min(pad, visual_indent(ln, 8)):] for ln in raw_lines[i + 1:j]]
        if info in FENCE_LANGS:
            yield FENCE_LANGS[info], i + 1, body
        i = j + 1


def check_source(src, opts):
    """Return the findings of one Source."""
    raw_lines = src.text.splitlines()
    if src.markdown:
        return [f for key, offset, body in markdown_blocks(raw_lines)
                for f in check_lines(src.path, body, LANGS[key], opts, offset)]
    return check_lines(src.path, raw_lines, LANGS[language_of(src.path)], opts)


def read_text(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        return f.read()


def walk_files(path):
    """Yield `path` when it is a file, or else each file below it outside SKIP_DIRS, sorted."""
    if os.path.isfile(path):
        yield path
        return
    for root, dirs, files in os.walk(path):
        dirs[:] = sorted(d for d in dirs if d not in SKIP_DIRS)
        for name in sorted(files):
            yield os.path.join(root, name)


def path_sources(paths, markdown_paths):
    """Return the Sources of the given files and directories and Markdown documents."""
    code = [Source(p, False, read_text(p))
            for root in paths for p in walk_files(root) if language_of(p)]
    return code + [Source(p, True, read_text(p)) for p in markdown_paths]


def parse_range(text):
    """Return (base, rev) of a range "<base>...<rev>", or None when it has another shape."""
    base, sep, rev = text.partition("...")
    # A side that starts with "-" would reach git as an option. A side next to
    # a fourth "." means the range had more than three dots.
    if not sep or not base or not rev or "..." in rev:
        return None
    if base.startswith("-") or rev.startswith(("-", ".")) or base.endswith("."):
        return None
    return base, rev


def git(args):
    """Return the stdout of git run with `args` in the current directory.

    Raises CalledProcessError when git fails and OSError when it cannot run.
    """
    return subprocess.run(["git"] + args, capture_output=True, check=True).stdout


def range_sources(base, rev, markdown_too):
    """Return (short sha of rev, Sources) for the files that a range changed.

    The files are those that differ between rev and the merge base of base and
    rev, and their text is their content at rev. Raises as `git` does.
    """
    sha = git(["rev-parse", "--verify", "--short", rev + "^{commit}"]).decode().strip()
    # The filter keeps added, modified and renamed files, and -z keeps unusual
    # names intact.
    names = git(["diff", "--name-only", "-z", "-M", "--diff-filter=AMR",
                 base + "..." + rev, "--"]).decode("utf-8", "surrogateescape")
    paths = [p for p in names.split("\0") if p
             and not SKIP_DIRS.intersection(p.split("/")[:-1])]
    # A path without a language is a Markdown file.
    wanted = [(p, language_of(p) is None) for p in paths
              if language_of(p) or (markdown_too and p.lower().endswith(".md"))]
    return sha, [Source(p, md, git(["show", rev + ":" + p]).decode("utf-8", "replace"))
                 for p, md in wanted]


# ---------------------------------------------------------------------------
# Command line
# ---------------------------------------------------------------------------

EPILOG = """\
rules:
%s

N1 to N8 and D1 are findings. W1 to W3 are warnings, which are off by default
and never change the exit status.

R0, which separates an item from a wrapped sentence: a comment line that
starts with "N." is a numbered item when it is the first line of its block,
when the line above is empty, or when the line above ends a sentence or a
clause (. ! ? : ;). A number indented further than the first item belongs
to the item above. Any other such line is a wrapped sentence. A line right
below an item continues it, and so does a line indented further than its
number.

A walkthrough is a comment block with two items or more, numbered from 1.
When the items are numbered 1..n and each ends a sentence, the block is a
walkthrough even if the line above item 1 has no end. A block numbered 1..n
in which no item ends a sentence is a walkthrough of titles and is reported
once as N7.

A back-reference is a comment whose first line is "N. words", outside a
walkthrough. Lines below it that start at the column of the number or before
it are a note. A back-reference whose next line is indented further than
the number, or whose note holds a numbered line, is reported as N7. A
comment "N. words" after code counts as a back-reference and is reported as
N8.

The scope of a walkthrough runs from the line after it to the first code
line indented less, or to the next walkthrough at the same or a smaller
indentation. A numbered list below another numbered comment, or right below
code inside the scope of a walkthrough, is a note and is reported once as
N7.

words: --words takes the rest of the command line, so it comes after every
other option. The scope is the words before the first word that ends with a
colon, such as "focus:" or "plan:", and the check ignores that word and every
word after it. In the scope, a word that names an existing file or directory
is checked as a path, and an existing .md file as with --markdown. Any other
word BASE...REV (exactly three dots, neither side starting with "-") is
checked as --git-range with --markdown-too, and the check ignores the
remaining words. A range that git does not know is ignored with the line
"check_steps: ignored '<word>': git does not know this range" on stderr.
With a range and paths, both are checked. Without any range or path the
check prints "check_steps: no range and no path in the words" and exits 2.
Two ranges are refused.

limits:
%s
output: one line "path:line: rule: message" per finding, then
"check_steps: <n> findings in <m> files". With warnings on, the count reads
"<n> findings, <w> warnings". With a range " at <short sha>" follows.

exit status: 0 without findings, 1 with findings, 2 on a usage or git error.
""" % ("\n".join("  %s  %s" % kv for kv in RULES.items()),
       (__doc__ or "").partition("Accepted limits:\n")[2])


def split_words(argv):
    """Return (options, words): the arguments before "--words" and the words after it.

    words is None when argv holds no "--words".
    """
    for i, a in enumerate(argv):
        if a == "--words" or a.startswith("--words="):
            first = a.partition("=")[2]
            return argv[:i], ([first] if first else []) + argv[i + 1:]
    return argv, None


def parse_args(argv):
    """Return the parsed arguments. Exits with status 2 on a usage error."""
    options, words = split_words(argv)
    p = argparse.ArgumentParser(
        prog="check_steps.py",
        description="Check walkthrough step numbers against their back-references.",
        epilog=EPILOG, formatter_class=argparse.RawDescriptionHelpFormatter,
        # split_words finds only the full "--words", so an abbreviation such as
        # "--wor" must be a usage error.
        allow_abbrev=False)
    p.add_argument("paths", nargs="*", metavar="path",
                   help="a source file or a directory to walk")
    p.add_argument("--git-range", metavar="BASE...REV",
                   help="check the files that differ between the merge base of BASE "
                        "and REV, as they are at REV. Takes no paths")
    p.add_argument("--markdown", action="append", default=[], metavar="FILE",
                   help="check the fenced code blocks of a Markdown document (repeatable)")
    p.add_argument("--markdown-too", action="store_true",
                   help="with --git-range, also check the changed .md files")
    p.add_argument("--warn", action="store_true",
                   help="report warnings W1, W2 and W3")
    p.add_argument("--warn-long", type=int, metavar="N",
                   help="report W1 for back-references of more than N words")
    p.add_argument("--deny", action="append", default=[], metavar="REGEX",
                   help="report comment lines that match REGEX (repeatable)")
    p.add_argument("--tab-width", type=int, default=4, metavar="N",
                   help="columns per tab when comparing indentation (default 4)")
    # split_words takes "--words" out of the arguments before they are parsed,
    # because argparse cannot give an option the rest of the command line.
    p.add_argument("--words", nargs="+", metavar="W",
                   help="check the ranges and paths among the words that follow, up "
                        "to the first word that ends with a colon, and ignore the "
                        "other words, see \"words\" below. Takes no paths, "
                        "--git-range or --markdown")
    args = p.parse_args(options)
    args.words = words
    if words is not None and (args.paths or args.markdown or args.git_range):
        p.error("--words takes no paths, no --git-range and no --markdown")
    if args.git_range and (args.paths or args.markdown):
        p.error("--git-range takes no paths and no --markdown")
    if args.markdown_too and not args.git_range:
        p.error("--markdown-too needs --git-range")
    if not (args.paths or args.markdown or args.git_range or words is not None):
        p.error("give a path, --markdown FILE, --git-range BASE...REV or --words W")
    for path in args.paths + args.markdown:
        if not os.path.exists(path):
            p.error("no such path: %s" % path)
    try:
        args.deny = [re.compile(d) for d in args.deny]
    except re.error as e:
        p.error("bad --deny pattern: %s" % e)
    return args


def plural(n, word):
    return "%d %s%s" % (n, word, "" if n == 1 else "s")


def summary(findings, warnings, files, warn_on, sha):
    """Return the last output line."""
    counts = plural(findings, "finding")
    if warn_on:
        counts += ", " + plural(warnings, "warning")
    return "check_steps: %s in %s%s" % (counts, plural(files, "file"),
                                        " at " + sha if sha else "")


def range_or_error(text, markdown_too):
    """Return (short sha, Sources, None) for a --git-range value.

    Returns (None, None, message) when the range is malformed or git fails.
    """
    parsed = parse_range(text)
    if parsed is None:
        return None, None, "malformed range %r, expected BASE...REV" % text
    try:
        sha, sources = range_sources(parsed[0], parsed[1], markdown_too)
    except subprocess.CalledProcessError as e:
        reason = e.stderr.decode("utf-8", "replace").strip().splitlines()
        return None, None, "git %s failed: %s" % (
            e.cmd[1], reason[0] if reason else "exit status %d" % e.returncode)
    except OSError as e:
        return None, None, "cannot run git: %s" % e
    return sha, sources, None


def scope_words(words):
    """Return the words before the first word that ends with a colon, such as "focus:"."""
    for i, w in enumerate(words):
        if w.endswith(":"):
            return words[:i]
    return words


def classify_words(words):
    """Return (ranges, paths, Markdown paths) among the scope words, dropping the other words."""
    ranges, paths, markdown = [], [], []
    for w in words:
        # An existing file is a path even when its name looks like a range, as "v1...v2.hs".
        if os.path.isfile(w) and w.lower().endswith(".md"):
            markdown.append(w)
        elif os.path.exists(w):
            paths.append(w)
        elif parse_range(w):
            ranges.append(w)
    return ranges, paths, markdown


def is_known_range(text):
    """Return True when git knows both revisions of a range that parse_range accepts."""
    try:
        for rev in parse_range(text):
            git(["rev-parse", "--verify", "--quiet", rev + "^{commit}"])
    except subprocess.CalledProcessError:
        return False
    except OSError:
        # range_or_error runs git again and reports that it cannot run.
        return True
    return True


def words_or_error(words):
    """Return (short sha or None, Sources, None) for the ranges and paths among the scope words.

    Prints one line to stderr for each range that git does not know, and drops
    it. Returns (None, None, message) without a range or path, with two ranges,
    or when the range fails as in range_or_error.
    """
    candidates, paths, markdown = classify_words(scope_words(words))
    ranges = [w for w in candidates if is_known_range(w)]
    for w in candidates:
        if w not in ranges:
            print("check_steps: ignored '%s': git does not know this range" % w,
                  file=sys.stderr)
    if not (ranges or paths or markdown):
        return None, None, "no range and no path in the words"
    if len(ranges) > 1:
        return None, None, "more than one range in the words: " + " ".join(ranges)
    sha, sources, error = range_or_error(ranges[0], True) if ranges else (None, [], None)
    if error:
        return None, None, error
    return sha, sources + path_sources(paths, markdown), None


def sources_or_error(args):
    """Return (short sha or None, Sources, None) for the parsed arguments.

    Returns (None, None, message) on an error in the range or the words.
    """
    if args.words is not None:
        return words_or_error(args.words)
    if args.git_range:
        return range_or_error(args.git_range, args.markdown_too)
    return None, path_sources(args.paths, args.markdown), None


def main(argv=None):
    """Run the check, print its findings and return the exit status."""
    args = parse_args(sys.argv[1:] if argv is None else argv)
    warn_long = args.warn_long if args.warn_long is not None else (8 if args.warn else None)
    opts = Options(args.tab_width, args.deny, warn_long, args.warn)
    sha, sources, error = sources_or_error(args)
    if error:
        print("check_steps: " + error, file=sys.stderr)
        return 2
    found = sorted({f for s in sources for f in check_source(s, opts)})
    for f in found:
        print("%s:%d: %s: %s" % f)
    warnings = sum(1 for f in found if f.rule.startswith("W"))
    print(summary(len(found) - warnings, warnings, len(sources),
                  warn_long is not None or args.warn, sha))
    return 1 if len(found) > warnings else 0


if __name__ == "__main__":
    sys.exit(main())
