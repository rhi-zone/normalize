<?php
function foo() {
    try { doStuff(); }
    catch (Exception $e) { log($e); }
}
