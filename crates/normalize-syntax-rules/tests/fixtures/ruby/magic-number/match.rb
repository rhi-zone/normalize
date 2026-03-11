def check(x, retries)
  if x > 42
    nil
  end
  if retries >= 10
    nil
  end
  if x < 3600
    nil
  end
  if 100 <= x
    nil
  end
  if x == 255
    nil
  end
  if x != 1024
    nil
  end
end
