with open("data.txt") as f:
    data = f.read()

with open("log.txt", "w") as log:
    log.write("hello")

# Not open() calls
x = close("something")
y = read("file")
