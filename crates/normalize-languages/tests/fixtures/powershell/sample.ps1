Import-Module PSReadLine
Import-Module Microsoft.PowerShell.Utility

class Calculator {
    [int]$Precision

    Calculator([int]$precision) {
        $this.Precision = $precision
    }

    [double] Add([double]$a, [double]$b) {
        return [Math]::Round($a + $b, $this.Precision)
    }

    [double] Multiply([double]$a, [double]$b) {
        return [Math]::Round($a * $b, $this.Precision)
    }
}

function Invoke-Classify {
    param(
        [Parameter(Mandatory)]
        [int]$Number
    )
    if ($Number -lt 0) {
        return "negative"
    } elseif ($Number -eq 0) {
        return "zero"
    } else {
        return "positive"
    }
}

function Get-Sum {
    param([int[]]$Numbers)
    $total = 0
    foreach ($n in $Numbers) {
        $total += $n
    }
    return $total
}

function Get-Factorial {
    param([int]$N)
    if ($N -le 1) { return 1 }
    $result = 1
    for ($i = 2; $i -le $N; $i++) {
        $result *= $i
    }
    return $result
}

$calc = [Calculator]::new(2)
Write-Host "Add: $($calc.Add(3.5, 2.1))"
Write-Host "Classify: $(Invoke-Classify -Number -5)"
Write-Host "Sum: $(Get-Sum -Numbers 1,2,3,4,5)"
Write-Host "Factorial: $(Get-Factorial -N 5)"
