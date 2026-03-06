using System;
using AddNumbers;

namespace AddNumbers
{
    class Program
    {
        static void Main(string[] args)
        {
            int sum = MathUtils.Add(2, 3);
            int product = MathUtils.Multiply(4, 5);
            Console.WriteLine(sum);
            Console.WriteLine(product);
        }
    }
}
