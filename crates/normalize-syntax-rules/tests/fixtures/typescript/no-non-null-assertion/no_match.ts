// Proper null narrowing — no non-null assertions

const el = document.getElementById("app");
if (el === null) {
  throw new Error("element #app not found");
}
const text = el.textContent;

function getUser(id: string): User | null {
  return users.get(id) ?? null;
}

const user = getUser("123");
if (user === undefined) {
  throw new Error(`user ${id} not found`);
}
const name = user.name;

// Optional chaining is fine
const maybeText = document.getElementById("app")?.textContent ?? "";
