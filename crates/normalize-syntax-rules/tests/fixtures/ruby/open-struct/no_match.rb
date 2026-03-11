# Struct — fast, explicit
Person = Struct.new(:name, :age, keyword_init: true)

# Data (Ruby 3.2+) — immutable value object
Point = Data.define(:x, :y)

# Regular class instantiation — fine
result = MyClass.new(x: 1)
obj = SomeModule::Thing.new
