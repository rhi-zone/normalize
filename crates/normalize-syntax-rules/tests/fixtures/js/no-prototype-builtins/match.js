// Calling hasOwnProperty directly on an object
if (obj.hasOwnProperty('key')) {
  console.log('has key');
}

// isPrototypeOf called directly
if (proto.isPrototypeOf(obj)) {
  console.log('is prototype');
}

// propertyIsEnumerable called directly
if (obj.propertyIsEnumerable('prop')) {
  console.log('enumerable');
}

// hasOwnProperty on a dynamically-looked-up object
function checkKey(data, key) {
  return data.hasOwnProperty(key);
}
