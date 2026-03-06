require "json"
require_relative "math_helpers"

class Calculator
  attr_reader :history

  def initialize(name)
    @name = name
    @history = []
  end

  def add(a, b)
    result = MathHelpers.add(a, b)
    @history.push(result)
    result
  end

  def multiply(a, b)
    result = MathHelpers.multiply(a, b)
    @history.push(result)
    result
  end

  def to_s
    "Calculator(#{@name})"
  end
end
