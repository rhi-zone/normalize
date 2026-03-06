class MyLibrary
  def process(data)
    @logger.info("Processing #{data}")
    result = data.upcase
    result
  end

  def error_case
    raise ArgumentError, "invalid input"
  end
end
