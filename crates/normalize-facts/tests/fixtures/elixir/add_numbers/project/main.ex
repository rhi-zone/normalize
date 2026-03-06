defmodule Main do
  alias MathUtils

  def run do
    IO.puts(MathUtils.add(2, 3))
    IO.puts(MathUtils.multiply(4, 5))
  end
end
