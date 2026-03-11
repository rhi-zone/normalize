# String interpolation — idiomatic Ruby

greeting = "Hello, #{name}"
full_name = "#{first_name} #{last_name}"
path = "#{base_dir}/#{filename}"
message = "Error: #{error.message}"

# Non-string concatenation — not flagged
total = count + extra
numbers = [1] + [2, 3]

# Appending to buffer with << — not flagged
buf = ""
buf << "prefix"
buf << value
