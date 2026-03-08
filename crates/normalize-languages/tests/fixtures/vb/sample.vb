Imports System
Imports System.Collections.Generic
Imports System.Linq

Module MathUtils
    Function Square(n As Double) As Double
        Return n * n
    End Function

    Function Classify(n As Integer) As String
        If n < 0 Then
            Return "negative"
        ElseIf n = 0 Then
            Return "zero"
        Else
            Return "positive"
        End If
    End Function

    Function SumEvens(values As List(Of Integer)) As Integer
        Dim total As Integer = 0
        For Each v In values
            If v Mod 2 = 0 Then
                total += v
            End If
        Next
        Return total
    End Function
End Module

Class Shape
    Public Property Name As String

    Public Sub New(name As String)
        Me.Name = name
    End Sub

    Public Overridable Function Area() As Double
        Return 0.0
    End Function
End Class

Class Circle
    Inherits Shape

    Public Property Radius As Double

    Public Sub New(radius As Double)
        MyBase.New("circle")
        Me.Radius = radius
    End Sub

    Public Overrides Function Area() As Double
        Return Math.PI * Radius * Radius
    End Function
End Class

Module Program
    Sub Main()
        Dim c As New Circle(5.0)
        Console.WriteLine(c.Area())
        Console.WriteLine(MathUtils.Classify(-3))
        Dim nums As New List(Of Integer) From {1, 2, 3, 4, 5, 6}
        Console.WriteLine(MathUtils.SumEvens(nums))
    End Sub
End Module
