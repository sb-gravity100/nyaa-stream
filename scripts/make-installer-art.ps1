# Renders the MSI (WiX) installer artwork from the app icon + brand colors:
#   src-tauri/installer/banner.bmp  493x58   (top strip of the inner dialogs)
#   src-tauri/installer/dialog.bmp  493x312  (welcome/finish dialogs; only the
#                                             left 164px is visible, the rest is
#                                             covered by the dialog's white area)
# Run: powershell -File scripts/make-installer-art.ps1
Add-Type -AssemblyName System.Drawing
$root = Split-Path $PSScriptRoot -Parent
$out = Join-Path $root "src-tauri\installer"
New-Item -ItemType Directory -Force $out | Out-Null
$icon = [System.Drawing.Image]::FromFile((Join-Path $root "src-tauri\icons\icon.png"))

$ink = [System.Drawing.ColorTranslator]::FromHtml("#171a2b")
$inkDeep = [System.Drawing.ColorTranslator]::FromHtml("#10121f")
$accent = [System.Drawing.ColorTranslator]::FromHtml("#f07aa6")

function New-Canvas($w, $h) {
  $bmp = New-Object System.Drawing.Bitmap $w, $h, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = "AntiAlias"
  $g.InterpolationMode = "HighQualityBicubic"
  $g.TextRenderingHint = "AntiAliasGridFit"
  return @($bmp, $g)
}

function Fill-Gradient($g, $x, $y, $w, $h, $c1, $c2, $angle) {
  $rect = New-Object System.Drawing.Rectangle $x, $y, $w, $h
  $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, $c1, $c2, $angle
  $g.FillRectangle($brush, $rect)
  $brush.Dispose()
}

# Banner: white strip, ink block on the right holding the mark (WiX draws the
# dialog title text on the left).
$bmp, $g = New-Canvas 493 58
$g.Clear([System.Drawing.Color]::White)
Fill-Gradient $g 373 0 120 58 $ink $inkDeep 45
$g.DrawImage($icon, 415, 8, 42, 42)
$bar = New-Object System.Drawing.SolidBrush $accent
$g.FillRectangle($bar, 0, 55, 493, 3)
$g.Dispose(); $bmp.Save((Join-Path $out "banner.bmp"), [System.Drawing.Imaging.ImageFormat]::Bmp); $bmp.Dispose()

# Dialog: 164px ink panel with the mark and wordmark, white elsewhere.
$bmp, $g = New-Canvas 493 312
$g.Clear([System.Drawing.Color]::White)
Fill-Gradient $g 0 0 164 312 $ink $inkDeep 70
$g.FillRectangle($bar, 161, 0, 3, 312)
$g.DrawImage($icon, 32, 78, 100, 100)
$font = New-Object System.Drawing.Font "Segoe UI Semibold", 15
$sub = New-Object System.Drawing.Font "Segoe UI", 8.5
$white = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::White)
$dim = New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml("#a9a7c4"))
$fmt = New-Object System.Drawing.StringFormat
$fmt.Alignment = "Center"
$g.DrawString("nyaa-stream", $font, $white, (New-Object System.Drawing.RectangleF 0, 190, 161, 28), $fmt)
$g.DrawString("Stream anime, straight from nyaa", $sub, $dim, (New-Object System.Drawing.RectangleF 8, 220, 145, 36), $fmt)
$g.Dispose(); $bmp.Save((Join-Path $out "dialog.bmp"), [System.Drawing.Imaging.ImageFormat]::Bmp); $bmp.Dispose()
$icon.Dispose()
Write-Host "wrote $out\banner.bmp and dialog.bmp"
