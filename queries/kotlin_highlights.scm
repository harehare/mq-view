; Custom highlights query for the tree-sitter-kotlin-ng grammar
; (node names differ from the older fwcd/tree-sitter-kotlin grammar,
; so this cannot be reused as-is from upstream).

[
  (line_comment)
  (block_comment)
] @comment

(string_content) @string
(character_literal) @string
(number_literal) @number
(float_literal) @number

(this_expression) @variable.builtin
(super_expression) @variable.builtin

(class_declaration name: (identifier) @type)
(object_declaration name: (identifier) @type)
(user_type (identifier) @type)

(function_declaration name: (identifier) @function)

(parameter (identifier) @variable.parameter)

(import (identifier) @namespace)
(import (qualified_identifier) @namespace)
(package_header (qualified_identifier) @namespace)

(annotation "@" @attribute)

[
  "fun"
  "val"
  "var"
  "class"
  "object"
  "interface"
  "enum"
  "companion"
  "constructor"
  "init"
  "typealias"
  "package"
  "import"
  "return"
  "throw"
  "if"
  "else"
  "when"
  "for"
  "do"
  "while"
  "try"
  "catch"
  "finally"
  "is"
  "in"
  "!in"
  "!is"
  "as"
  "as?"
  "by"
  "out"
  "public"
  "private"
  "protected"
  "internal"
  "open"
  "final"
  "abstract"
  "override"
  "sealed"
  "data"
  "inline"
  "noinline"
  "crossinline"
  "suspend"
  "operator"
  "infix"
  "tailrec"
  "vararg"
  "lateinit"
  "const"
  "external"
  "annotation"
  "actual"
  "expect"
  "value"
  "inner"
  "get"
  "set"
  "where"
  "dynamic"
] @keyword

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  "."
  ","
  ";"
  ":"
  "::"
] @punctuation.delimiter

[
  "="
  "=="
  "==="
  "!="
  "!=="
  "<"
  "<="
  ">"
  ">="
  "&&"
  "||"
  "!"
  "+"
  "-"
  "*"
  "/"
  "%"
  "+="
  "-="
  "*="
  "/="
  "%="
  "++"
  "--"
  "->"
  "?."
  "?:"
  ".."
  "..<"
] @operator
