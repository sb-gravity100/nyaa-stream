# Creates branded NSIS sidebar/header artwork from the app icon.
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$installer = Join-Path $root "src-tauri\installer"
$iconPath = Join-Path $root "src-tauri\icons\128x128.png"
$icon = [System.Drawing.Image]::FromFile($iconPath)

function New-GradientBitmap([int]$width, [int]$height, [string]$path, [bool]$sidebar) {
  $bitmap = [System.Drawing.Bitmap]::new($width, $height, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $top = [System.Drawing.ColorTranslator]::FromHtml("#292E4A")
  $bottom = [System.Drawing.ColorTranslator]::FromHtml("#171A2B")
  $brush = [System.Drawing.Drawing2D.LinearGradientBrush]::new([System.Drawing.Rectangle]::new(0, 0, $width, $height), $top, $bottom, 90)
  $graphics.FillRectangle($brush, 0, 0, $width, $height)
  $accent = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml("#F07AA6"))
  if ($sidebar) {
    $graphics.FillEllipse($accent, 138, -25, 80, 80)
    $graphics.DrawImage($icon, 40, 58, 84, 84)
    $font = [System.Drawing.Font]::new("Segoe UI Semibold", 12, [System.Drawing.FontStyle]::Regular)
    $sub = [System.Drawing.Font]::new("Segoe UI", 8, [System.Drawing.FontStyle]::Regular)
    $white = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml("#ECEAF6"))
    $muted = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml("#A8ABC4"))
    $graphics.DrawString("nyaa-stream", $font, $white, 26, 166)
    $graphics.DrawString("Find your next episode.", $sub, $muted, 26, 190)
    $graphics.FillRectangle($accent, 26, 222, 34, 3)
    $font.Dispose(); $sub.Dispose(); $white.Dispose(); $muted.Dispose()
  } else {
    $graphics.FillRectangle($accent, 0, $height - 3, $width, 3)
    $graphics.DrawImage($icon, 12, 9, 38, 38)
    $font = [System.Drawing.Font]::new("Segoe UI Semibold", 10, [System.Drawing.FontStyle]::Regular)
    $white = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml("#ECEAF6"))
    $graphics.DrawString("nyaa-stream setup", $font, $white, 58, 19)
    $font.Dispose(); $white.Dispose()
  }
  $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Bmp)
  $accent.Dispose(); $brush.Dispose(); $graphics.Dispose(); $bitmap.Dispose()
}

New-GradientBitmap 164 314 (Join-Path $installer "nsis-sidebar.bmp") $true
New-GradientBitmap 150 57 (Join-Path $installer "nsis-header.bmp") $false
$icon.Dispose()
