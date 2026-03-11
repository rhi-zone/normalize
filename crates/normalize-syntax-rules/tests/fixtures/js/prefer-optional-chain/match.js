// Guard then member access — same identifier
const name = user && user.name;

// Guard then method call
const result = handler && handler.process();

// Guard then call of same identifier
const out = callback && callback();

// Nested guard chains
const city = user && user.address && user.address.city;
