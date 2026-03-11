# Correct: explicit exception chaining
try:
    parse(data)
except ValueError as e:
    raise RuntimeError("parsing failed") from e

# Correct: explicitly suppress context
try:
    connect(host)
except ConnectionError:
    raise ServiceError("clean message") from None

# Correct: bare re-raise (preserves original exception)
try:
    risky()
except Exception:
    log.error("failed")
    raise

# Raise outside try/except — not flagged
raise ValueError("bad input")
