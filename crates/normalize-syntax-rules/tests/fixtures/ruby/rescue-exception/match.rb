def risky
  do_something
rescue Exception => e
  log(e)
end
