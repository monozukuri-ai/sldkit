# Syntax and mock checks only; no SolidWorks process is created.
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$source = Join-Path $PSScriptRoot "capture_assembly_transforms.ps1"
$tokens = $null
$parseErrors = $null
$ast = [Management.Automation.Language.Parser]::ParseFile($source, [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count -ne 0) { throw ($parseErrors | Out-String) }
$names = @("Get-FiniteArray", "Get-TransformFacts", "Get-RelativePath")
foreach ($definition in $ast.FindAll({
            param($node)
            $node -is [Management.Automation.Language.FunctionDefinitionAst] -and
            $names -contains $node.Name
        }, $true)) {
    Invoke-Expression $definition.Extent.Text
}
$utility = [pscustomobject]@{}
$utility | Add-Member ScriptMethod CreatePoint {
    param($coordinates)
    $point = [pscustomobject]@{ ArrayData = @($coordinates) }
    $point | Add-Member ScriptMethod MultiplyTransform {
        param($transform)
        [pscustomobject]@{
            ArrayData = @(
                $this.ArrayData[0] + $transform.ArrayData[9]
                $this.ArrayData[1] + $transform.ArrayData[10]
                $this.ArrayData[2] + $transform.ArrayData[11]
            )
        }
    }
    $point
}
$component = [pscustomobject]@{
    Transform2 = [pscustomobject]@{ ArrayData = @(1,0,0,0,1,0,0,0,1,0.01,0.02,0.03,1,0,0,0) }
}
$facts = Get-TransformFacts $component $utility
if ($facts.status -ne "captured" -or $facts.array_data.Count -ne 16 -or
        $facts.probes.Count -ne 4 -or $facts.probes[0].root_m[2] -ne 0.03 -or
        [math]::Abs($facts.probes[1].root_m[0] - 0.011) -gt 1e-12) {
    throw "SDK point probes were not retained correctly"
}
$json = $facts | ConvertTo-Json -Depth 10 | ConvertFrom-Json
if ($json.probes[2].local_m.Count -ne 3) { throw "probe JSON array shape changed" }
foreach ($invalid in @($null, @(1, 2), @(1, [double]::NaN, 3), @(1, $null, 3))) {
    $rejected = $false
    try { $null = Get-FiniteArray $invalid 3 } catch { $rejected = $true }
    if (-not $rejected) { throw "invalid numeric evidence was accepted" }
}
$component.Transform2 = $null
if ((Get-TransformFacts $component $utility).status -ne "unavailable") {
    throw "missing transform must remain unavailable"
}
Write-Output "assembly capture syntax and mocked SDK probe checks passed"
