<?php

use App\Models\User;
use Illuminate\Support\Collection;

class Stack {
    private array $items = [];

    public function push(mixed $item): void {
        array_push($this->items, $item);
    }

    public function pop(): mixed {
        if (empty($this->items)) {
            throw new \UnderflowException("Stack is empty");
        }
        return array_pop($this->items);
    }

    public function peek(): mixed {
        if (empty($this->items)) {
            return null;
        }
        return end($this->items);
    }

    public function isEmpty(): bool {
        return empty($this->items);
    }

    public function size(): int {
        return count($this->items);
    }
}

/**
 * Classify a number as negative, zero, or positive.
 */
#[Pure]
function classify(int $n): string {
    if ($n < 0) {
        return "negative";
    } elseif ($n === 0) {
        return "zero";
    } else {
        return "positive";
    }
}

function sumEvens(array $numbers): int {
    $total = 0;
    foreach ($numbers as $n) {
        if ($n % 2 === 0) {
            $total += $n;
        }
    }
    return $total;
}

$stack = new Stack();
$stack->push(1);
$stack->push(2);
echo $stack->pop() . "\n";
echo classify(-5) . "\n";
echo sumEvens([1, 2, 3, 4, 5]) . "\n";
