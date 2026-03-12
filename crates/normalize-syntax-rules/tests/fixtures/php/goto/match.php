<?php
function foo() {
    goto label;
    label:
    doStuff();
}
