// Correct: borrowed from Object.prototype via .call()
if (Object.prototype.hasOwnProperty.call(obj, 'key')) {
  console.log('has key');
}

// Modern ES2022 alternative
if (Object.hasOwn(obj, 'key')) {
  console.log('has key');
}

// isPrototypeOf via .call()
if (Object.prototype.isPrototypeOf.call(proto, obj)) {
  console.log('is prototype');
}

// propertyIsEnumerable via .call()
if (Object.prototype.propertyIsEnumerable.call(obj, 'prop')) {
  console.log('enumerable');
}

// Unrelated method calls — no false positives
obj.toString();
obj.valueOf();
arr.includes('item');
