require 'json'
require 'set'

# A simple stack data structure
class Stack
  def initialize
    @data = []
  end

  def push(item)
    @data.push(item)
    self
  end

  def pop
    if @data.empty?
      raise "Stack is empty"
    end
    @data.pop
  end

  def peek
    @data.last
  end

  def empty?
    @data.empty?
  end

  def size
    @data.size
  end
end

# Classify a number as negative, zero, or positive
def classify(n)
  if n < 0
    :negative
  elsif n == 0
    :zero
  else
    :positive
  end
end

# Sum elements in an array that satisfy a predicate
def sum_if(arr, &block)
  total = 0
  arr.each do |x|
    total += x if block.call(x)
  end
  total
end

stack = Stack.new
stack.push(1).push(2).push(3)
puts stack.pop
puts classify(-5)
puts sum_if([1, 2, 3, 4, 5]) { |x| x.even? }
