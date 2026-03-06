<?php

namespace App\Math;

use App\Logging\Logger;
use InvalidArgumentException;

interface MathOperation {
    public function execute(int $a, int $b): int;
}

function add(int $a, int $b): int {
    return $a + $b;
}

class Calculator implements MathOperation {
    private array $history;
    private Logger $logger;

    public function __construct(Logger $logger) {
        $this->history = [];
        $this->logger = $logger;
    }

    public function execute(int $a, int $b): int {
        $result = add($a, $b);
        $this->history[] = $result;
        $this->logger->log("Result: $result");
        return $result;
    }

    public function multiply(int $a, int $b): int {
        $result = $a * $b;
        $this->history[] = $result;
        return $result;
    }

    public function getHistory(): array {
        return $this->history;
    }
}
