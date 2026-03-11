// No type annotation — inferred
const name = "Alice";
let count = 0;
const flag = true;

// Non-literal value — type annotation may be useful
const value: string = computeValue();
const result: number = someFunction();

// Complex types — annotation is useful
const items: string[] = [];
const map: Record<string, number> = {};

// Union types — annotation is useful
let status: "active" | "inactive" = "active";
