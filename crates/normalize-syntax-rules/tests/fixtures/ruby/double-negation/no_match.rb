# Single negation — fine
not_valid = !value

# Direct boolean comparisons — fine
active = user.active? == true
present = !value.nil?

# Boolean method — fine
exists = value.present?

# Normal arithmetic negation — fine
x = -count
