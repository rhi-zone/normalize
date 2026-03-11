// Non-null assertions that should be flagged

const el = document.getElementById("app")!;
const text = el!.textContent;

function getUser(id: string): User | null {
  return users.get(id) ?? null;
}

const user = getUser("123")!;
const name = user!.name;

// Chained non-null assertions
const value = obj!.prop!.nested;
