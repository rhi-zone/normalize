# enumerate — idiomatic
for i, item in enumerate(items):
    print(i, item)

# range with explicit count — fine
for i in range(10):
    print(i)

# range with two arguments — not flagged
for i in range(0, 10):
    print(i)

# range(len()) nested differently — not flagged
n = len(items)
for i in range(n):
    print(i)
