#requires -Version 5.1
<#
.SYNOPSIS
Capture independent assembly placement evidence, without saving the source.
.DESCRIPTION
Run on Windows with SolidWorks installed and no existing SolidWorks session.
Raw Transform2.ArrayData and SDK-transformed basis points are retained together.
This does not interpret the native swTransform XML attribute.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ProjectRoot,
    [Parameter(Mandatory = $true)][string]$AssemblyPath,
    [Parameter(Mandatory = $true)][string]$OutputPath,
    [string]$Configuration,
    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-RelativePath {
    param([string]$Path, [string]$Root)
    if ([string]::IsNullOrWhiteSpace($Path)) { return $null }
    $full = [IO.Path]::GetFullPath($Path)
    $prefix = $Root.TrimEnd([char[]]"\/") + [IO.Path]::DirectorySeparatorChar
    if (-not $full.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        return $null
    }
    return $full.Substring($prefix.Length).Replace("\", "/")
}

function Get-FiniteArray {
    param([object]$Value, [int]$Count)
    $items = @($Value)
    if ($items.Count -ne $Count) { throw "unexpected numeric array length" }
    foreach ($item in $items) {
        if ($null -eq $item) { throw "missing numeric value" }
        $number = [double]$item
        if ([double]::IsNaN($number) -or [double]::IsInfinity($number)) {
            throw "non-finite numeric value"
        }
        $number
    }
}

function Get-TransformFacts {
    param([object]$Component, [object]$MathUtility)
    try {
        $transform = $Component.Transform2
        if ($null -eq $transform) { throw "transform unavailable" }
        $raw = @(Get-FiniteArray $transform.ArrayData 16)
        $probes = New-Object Collections.ArrayList
        # The SDK evaluates the mapping, so no matrix layout is assumed here.
        foreach ($coordinates in @(
                @(0.0, 0.0, 0.0), @(0.001, 0.0, 0.0),
                @(0.0, 0.001, 0.0), @(0.0, 0.0, 0.001)
            )) {
            $point = $MathUtility.CreatePoint([double[]]$coordinates)
            $mapped = $point.MultiplyTransform($transform)
            if ($null -eq $mapped) { throw "SDK point transform unavailable" }
            [void]$probes.Add([ordered]@{
                    local_m = @($coordinates)
                    root_m = @(Get-FiniteArray $mapped.ArrayData 3)
                })
        }
        return [ordered]@{
            status = "captured"
            array_data = $raw
            probes = @($probes)
        }
    } catch {
        # COM exceptions can contain machine-local paths; do not serialize them.
        return [ordered]@{
            status = "unavailable"
            array_data = $null
            probes = @()
        }
    }
}

function Add-Occurrence {
    param(
        [object]$Component, [AllowNull()][object]$ParentId, [int]$Depth,
        [string]$Root, [object]$MathUtility, [Collections.ArrayList]$Output
    )
    if ($Depth -gt 128 -or $Output.Count -ge 100000) {
        throw "assembly traversal limit exceeded"
    }
    $id = $Output.Count
    $storedPath = [string]$Component.GetPathName()
    $relative = Get-RelativePath $storedPath $Root
    $reference = [ordered]@{
        basename = [IO.Path]::GetFileName($storedPath)
        project_path = $relative
        sha256 = $null
    }
    if ($null -ne $relative -and (Test-Path -LiteralPath $storedPath -PathType Leaf)) {
        $reference.sha256 = (Get-FileHash -LiteralPath $storedPath -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    $entry = [ordered]@{
        id = $id
        parent_id = $ParentId
        # Name2 is already a full SDK occurrence name; do not prepend ancestors.
        name = [string]$Component.Name2
        referenced_configuration = [string]$Component.ReferencedConfiguration
        reference = $reference
        suppression_code = [int]$Component.GetSuppression2()
        visibility_code = [int]$Component.Visible
        transform = Get-TransformFacts $Component $MathUtility
        children_status = "enumerated"
    }
    [void]$Output.Add($entry)
    $children = @()
    try {
        $rawChildren = $Component.GetChildren()
        if ($null -ne $rawChildren) { $children = @($rawChildren) }
    } catch {
        $entry.children_status = "unavailable"
    }
    foreach ($child in $children) {
        Add-Occurrence $child $id ($Depth + 1) $Root $MathUtility $Output
    }
}

$root = [IO.Path]::GetFullPath($ProjectRoot)
if (-not (Test-Path -LiteralPath $root -PathType Container)) {
    throw "ProjectRoot must be an existing directory"
}
$inputPath = if ([IO.Path]::IsPathRooted($AssemblyPath)) {
    [IO.Path]::GetFullPath($AssemblyPath)
} else {
    [IO.Path]::GetFullPath((Join-Path $root $AssemblyPath))
}
$relativeInput = Get-RelativePath $inputPath $root
if ($null -eq $relativeInput -or
        [IO.Path]::GetExtension($inputPath).ToLowerInvariant() -ne ".sldasm" -or
        -not (Test-Path -LiteralPath $inputPath -PathType Leaf)) {
    throw "AssemblyPath must be a .SLDASM file inside ProjectRoot"
}
$output = [IO.Path]::GetFullPath($OutputPath)
if ([IO.Path]::GetExtension($output).ToLowerInvariant() -ne ".json") {
    throw "OutputPath must have a .json extension"
}
if ((Test-Path -LiteralPath $output) -and -not $Force) {
    throw "OutputPath exists; use -Force to replace the capture"
}
if (@(Get-Process -Name SLDWORKS -ErrorAction SilentlyContinue).Count -ne 0) {
    throw "Close existing SolidWorks sessions before running this capture"
}
$beforeHash = (Get-FileHash -LiteralPath $inputPath -Algorithm SHA256).Hash.ToLowerInvariant()
$application = $null
$model = $null
try {
    $application = New-Object -ComObject SldWorks.Application
    $application.Visible = $false
    [int]$openErrors = 0
    [int]$openWarnings = 0
    # swDocASSEMBLY=2; swOpenDocOptions_Silent | ReadOnly=3.
    $model = $application.OpenDoc6($inputPath, 2, 3, "", [ref]$openErrors, [ref]$openWarnings)
    if ($null -eq $model) { throw "OpenDoc6 returned no document (code $openErrors)" }
    $savedConfiguration = [string]$model.ConfigurationManager.ActiveConfiguration.Name
    if (-not [string]::IsNullOrWhiteSpace($Configuration)) {
        if (-not $model.ShowConfiguration2($Configuration)) {
            throw "Requested configuration could not be activated"
        }
    }
    $active = $model.ConfigurationManager.ActiveConfiguration
    $rootComponent = $active.GetRootComponent3($true)
    if ($null -eq $rootComponent) { throw "Assembly root component unavailable" }
    $math = $application.GetMathUtility()
    $entries = New-Object Collections.ArrayList
    $children = $rootComponent.GetChildren()
    if ($null -ne $children) {
        foreach ($child in @($children)) {
            Add-Occurrence $child $null 0 $root $math $entries
        }
    }
    $afterHash = (Get-FileHash -LiteralPath $inputPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($beforeHash -ne $afterHash) { throw "Source bytes changed during capture" }
    $record = [ordered]@{
        schema_version = 1
        capture_tool = "capture_assembly_transforms.ps1"
        captured_at_utc = [DateTime]::UtcNow.ToString("o")
        solidworks_revision = [string]$application.RevisionNumber()
        source = [ordered]@{
            path = $relativeInput
            sha256 = $beforeHash
            byte_size = (Get-Item -LiteralPath $inputPath).Length
            open_errors = $openErrors
            open_warnings = $openWarnings
            saved_configuration = $savedConfiguration
        }
        configuration = [string]$active.Name
        length_unit = "meter"
        transform_api = "IComponent2.Transform2"
        probe_api = "IMathPoint.MultiplyTransform"
        occurrences = @($entries)
    }
    [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($output))
    $json = $record | ConvertTo-Json -Depth 30
    [IO.File]::WriteAllText($output, "$json`n", [Text.UTF8Encoding]::new($false))
    Write-Output "wrote assembly transform evidence: $output"
} finally {
    try {
        if ($null -ne $model) { [void]$application.CloseDoc([string]$model.GetTitle()) }
    } finally {
        if ($null -ne $application) {
            try { [void]$application.ExitApp() }
            finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($application) }
        }
    }
}
