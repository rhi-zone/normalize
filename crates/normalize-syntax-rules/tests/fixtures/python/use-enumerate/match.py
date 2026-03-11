# range(len(...)) — use enumerate() instead

items = ['a', 'b', 'c']
for i in range(len(items)):
    print(i, items[i])

data = [1, 2, 3]
for idx in range(len(data)):
    data[idx] = data[idx] * 2
