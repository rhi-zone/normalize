import { EventEmitter } from 'events';
import path from 'path';
import { readFileSync, writeFileSync } from 'fs';

class Stack extends EventEmitter {
    #items = [];

    constructor(name) {
        super();
        this.name = name;
    }

    push(item) {
        this.#items.push(item);
        this.emit('push', item);
        return this;
    }

    pop() {
        if (this.isEmpty()) {
            return undefined;
        }
        const item = this.#items.pop();
        this.emit('pop', item);
        return item;
    }

    isEmpty() {
        return this.#items.length === 0;
    }

    size() {
        return this.#items.length;
    }
}

function classify(n) {
    if (n < 0) {
        return 'negative';
    } else if (n === 0) {
        return 'zero';
    } else {
        return 'positive';
    }
}

const sumArray = (nums) => {
    let total = 0;
    for (const n of nums) {
        total += n;
    }
    return total;
};

function fibonacci(n) {
    if (n <= 1) return n;
    let a = 0, b = 1;
    for (let i = 2; i <= n; i++) {
        [a, b] = [b, a + b];
    }
    return b;
}

const stack = new Stack('demo');
stack.push(1).push(2).push(3);
console.log(classify(-1));
console.log(sumArray([1, 2, 3, 4, 5]));
console.log(fibonacci(10));
const resolved = path.resolve('./sample.js');
console.log(resolved);
