#requires -Version 5.1

<#
.SYNOPSIS
Captures path-free SolidWorks API facts for one controlled Drawing fixture.

.DESCRIPTION
Run with Windows PowerShell on a Windows host with SolidWorks installed.
The script refuses an already-running SolidWorks session, opens the Drawing
silent and read-only, and writes only project-relative paths or basenames.
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ProjectRoot,

    [Parameter(Mandatory = $true)]
    [string]$DrawingPath,

    [Parameter(Mandatory = $true)]
    [string]$OutputPath,

    [string]$FixtureId = "m6a-drawing-controlled",

    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$DocumentExtensions = @(".sldprt", ".sldasm", ".slddrw")

function Convert-ToObjectArray {
    param([AllowNull()][object]$Value)

    if ($null -eq $Value) {
        return @()
    }
    return @($Value)
}

function Convert-ToDoubleArray {
    param(
        [AllowNull()][object]$Value,
        [int]$ExpectedCount,
        [string]$Field
    )

    $items = @(Convert-ToObjectArray -Value $Value)
    if ($items.Count -ne $ExpectedCount) {
        throw "$Field returned $($items.Count) values; expected $ExpectedCount"
    }
    $result = New-Object Collections.ArrayList
    foreach ($item in $items) {
        [void]$result.Add([double]$item)
    }
    return @($result)
}

function Get-ProjectRelativePath {
    param(
        [string]$Path,
        [string]$NormalizedProjectRoot
    )

    $fullPath = [IO.Path]::GetFullPath($Path)
    $separator = [IO.Path]::DirectorySeparatorChar
    $prefix = $NormalizedProjectRoot.TrimEnd([char[]]"\/") + $separator
    if (-not $fullPath.StartsWith(
            $prefix,
            [StringComparison]::OrdinalIgnoreCase
        )) {
        return $null
    }
    return $fullPath.Substring($prefix.Length).Replace("\", "/")
}

function Resolve-ProjectReference {
    param(
        [AllowNull()][string]$RawPath,
        [string]$NormalizedProjectRoot,
        [hashtable]$DocumentIndex
    )

    if ([string]::IsNullOrWhiteSpace($RawPath)) {
        return [ordered]@{
            stored_basename = $null
            resolved_project_path = $null
            resolution_status = "no_reference"
        }
    }

    $basename = [IO.Path]::GetFileName($RawPath)
    $relative = $null
    if ([IO.Path]::IsPathRooted($RawPath)) {
        $relative = Get-ProjectRelativePath $RawPath $NormalizedProjectRoot
    }
    if ($null -ne $relative -and (Test-Path -LiteralPath $RawPath -PathType Leaf)) {
        return [ordered]@{
            stored_basename = $basename
            resolved_project_path = $relative
            resolution_status = "resolved"
        }
    }

    $candidates = @()
    $key = $basename.ToLowerInvariant()
    if ($DocumentIndex.ContainsKey($key)) {
        $candidates = @($DocumentIndex[$key])
    }
    if ($candidates.Count -eq 1) {
        return [ordered]@{
            stored_basename = $basename
            resolved_project_path = $candidates[0]
            resolution_status = "resolved"
        }
    }
    return [ordered]@{
        stored_basename = $basename
        resolved_project_path = $null
        resolution_status = $(if ($candidates.Count -gt 1) {
                "ambiguous"
            } else {
                "missing"
            })
    }
}

function Get-DrawingViewTypeName {
    param([int]$Code)

    switch ($Code) {
        1 { return "sheet" }
        2 { return "section" }
        3 { return "detail" }
        4 { return "projected" }
        5 { return "auxiliary" }
        6 { return "standard" }
        7 { return "named" }
        8 { return "relative" }
        9 { return "detached" }
        10 { return "alternate_position" }
        default { return "unknown_$Code" }
    }
}

function Get-LicenseTypeName {
    param([int]$Code)

    switch ($Code) {
        0 { return "full" }
        1 { return "educational" }
        2 { return "student" }
        3 { return "student_design_kit" }
        4 { return "personal_edition" }
        5 { return "full_office" }
        6 { return "full_professional" }
        7 { return "full_premium" }
        8 { return "maker" }
        9 { return "full_ultimate" }
        default { return "unknown_$Code" }
    }
}

function Get-ByteArraySha256 {
    param([byte[]]$Value)

    $algorithm = [Security.Cryptography.SHA256]::Create()
    try {
        $hash = $algorithm.ComputeHash($Value)
        return [BitConverter]::ToString($hash).Replace("-", "").ToLowerInvariant()
    } finally {
        $algorithm.Dispose()
    }
}

function Get-PersistentReferenceFacts {
    param(
        [object]$ModelExtension,
        [object]$Object
    )

    try {
        $raw = $ModelExtension.GetPersistReference3($Object)
        $items = @(Convert-ToObjectArray -Value $raw)
        if ($items.Count -eq 0) {
            throw "GetPersistReference3 returned an empty array"
        }
        [byte[]]$bytes = @($items | ForEach-Object { [byte]$_ })
        return [ordered]@{
            status = "captured"
            encoding = "base64"
            byte_size = $bytes.Length
            sha256 = Get-ByteArraySha256 $bytes
            value = [Convert]::ToBase64String($bytes)
        }
    } catch {
        return [ordered]@{
            status = "unavailable"
            encoding = $null
            byte_size = $null
            sha256 = $null
            value = $null
        }
    }
}

function Get-BaseViewFacts {
    param(
        [object]$View,
        [object]$ModelExtension
    )

    try {
        $baseView = $View.GetBaseView()
    } catch {
        return [ordered]@{
            status = "unavailable"
            name = $null
            persistent_reference = $null
        }
    }
    if ($null -eq $baseView) {
        return [ordered]@{
            status = "none"
            name = $null
            persistent_reference = $null
        }
    }
    return [ordered]@{
        status = "captured"
        name = [string]($baseView.GetName2())
        persistent_reference = Get-PersistentReferenceFacts `
            -ModelExtension $ModelExtension `
            -Object $baseView
    }
}

function Get-DrawingFacts {
    param(
        [object]$Drawing,
        [object]$ModelExtension,
        [string]$NormalizedProjectRoot,
        [hashtable]$DocumentIndex
    )

    $originalSheetName = $null
    $originalSheet = $Drawing.GetCurrentSheet()
    if ($null -ne $originalSheet) {
        $originalSheetName = [string]$originalSheet.GetName()
    }

    $sheets = New-Object Collections.ArrayList
    try {
        foreach ($sheetName in @(
                Convert-ToObjectArray -Value ($Drawing.GetSheetNames())
            )) {
            if (-not $Drawing.ActivateSheet([string]$sheetName)) {
                throw "failed to activate drawing sheet '$sheetName'"
            }
            $sheet = $Drawing.GetCurrentSheet()
            if ($null -eq $sheet) {
                throw "GetCurrentSheet returned null for '$sheetName'"
            }
            $sheetProperties = @(
                Convert-ToDoubleArray `
                    -Value ($sheet.GetProperties2()) `
                    -ExpectedCount 8 `
                    -Field "ISheet.GetProperties2"
            )
            $views = New-Object Collections.ArrayList
            foreach ($view in @(
                    Convert-ToObjectArray -Value ($sheet.GetViews())
                )) {
                $position = @(
                    Convert-ToDoubleArray `
                        -Value $view.Position `
                        -ExpectedCount 2 `
                        -Field "IView.Position"
                )
                $scaleRatio = @(
                    Convert-ToDoubleArray `
                        -Value $view.ScaleRatio `
                        -ExpectedCount 2 `
                        -Field "IView.ScaleRatio"
                )
                $viewTransform = @(
                    Convert-ToDoubleArray `
                        -Value ($view.GetViewXform()) `
                        -ExpectedCount 13 `
                        -Field "IView.GetViewXform"
                )
                [int]$viewType = [int]$view.Type
                $reference = Resolve-ProjectReference `
                    -RawPath ([string]($view.GetReferencedModelName())) `
                    -NormalizedProjectRoot $NormalizedProjectRoot `
                    -DocumentIndex $DocumentIndex
                $referencedConfiguration = [string]$view.ReferencedConfiguration
                if ([string]::IsNullOrWhiteSpace($referencedConfiguration)) {
                    $referencedConfiguration = $null
                }
                [void]$views.Add([ordered]@{
                        name = [string]($view.GetName2())
                        persistent_reference = Get-PersistentReferenceFacts `
                            -ModelExtension $ModelExtension `
                            -Object $view
                        view_type = [ordered]@{
                            code = $viewType
                            name = Get-DrawingViewTypeName $viewType
                        }
                        base_view = Get-BaseViewFacts `
                            -View $view `
                            -ModelExtension $ModelExtension
                        reference = $reference
                        referenced_configuration = $referencedConfiguration
                        position_m = $position
                        scale_decimal = [double]$view.ScaleDecimal
                        scale_ratio = $scaleRatio
                        use_sheet_scale = ([int]$view.UseSheetScale -ne 0)
                        use_parent_scale = [bool]$view.UseParentScale
                        angle_rad = [double]$view.Angle
                        model_to_view_transform = $viewTransform
                    })
            }
            [void]$sheets.Add([ordered]@{
                    name = [string]$sheetName
                    persistent_reference = Get-PersistentReferenceFacts `
                        -ModelExtension $ModelExtension `
                        -Object $sheet
                    properties = [ordered]@{
                        paper_size_code = [int]$sheetProperties[0]
                        template_code = [int]$sheetProperties[1]
                        scale_ratio = @(
                            [double]$sheetProperties[2],
                            [double]$sheetProperties[3]
                        )
                        first_angle_projection = ([double]$sheetProperties[4] -ne 0.0)
                        width_m = [double]$sheetProperties[5]
                        height_m = [double]$sheetProperties[6]
                        same_custom_properties = ([double]$sheetProperties[7] -ne 0.0)
                    }
                    views = @($views)
                })
        }
    } finally {
        if (-not [string]::IsNullOrWhiteSpace($originalSheetName)) {
            [void]$Drawing.ActivateSheet($originalSheetName)
        }
    }
    return @($sheets)
}

$normalizedProjectRoot = [IO.Path]::GetFullPath($ProjectRoot)
if (-not (Test-Path -LiteralPath $normalizedProjectRoot -PathType Container)) {
    throw "ProjectRoot is not a directory: $ProjectRoot"
}
$drawingFullPath = if ([IO.Path]::IsPathRooted($DrawingPath)) {
    [IO.Path]::GetFullPath($DrawingPath)
} else {
    [IO.Path]::GetFullPath((Join-Path $normalizedProjectRoot $DrawingPath))
}
$drawingRelativePath = Get-ProjectRelativePath `
    -Path $drawingFullPath `
    -NormalizedProjectRoot $normalizedProjectRoot
if ($null -eq $drawingRelativePath -or `
        -not (Test-Path -LiteralPath $drawingFullPath -PathType Leaf)) {
    throw "DrawingPath must be a file inside ProjectRoot: $DrawingPath"
}
if ([IO.Path]::GetExtension($drawingFullPath).ToLowerInvariant() -ne ".slddrw") {
    throw "DrawingPath must have a .SLDDRW extension: $DrawingPath"
}

$outputFullPath = [IO.Path]::GetFullPath($OutputPath)
if ((Test-Path -LiteralPath $outputFullPath) -and -not $Force) {
    throw "OutputPath already exists; pass -Force to replace it: $OutputPath"
}

$documentIndex = @{}
foreach ($file in @(
        Get-ChildItem -LiteralPath $normalizedProjectRoot -File -Recurse |
            Where-Object {
                $DocumentExtensions -contains $_.Extension.ToLowerInvariant()
            } |
            Sort-Object FullName
    )) {
    $relative = Get-ProjectRelativePath `
        -Path $file.FullName `
        -NormalizedProjectRoot $normalizedProjectRoot
    $key = $file.Name.ToLowerInvariant()
    if (-not $documentIndex.ContainsKey($key)) {
        $documentIndex[$key] = @()
    }
    $documentIndex[$key] = @($documentIndex[$key]) + @($relative)
}

$runningApplication = $null
try {
    $runningApplication = [Runtime.InteropServices.Marshal]::GetActiveObject(
        "SldWorks.Application"
    )
} catch {
    $runningApplication = $null
}
if ($null -ne $runningApplication) {
    [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject(
        $runningApplication
    )
    throw "SolidWorks is already running; close it before capturing ground truth"
}

$swApplication = $null
$model = $null
$modelTitle = $null
try {
    $swApplication = New-Object -ComObject SldWorks.Application
    $swApplication.Visible = $false
    [int]$openErrors = 0
    [int]$openWarnings = 0
    $model = $swApplication.OpenDoc6(
        $drawingFullPath,
        3,
        3,
        "",
        [ref]$openErrors,
        [ref]$openWarnings
    )
    if ($null -eq $model) {
        throw "OpenDoc6 failed (errors=$openErrors, warnings=$openWarnings)"
    }
    $modelTitle = [string]$model.GetTitle()
    $modelExtension = $model.Extension
    [int]$savedLicenseCode = [int]($modelExtension.GetLicenseType())
    $sheets = @(Get-DrawingFacts `
            -Drawing $model `
            -ModelExtension $modelExtension `
            -NormalizedProjectRoot $normalizedProjectRoot `
            -DocumentIndex $documentIndex)

    [string]$baseVersion = ""
    [string]$buildNumber = ""
    [string]$hotFixes = ""
    [void]$swApplication.GetBuildNumbers2(
        [ref]$baseVersion,
        [ref]$buildNumber,
        [ref]$hotFixes
    )
    [int]$currentLicenseCode = [int]($swApplication.GetCurrentLicenseType())
    $sourceFile = Get-Item -LiteralPath $drawingFullPath
    $record = [ordered]@{
        schema_version = 1
        fixture_id = $FixtureId
        captured_at_utc = [DateTime]::UtcNow.ToString("o")
        capture_tool = [ordered]@{
            name = "capture_drawing_ground_truth.ps1"
            version = 1
            powershell_version = $PSVersionTable.PSVersion.ToString()
        }
        solidworks = [ordered]@{
            revision_number = [string]($swApplication.RevisionNumber())
            base_version = $baseVersion
            build_number = $buildNumber
            hot_fixes = $hotFixes
            current_license_type = [ordered]@{
                code = $currentLicenseCode
                name = Get-LicenseTypeName $currentLicenseCode
            }
        }
        source = [ordered]@{
            path = $drawingRelativePath
            sha256 = (Get-FileHash `
                    -LiteralPath $drawingFullPath `
                    -Algorithm SHA256).Hash.ToLowerInvariant()
            byte_size = $sourceFile.Length
            open_errors = $openErrors
            open_warnings = $openWarnings
            saved_license_type = [ordered]@{
                code = $savedLicenseCode
                name = Get-LicenseTypeName $savedLicenseCode
            }
        }
        drawing = [ordered]@{
            length_unit = "meter"
            angle_unit = "radian"
            sheets = $sheets
        }
    }

    $outputDirectory = [IO.Path]::GetDirectoryName($outputFullPath)
    if (-not [string]::IsNullOrWhiteSpace($outputDirectory)) {
        [void][IO.Directory]::CreateDirectory($outputDirectory)
    }
    $json = $record | ConvertTo-Json -Depth 100
    $utf8WithoutBom = [Text.UTF8Encoding]::new($false)
    [IO.File]::WriteAllText($outputFullPath, "$json`n", $utf8WithoutBom)
    Write-Output "wrote Drawing ground truth: $outputFullPath"
} finally {
    try {
        if ($null -ne $model -and `
                -not [string]::IsNullOrWhiteSpace($modelTitle)) {
            [void]$swApplication.CloseDoc($modelTitle)
        }
    } finally {
        if ($null -ne $swApplication) {
            try {
                [void]$swApplication.ExitApp()
            } finally {
                [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject(
                    $swApplication
                )
            }
        }
    }
}
