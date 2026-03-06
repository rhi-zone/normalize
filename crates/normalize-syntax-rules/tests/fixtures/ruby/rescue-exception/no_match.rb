def safe
  do_something
rescue StandardError => e
  log(e)
end

def also_safe
  do_something
rescue RuntimeError
  retry
end
