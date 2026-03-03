/**
 * Simple math utilities.
 */

function add(a, b) {
  return a + b;
}

function multiply(a, b) {
  return a * b;
}

class Calculator {
  constructor() {
    this.history = [];
  }

  compute(op, a, b) {
    const result = op === "add" ? add(a, b) : multiply(a, b);
    this.history.push(result);
    return result;
  }
}

module.exports = { add, multiply, Calculator };
