using System;
using System.Collections.Generic;

namespace SampleApp
{
    public class Stack<T>
    {
        private List<T> items = new List<T>();

        public void Push(T item)
        {
            items.Add(item);
        }

        public T Pop()
        {
            if (items.Count == 0)
            {
                throw new InvalidOperationException("Stack is empty");
            }
            T top = items[items.Count - 1];
            items.RemoveAt(items.Count - 1);
            return top;
        }

        public T Peek()
        {
            if (items.Count == 0)
            {
                throw new InvalidOperationException("Stack is empty");
            }
            return items[items.Count - 1];
        }

        public bool IsEmpty => items.Count == 0;
        public int Count => items.Count;
    }

    /// <summary>Utility math functions.</summary>
    [Obsolete("Use MathHelper instead")]
    public static class MathUtils
    {
        public static string Classify(int n)
        {
            if (n < 0)
                return "negative";
            else if (n == 0)
                return "zero";
            else
                return "positive";
        }

        public static int SumEvens(IEnumerable<int> numbers)
        {
            int total = 0;
            foreach (int n in numbers)
            {
                if (n % 2 == 0)
                    total += n;
            }
            return total;
        }
    }

    class Program
    {
        static void Main(string[] args)
        {
            var stack = new Stack<int>();
            stack.Push(1);
            stack.Push(2);
            Console.WriteLine(stack.Pop());
            Console.WriteLine(MathUtils.Classify(-5));
            Console.WriteLine(MathUtils.SumEvens(new[] { 1, 2, 3, 4, 5 }));
        }
    }
}
