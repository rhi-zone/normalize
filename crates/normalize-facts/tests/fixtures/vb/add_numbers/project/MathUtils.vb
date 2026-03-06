Module MathUtils

    Function Add(a As Integer, b As Integer) As Integer
        Return a + b
    End Function

    Function Multiply(a As Integer, b As Integer) As Integer
        Return a * b
    End Function

End Module

Class Calculator

    Private history As New List(Of Integer)

    Function Compute(op As String, a As Integer, b As Integer) As Integer
        Dim result As Integer
        If op = "add" Then
            result = Add(a, b)
        Else
            result = Multiply(a, b)
        End If
        history.Add(result)
        Return result
    End Function

End Class
