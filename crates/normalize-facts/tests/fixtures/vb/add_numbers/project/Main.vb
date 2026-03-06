Imports System

Module Program

    Sub Main()
        Dim calc As New Calculator()
        Console.WriteLine(calc.Compute("add", 2, 3))
        Console.WriteLine(calc.Compute("mul", 4, 5))
    End Sub

End Module
