// Non-empty interface — fine
interface User {
  name: string;
  age: number;
}

// Interface with one member — fine
interface Printable {
  toString(): string;
}

// Type alias — not flagged by this rule
type Empty = {};

// Interface with only index signature — fine
interface StringMap {
  [key: string]: string;
}
