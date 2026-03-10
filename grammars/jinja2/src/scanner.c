/**
 * External scanner for the Jinja2 tree-sitter grammar.
 *
 * Handles two tokens that cannot be expressed as simple regexes:
 *
 *   content     — literal template text; advances until {%, {{, or {# is seen
 *                 (the opening of a template tag or comment).
 *
 *   raw_content — text inside {% raw %}...{% endraw %}; advances until the
 *                 literal byte sequence "{%" is seen (the endraw closing tag
 *                 is matched by the grammar after the scanner yields).
 *
 * The grammar declares both in its `externals` array in this order, so
 * valid_symbols[0] == content, valid_symbols[1] == raw_content.
 */

#include "tree_sitter/parser.h"
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

typedef enum {
  TOKEN_CONTENT = 0,
  TOKEN_RAW_CONTENT = 1,
} TokenType;

/* No per-parse state needed. */
void *tree_sitter_jinja2_external_scanner_create(void) { return NULL; }
void  tree_sitter_jinja2_external_scanner_destroy(void *p) { (void)p; }
void  tree_sitter_jinja2_external_scanner_reset(void *p) { (void)p; }
unsigned tree_sitter_jinja2_external_scanner_serialize(void *p, char *buf) {
  (void)p; (void)buf; return 0;
}
void tree_sitter_jinja2_external_scanner_deserialize(void *p, const char *buf, unsigned n) {
  (void)p; (void)buf; (void)n;
}

bool tree_sitter_jinja2_external_scanner_scan(
    void *payload,
    TSLexer *lexer,
    const bool *valid_symbols)
{
  (void)payload;

  /* ── raw_content ─────────────────────────────────────────────────────── */
  if (valid_symbols[TOKEN_RAW_CONTENT]) {
    bool consumed = false;
    while (!lexer->eof(lexer)) {
      if (lexer->lookahead == '{') {
        /* Mark the end of accepted content BEFORE the '{'. */
        lexer->mark_end(lexer);
        lexer->advance(lexer, false);
        if (lexer->lookahead == '%') {
          /* Possible {% endraw %} — stop here; grammar will check. */
          if (consumed) {
            lexer->result_symbol = TOKEN_RAW_CONTENT;
            return true;
          }
          return false;
        }
        /* Was '{' but not '{%' — still raw content. */
        consumed = true;
        continue;
      }
      lexer->advance(lexer, false);
      consumed = true;
    }
    if (consumed) {
      lexer->mark_end(lexer);
      lexer->result_symbol = TOKEN_RAW_CONTENT;
      return true;
    }
    return false;
  }

  /* ── content ──────────────────────────────────────────────────────────── */
  if (valid_symbols[TOKEN_CONTENT]) {
    bool consumed = false;
    while (!lexer->eof(lexer)) {
      if (lexer->lookahead == '{') {
        /* Mark the end of accepted content BEFORE the '{'. */
        lexer->mark_end(lexer);
        lexer->advance(lexer, false);
        /* Check next character: %, {, or # start a template tag. */
        if (lexer->lookahead == '%' ||
            lexer->lookahead == '{' ||
            lexer->lookahead == '#') {
          if (consumed) {
            lexer->result_symbol = TOKEN_CONTENT;
            return true;
          }
          return false;
        }
        /* Just a lone '{' in literal text — continue. */
        consumed = true;
        continue;
      }
      lexer->advance(lexer, false);
      consumed = true;
    }
    /* EOF — emit remaining content. */
    if (consumed) {
      lexer->mark_end(lexer);
      lexer->result_symbol = TOKEN_CONTENT;
      return true;
    }
    return false;
  }

  return false;
}
