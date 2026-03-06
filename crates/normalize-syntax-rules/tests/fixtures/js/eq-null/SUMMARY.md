# fixtures/js/eq-null

Test fixtures for the `js/eq-null` syntax rule. `match.js` contains `== null` and `!= null` loose equality checks that the rule should flag. `no_match.js` uses strict equality (`===`/`!==`) which should produce no findings.
