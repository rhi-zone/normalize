; Svelte CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium Svelte grammar node types.
;
; Svelte has template-level control flow via {#if}, {#each}, {#await}.
; JavaScript logic inside <script> blocks would be handled by the JS grammar.

; ---------------------------------------------------------------------------
; {#if} / {:else if} / {:else} (branch)
; ---------------------------------------------------------------------------

(if_statement
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  (else_if_block) @cfg.branch.else
) @cfg.branch

(if_statement
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  (else_block
    body: (_) @cfg.branch.else)
) @cfg.branch

(if_statement
  condition: (_) @cfg.branch.condition
  consequence: (_) @cfg.branch.then
  .
) @cfg.branch

; ---------------------------------------------------------------------------
; {#each} (loop over collection)
; ---------------------------------------------------------------------------

(each_statement
  (expression) @cfg.loop.condition
  body: (_) @cfg.loop.body
) @cfg.loop

(each_statement
  body: (_) @cfg.loop.body
) @cfg.loop
