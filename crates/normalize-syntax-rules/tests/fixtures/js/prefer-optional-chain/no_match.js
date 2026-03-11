// Already using optional chaining — no violation
const name = user?.name;
const result = handler?.process();
const out = callback?.();

// Guard with different identifiers — not the same variable
const a = foo && bar.baz;

// Boolean short-circuit with unrelated operands
const x = isEnabled && doSomething();

// Logical AND with non-member right side
const val = items && items.length > 0;
