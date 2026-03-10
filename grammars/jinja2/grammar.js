/**
 * @file Jinja2 template grammar for tree-sitter
 * @license Apache-2.0
 *
 * Based on the Jinja2 language specification:
 * https://jinja.palletsprojects.com/en/3.1.x/templates/
 *
 * Supports all standard Jinja2 constructs:
 *   - Template structure: literal content, comments, expression output, statements
 *   - Compound statements: block, for, if, macro, call, filter, set (block), with,
 *     autoescape, trans
 *   - Simple statements: extends, include, import, from, set (assign), do, debug
 *   - Expression language: literals, variables, filters, tests, all operators,
 *     function calls, attribute access, subscript, list/dict/tuple literals
 */

// @ts-check

/** Operator precedence table (higher = tighter binding) */
const PREC = {
  TERNARY: 1,   // a if c else b
  OR: 2,        // a or b
  AND: 3,       // a and b
  NOT: 4,       // not a
  COMPARE: 5,   // == != < > <= >= in not_in is is_not
  CONCAT: 6,    // a ~ b
  ADD: 7,       // a + b, a - b
  MUL: 8,       // a * b, a / b, a // b, a % b
  UNARY: 9,     // -a, +a
  POWER: 10,    // a ** b
  FILTER: 11,   // a | f
  CALL: 12,     // f(args)
  ATTRIBUTE: 13, // a.b, a[b]
};

module.exports = grammar({
  name: 'jinja2',

  // External scanner handles:
  //   content     — literal text that stops at {%, {{, {#
  //   raw_content — text inside {% raw %}...{% endraw %} (stops at that tag)
  externals: $ => [
    $.content,
    $._raw_content,
  ],

  // Whitespace is insignificant inside template tags
  extras: $ => [
    /[ \t\r\n]/,
  ],

  // Reserved keywords — prevents 'not' from being an identifier, etc.
  word: $ => $.identifier,

  conflicts: $ => [
    // {% call(params) expr %} vs {% call expr %}: both start with call+(
    [$._call_caller_params, $._expression],
    [$.parameter, $._primary_expression],
    // Ternary 'if' vs for-loop's 'if' filter
    [$.ternary_expression, $.for_statement],
    // 'not in' is two tokens but one operator
    [$._not_in, $.not_expression],
    // for-loop target: 'identifier' can be _for_target directly or start of tuple
    [$._for_target, $._primary_expression],
    // identifier_tuple in for-loop target vs plain identifier
    [$._for_target, $._primary_expression],
    // Clauses with optional bodies: GLR resolves when body ends vs closing tag
    [$.else_clause],
    [$.for_else],
    [$.elif_clause],
    [$.pluralize_clause],
    [$.block_statement],
    [$.for_statement],
    [$.if_statement],
    [$.macro_statement],
    [$.call_statement],
    [$.filter_block_statement],
    [$.set_block_statement],
    [$.with_statement],
    [$.autoescape_statement],
    [$.trans_statement],
  ],

  rules: {
    // =========================================================================
    // Top-level
    // =========================================================================

    source_file: $ => repeat($._node),

    _node: $ => choice(
      $.comment,
      $.expression_statement,
      $.raw_statement,
      // compound (have bodies)
      $.block_statement,
      $.for_statement,
      $.if_statement,
      $.macro_statement,
      $.call_statement,
      $.filter_block_statement,
      $.set_block_statement,
      $.with_statement,
      $.autoescape_statement,
      $.trans_statement,
      // simple (no body)
      $.extends_statement,
      $.include_statement,
      $.import_statement,
      $.from_statement,
      $.set_statement,
      $.do_statement,
      $.debug_statement,
      // literal
      $.content,
    ),

    // =========================================================================
    // Delimiters — with optional whitespace-control characters (- and +)
    // =========================================================================

    _block_open:  _ => token(choice('{%', '{%-', '{%+')),
    _block_close: _ => token(choice('%}', '-%}', '+%}')),
    _expr_open:   _ => token(choice('{{', '{{-', '{{+')),
    _expr_close:  _ => token(choice('}}', '-}}', '+}}')),

    // =========================================================================
    // Comment: {# ... #}
    // =========================================================================

    // Handled as a single regex token; comment content is opaque.
    // The pattern ([^#]|#[^}])* matches: any non-# char OR # not followed by }
    comment: _ => token(seq(
      '{#',
      /([^#]|#[^}])*/,
      '#}',
    )),

    // =========================================================================
    // Expression output: {{ expr }}
    // =========================================================================

    expression_statement: $ => seq(
      $._expr_open,
      field('value', $._expression),
      $._expr_close,
    ),

    // =========================================================================
    // Raw block: {% raw %}...{% endraw %}
    // External scanner ensures the content is opaque (no tag processing).
    // =========================================================================

    raw_statement: $ => seq(
      $._block_open, 'raw', $._block_close,
      optional($._raw_content),
      $._block_open, 'endraw', $._block_close,
    ),

    // =========================================================================
    // Simple statements (single tag, no body)
    // =========================================================================

    // {% extends "base.html" %}
    extends_statement: $ => seq(
      $._block_open,
      'extends',
      field('path', $._expression),
      $._block_close,
    ),

    // {% include expr [ignore missing] [with[out] context] %}
    include_statement: $ => seq(
      $._block_open,
      'include',
      field('path', $._expression),
      optional($.include_ignore_missing),
      optional($.include_context),
      $._block_close,
    ),

    include_ignore_missing: _ => seq('ignore', 'missing'),
    include_context: _ => choice(seq('with', 'context'), seq('without', 'context')),

    // {% import "macros.html" as alias %}
    import_statement: $ => seq(
      $._block_open,
      'import',
      field('path', $._expression),
      'as',
      field('alias', $.identifier),
      $._block_close,
    ),

    // {% from "macros.html" import name [as alias] [, ...] %}
    from_statement: $ => seq(
      $._block_open,
      'from',
      field('path', $._expression),
      'import',
      field('names', $.import_list),
      $._block_close,
    ),

    import_list: $ => seq(
      $.import_item,
      repeat(seq(',', $.import_item)),
      optional(','),
    ),

    import_item: $ => seq(
      field('name', $.identifier),
      optional(seq('as', field('alias', $.identifier))),
    ),

    // {% set name = expr %}
    set_statement: $ => seq(
      $._block_open,
      'set',
      field('name', $.identifier),
      '=',
      field('value', $._expression),
      $._block_close,
    ),

    // {% do expr %}
    do_statement: $ => seq(
      $._block_open,
      'do',
      field('expression', $._expression),
      $._block_close,
    ),

    // {% debug %} or {% debug var %}
    debug_statement: $ => seq(
      $._block_open,
      'debug',
      optional(field('expression', $._expression)),
      $._block_close,
    ),

    // =========================================================================
    // Compound statements (have bodies)
    // =========================================================================

    // {% block name [scoped] %}...{% endblock [name] %}
    block_statement: $ => seq(
      $._block_open,
      'block',
      field('name', $.identifier),
      optional('scoped'),
      $._block_close,
      optional(field('body', $._body)),
      $._block_open, 'endblock', optional($.identifier), $._block_close,
    ),

    // {% for target in iterable [if condition] [recursive] %}
    //   body
    // [{% else %}
    //   else_body]
    // {% endfor %}
    for_statement: $ => seq(
      $._block_open,
      'for',
      field('target', $._for_target),
      'in',
      field('iterable', $._expression),
      optional(seq('if', field('condition', $._expression))),
      optional(field('recursive', alias('recursive', $.recursive))),
      $._block_close,
      optional(field('body', $._body)),
      optional($.for_else),
      $._block_open, 'endfor', $._block_close,
    ),

    // For-loop targets are always simple identifiers or comma-separated identifiers.
    // Jinja2 doesn't support arbitrary expression unpacking — only names.
    // Using tuple_expression here would cause `key, value in items` to be
    // mis-parsed as tuple(key, comparison(value in items)).
    _for_target: $ => choice(
      $.identifier,
      $.identifier_tuple,
    ),

    // Tuple of identifiers (only valid in for-loop targets)
    identifier_tuple: $ => seq(
      $.identifier,
      repeat1(seq(',', $.identifier)),
      optional(','),
    ),

    for_else: $ => seq(
      $._block_open, 'else', $._block_close,
      optional($._body),
    ),

    // {% if condition %}
    //   body
    // [{% elif condition %} body]*
    // [{% else %} body]
    // {% endif %}
    if_statement: $ => seq(
      $._block_open,
      'if',
      field('condition', $._expression),
      $._block_close,
      optional(field('body', $._body)),
      repeat($.elif_clause),
      optional($.else_clause),
      $._block_open, 'endif', $._block_close,
    ),

    elif_clause: $ => seq(
      $._block_open,
      'elif',
      field('condition', $._expression),
      $._block_close,
      optional(field('body', $._body)),
    ),

    else_clause: $ => seq(
      $._block_open, 'else', $._block_close,
      optional($._body),
    ),

    // {% macro name(params) %}...{% endmacro %}
    macro_statement: $ => seq(
      $._block_open,
      'macro',
      field('name', $.identifier),
      field('parameters', $.parameter_list),
      $._block_close,
      optional(field('body', $._body)),
      $._block_open, 'endmacro', $._block_close,
    ),

    // {% call[(caller_params)] callee(args) %}...{% endcall %}
    call_statement: $ => seq(
      $._block_open,
      'call',
      optional(field('caller_parameters', $._call_caller_params)),
      field('callee', $._expression),
      $._block_close,
      optional(field('body', $._body)),
      $._block_open, 'endcall', $._block_close,
    ),

    _call_caller_params: $ => seq(
      '(',
      optional(seq(
        $.parameter,
        repeat(seq(',', $.parameter)),
        optional(','),
      )),
      ')',
    ),

    // {% filter filter_name[(args)] %}...{% endfilter %}
    filter_block_statement: $ => seq(
      $._block_open,
      'filter',
      field('filter', $.filter_chain),
      $._block_close,
      optional(field('body', $._body)),
      $._block_open, 'endfilter', $._block_close,
    ),

    // {% set name [| filter] %}...{% endset %}
    set_block_statement: $ => seq(
      $._block_open,
      'set',
      field('name', $.identifier),
      optional(seq('|', field('filter', $.filter_chain))),
      $._block_close,
      optional(field('body', $._body)),
      $._block_open, 'endset', $._block_close,
    ),

    // {% with [name=value, ...] %}...{% endwith %}
    with_statement: $ => seq(
      $._block_open,
      'with',
      optional(field('assignments', $.with_assignments)),
      $._block_close,
      optional(field('body', $._body)),
      $._block_open, 'endwith', $._block_close,
    ),

    with_assignments: $ => seq(
      $.with_assignment,
      repeat(seq(',', $.with_assignment)),
    ),

    with_assignment: $ => seq(
      field('name', $.identifier),
      '=',
      field('value', $._expression),
    ),

    // {% autoescape [expr] %}...{% endautoescape %}
    autoescape_statement: $ => seq(
      $._block_open,
      'autoescape',
      optional(field('value', $._expression)),
      $._block_close,
      optional(field('body', $._body)),
      $._block_open, 'endautoescape', $._block_close,
    ),

    // {% trans [var=expr, ...] %}singular{% pluralize [count] %}plural{% endtrans %}
    //
    // GLR resolves the body/pluralize/endtrans ambiguity: the scanner can't know
    // when the singular body ends until it sees the keyword after {% ... %}.
    // Declaring [$.trans_statement] in conflicts enables GLR for this rule.
    trans_statement: $ => seq(
      $._block_open,
      'trans',
      optional(field('variables', $.trans_variables)),
      $._block_close,
      optional(field('singular', $._body)),
      optional(field('plural', $.pluralize_clause)),
      $._block_open, 'endtrans', $._block_close,
    ),

    trans_variables: $ => seq(
      $.trans_variable,
      repeat(seq(',', $.trans_variable)),
    ),

    trans_variable: $ => seq(
      field('name', $.identifier),
      '=',
      field('value', $._expression),
    ),

    pluralize_clause: $ => seq(
      $._block_open,
      'pluralize',
      optional(field('count', $.identifier)),
      $._block_close,
      optional(field('body', $._body)),
    ),

    // =========================================================================
    // Body: non-empty sequence of nodes between opening and closing tags.
    // tree-sitter forbids named rules matching empty string; use optional()
    // at each call site to handle potentially-empty bodies.
    // =========================================================================

    _body: $ => repeat1($._node),

    // =========================================================================
    // Parameters (for macro/call definitions)
    // =========================================================================

    parameter_list: $ => seq(
      '(',
      optional(seq(
        $.parameter,
        repeat(seq(',', $.parameter)),
        optional(','),
      )),
      ')',
    ),

    parameter: $ => seq(
      field('name', $.identifier),
      optional(seq('=', field('default', $._expression))),
    ),

    // =========================================================================
    // Expression grammar
    // =========================================================================

    _expression: $ => choice(
      $.ternary_expression,
      $.or_expression,
      $.and_expression,
      $.not_expression,
      $.comparison_expression,
      $.concat_expression,
      $.add_expression,
      $.mul_expression,
      $.unary_expression,
      $.power_expression,
      $.filter_expression,
      $.test_expression,
      $.call_expression,
      $.attribute_expression,
      $.subscript_expression,
      $._primary_expression,
      // Note: tuple syntax is NOT supported in general expressions.
      // Tuples only appear in for-loop targets (key, value unpacking),
      // handled via identifier_tuple in _for_target.
    ),

    // Ternary: value if condition else alternative
    ternary_expression: $ => prec.right(PREC.TERNARY, seq(
      field('value', $._expression),
      'if',
      field('condition', $._expression),
      'else',
      field('alternative', $._expression),
    )),

    // Logical or
    or_expression: $ => prec.left(PREC.OR, seq(
      field('left', $._expression),
      'or',
      field('right', $._expression),
    )),

    // Logical and
    and_expression: $ => prec.left(PREC.AND, seq(
      field('left', $._expression),
      'and',
      field('right', $._expression),
    )),

    // Logical not
    not_expression: $ => prec(PREC.NOT, seq(
      'not',
      field('operand', $._expression),
    )),

    // Comparison operators
    comparison_expression: $ => prec.left(PREC.COMPARE, seq(
      field('left', $._expression),
      field('operator', $._comparison_op),
      field('right', $._expression),
    )),

    _comparison_op: $ => choice(
      '==', '!=', '<', '>', '<=', '>=',
      'in',
      alias($._not_in, 'not in'),
    ),

    _not_in: _ => seq('not', 'in'),

    // String concatenation: a ~ b
    concat_expression: $ => prec.left(PREC.CONCAT, seq(
      field('left', $._expression),
      '~',
      field('right', $._expression),
    )),

    // Additive: + -
    add_expression: $ => prec.left(PREC.ADD, seq(
      field('left', $._expression),
      field('operator', choice('+', '-')),
      field('right', $._expression),
    )),

    // Multiplicative: * / // %
    mul_expression: $ => prec.left(PREC.MUL, seq(
      field('left', $._expression),
      field('operator', choice('*', '/', '//', '%')),
      field('right', $._expression),
    )),

    // Unary: -a +a
    unary_expression: $ => prec(PREC.UNARY, seq(
      field('operator', choice('-', '+')),
      field('operand', $._expression),
    )),

    // Power: a ** b (right-associative)
    power_expression: $ => prec.right(PREC.POWER, seq(
      field('base', $._expression),
      '**',
      field('exponent', $._expression),
    )),

    // Filter application: value | filter[(args)]
    filter_expression: $ => prec.left(PREC.FILTER, seq(
      field('value', $._expression),
      '|',
      field('filter', $.filter_chain),
    )),

    // filter_chain: a single filter or chained filters
    filter_chain: $ => prec.left(seq(
      $.filter_item,
      repeat(seq('|', $.filter_item)),
    )),

    filter_item: $ => prec.right(seq(
      field('name', $.identifier),
      optional(field('arguments', $.argument_list)),
    )),

    // Test: value is [not] test_name
    test_expression: $ => prec.left(PREC.COMPARE, seq(
      field('value', $._expression),
      'is',
      optional(field('negated', 'not')),
      field('test', $.identifier),
    )),

    // Function/method call: f(args)
    call_expression: $ => prec.left(PREC.CALL, seq(
      field('function', $._expression),
      field('arguments', $.argument_list),
    )),

    // Attribute access: a.b
    attribute_expression: $ => prec.left(PREC.ATTRIBUTE, seq(
      field('object', $._expression),
      '.',
      field('attribute', $.identifier),
    )),

    // Subscript: a[key]
    subscript_expression: $ => prec.left(PREC.ATTRIBUTE, seq(
      field('object', $._expression),
      '[',
      field('key', $._expression),
      ']',
    )),

    // =========================================================================
    // Primary expressions
    // =========================================================================

    _primary_expression: $ => choice(
      $.string,
      $.integer,
      $.float,
      $.boolean,
      $.none,
      $.identifier,
      $.list_expression,
      $.dict_expression,
      $.parenthesized_expression,
    ),

    parenthesized_expression: $ => seq('(', $._expression, ')'),

    list_expression: $ => seq(
      '[',
      optional(seq(
        $._expression,
        repeat(seq(',', $._expression)),
        optional(','),
      )),
      ']',
    ),

    dict_expression: $ => seq(
      '{',
      optional(seq(
        $.dict_pair,
        repeat(seq(',', $.dict_pair)),
        optional(','),
      )),
      '}',
    ),

    dict_pair: $ => seq(
      field('key', $._expression),
      ':',
      field('value', $._expression),
    ),

    // =========================================================================
    // Argument list (for calls)
    // =========================================================================

    argument_list: $ => seq(
      '(',
      optional(seq(
        $.argument,
        repeat(seq(',', $.argument)),
        optional(','),
      )),
      ')',
    ),

    argument: $ => choice(
      seq(field('name', $.identifier), '=', field('value', $._expression)),
      seq('*', field('splat', $._expression)),
      seq('**', field('double_splat', $._expression)),
      field('value', $._expression),
    ),

    // =========================================================================
    // Literals
    // =========================================================================

    string: _ => token(choice(
      seq('"', repeat(choice(/[^"\\]/, /\\./)), '"'),
      seq("'", repeat(choice(/[^'\\]/, /\\./)), "'"),
    )),

    integer: _ => token(/\d+/),

    float: _ => token(choice(
      /\d+\.\d*/,
      /\.\d+/,
      /\d+[eE][+-]?\d+/,
      /\d+\.\d*[eE][+-]?\d+/,
      /\.\d+[eE][+-]?\d+/,
    )),

    // Jinja2 accepts both Python-style and JS-style boolean/none literals
    boolean: _ => choice('true', 'false', 'True', 'False'),

    none: _ => choice('none', 'None', 'null'),

    identifier: _ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    recursive: _ => 'recursive',
  },
});
