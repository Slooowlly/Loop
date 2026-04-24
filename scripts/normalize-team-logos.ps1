param(
  [string]$InputDir = "image/Times",
  [string]$OutputDir = "image/TimesNormalized",
  [int]$CanvasWidth = 768,
  [int]$CanvasHeight = 512,
  [double]$WidthFillRatio = 0.88,
  [double]$HeightFillRatio = 0.74,
  [int]$AlphaThreshold = 8
)

Add-Type -AssemblyName System.Drawing

$ErrorActionPreference = "Stop"

function Get-ContentBounds {
  param(
    [System.Drawing.Bitmap]$Bitmap,
    [int]$AlphaThreshold
  )

  $minX = $Bitmap.Width
  $minY = $Bitmap.Height
  $maxX = -1
  $maxY = -1

  for ($y = 0; $y -lt $Bitmap.Height; $y++) {
    for ($x = 0; $x -lt $Bitmap.Width; $x++) {
      $pixel = $Bitmap.GetPixel($x, $y)
      if ($pixel.A -gt $AlphaThreshold) {
        if ($x -lt $minX) { $minX = $x }
        if ($y -lt $minY) { $minY = $y }
        if ($x -gt $maxX) { $maxX = $x }
        if ($y -gt $maxY) { $maxY = $y }
      }
    }
  }

  if ($maxX -lt 0 -or $maxY -lt 0) {
    return [System.Drawing.Rectangle]::new(0, 0, $Bitmap.Width, $Bitmap.Height)
  }

  return [System.Drawing.Rectangle]::new($minX, $minY, $maxX - $minX + 1, $maxY - $minY + 1)
}

function Normalize-Logo {
  param(
    [string]$SourcePath,
    [string]$DestinationPath,
    [int]$CanvasWidth,
    [int]$CanvasHeight,
    [double]$WidthFillRatio,
    [double]$HeightFillRatio,
    [int]$AlphaThreshold
  )

  $source = [System.Drawing.Bitmap]::new($SourcePath)
  try {
    $bounds = Get-ContentBounds -Bitmap $source -AlphaThreshold $AlphaThreshold
    $targetContentWidth = [Math]::Round($CanvasWidth * $WidthFillRatio)
    $targetContentHeight = [Math]::Round($CanvasHeight * $HeightFillRatio)
    $scale = [Math]::Min($targetContentWidth / $bounds.Width, $targetContentHeight / $bounds.Height)
    $drawWidth = [Math]::Max(1, [int][Math]::Round($bounds.Width * $scale))
    $drawHeight = [Math]::Max(1, [int][Math]::Round($bounds.Height * $scale))
    $drawX = [int][Math]::Round(($CanvasWidth - $drawWidth) / 2)
    $drawY = [int][Math]::Round(($CanvasHeight - $drawHeight) / 2)

    $canvas = [System.Drawing.Bitmap]::new($CanvasWidth, $CanvasHeight, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    try {
      $graphics = [System.Drawing.Graphics]::FromImage($canvas)
      try {
        $graphics.Clear([System.Drawing.Color]::Transparent)
        $graphics.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceOver
        $graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
        $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality

        $destinationRect = [System.Drawing.Rectangle]::new($drawX, $drawY, $drawWidth, $drawHeight)
        $graphics.DrawImage($source, $destinationRect, $bounds, [System.Drawing.GraphicsUnit]::Pixel)
      }
      finally {
        $graphics.Dispose()
      }

      $destinationDir = Split-Path -Parent $DestinationPath
      if (-not (Test-Path -LiteralPath $destinationDir)) {
        New-Item -ItemType Directory -Force -Path $destinationDir | Out-Null
      }

      $canvas.Save($DestinationPath, [System.Drawing.Imaging.ImageFormat]::Png)
    }
    finally {
      $canvas.Dispose()
    }

    return [pscustomobject]@{
      Source = $SourcePath
      Destination = $DestinationPath
      Original = "$($source.Width)x$($source.Height)"
      Bounds = "$($bounds.Width)x$($bounds.Height)"
      Normalized = "${CanvasWidth}x${CanvasHeight}"
      Drawn = "${drawWidth}x${drawHeight}"
    }
  }
  finally {
    $source.Dispose()
  }
}

$resolvedInput = Resolve-Path -LiteralPath $InputDir
$repoRoot = Resolve-Path -LiteralPath "."
$inputRoot = $resolvedInput.Path.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
$outputs = @()

Get-ChildItem -LiteralPath $resolvedInput -Recurse -File -Filter *.png | ForEach-Object {
  $relative = $_.FullName.Substring($inputRoot.Length).TrimStart([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
  $destination = Join-Path $repoRoot.Path (Join-Path $OutputDir $relative)
  $outputs += Normalize-Logo `
    -SourcePath $_.FullName `
    -DestinationPath $destination `
    -CanvasWidth $CanvasWidth `
    -CanvasHeight $CanvasHeight `
    -WidthFillRatio $WidthFillRatio `
    -HeightFillRatio $HeightFillRatio `
    -AlphaThreshold $AlphaThreshold
}

$outputs | Sort-Object Source | Format-Table -AutoSize
Write-Host "Normalized $($outputs.Count) logos into $OutputDir"
