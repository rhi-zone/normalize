defmodule MathUtils do
  alias Enum, as: E

  @doc "Classify a number as :negative, :zero, or :positive"
  def classify(n) do
    cond do
      n < 0 -> :negative
      n == 0 -> :zero
      true -> :positive
    end
  end

  @doc "Sum elements matching the predicate"
  def sum_if(list, predicate) do
    Enum.reduce(list, 0, fn x, acc ->
      if predicate.(x), do: acc + x, else: acc
    end)
  end

  def sum_evens(numbers) do
    sum_if(numbers, fn n -> rem(n, 2) == 0 end)
  end
end

defmodule Stack do
  import Enum, only: [reverse: 1]

  defstruct items: []

  def new(), do: %Stack{}

  def push(%Stack{items: items}, item) do
    %Stack{items: [item | items]}
  end

  def pop(%Stack{items: []}) do
    {:error, :empty}
  end

  def pop(%Stack{items: [head | tail]}) do
    {:ok, head, %Stack{items: tail}}
  end

  def peek(%Stack{items: []}), do: nil
  def peek(%Stack{items: [head | _]}), do: head

  def to_list(%Stack{items: items}), do: reverse(items)
end
