Add-Type -AssemblyName System.Drawing
$assetDirectory = Join-Path $PSScriptRoot '..\assets'
$assetDirectory = [IO.Path]::GetFullPath($assetDirectory)
$canvas = [Drawing.Bitmap]::new(256,256)
$graphics = [Drawing.Graphics]::FromImage($canvas)
$graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
$graphics.ScaleTransform(4,4)
$background = [Drawing.Drawing2D.GraphicsPath]::new()
$background.AddArc(2,2,30,30,180,90)
$background.AddArc(32,2,30,30,270,90)
$background.AddArc(32,32,30,30,0,90)
$background.AddArc(2,32,30,30,90,90)
$background.CloseFigure()
$green = [Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#23755c'))
$graphics.FillPath($green,$background)
$letters = [Drawing.Drawing2D.GraphicsPath]::new([Drawing.Drawing2D.FillMode]::Alternate)
$letters.AddLine(12,18,23,18)
$letters.AddBezier(23,18,35,18,35,36,23,36)
$letters.AddLine(23,36,18,36)
$letters.AddLine(18,36,18,46)
$letters.AddLine(18,46,12,46)
$letters.CloseFigure()
$letters.StartFigure()
$letters.AddLine(18,24,18,30)
$letters.AddLine(18,30,23,30)
$letters.AddBezier(23,30,27,30,27,24,23,24)
$letters.CloseFigure()
$letters.StartFigure()
$letters.AddLine(34,18,43,18)
$letters.AddBezier(43,18,54,18,56,28,49,32)
$letters.AddBezier(49,32,57,36,54,46,43,46)
$letters.AddLine(43,46,34,46)
$letters.CloseFigure()
$letters.StartFigure()
$letters.AddLine(40,24,40,29)
$letters.AddLine(40,29,43,29)
$letters.AddBezier(43,29,48,29,48,24,43,24)
$letters.CloseFigure()
$letters.StartFigure()
$letters.AddLine(40,35,40,40)
$letters.AddLine(40,40,44,40)
$letters.AddBezier(44,40,49,40,49,35,44,35)
$letters.CloseFigure()
$graphics.FillPath([Drawing.Brushes]::White,$letters)
$graphics.Dispose()
$sizes = @(16,32,48,64,128,256)
$frames = @()
foreach ($size in $sizes) {
  $frame = [Drawing.Bitmap]::new($size,$size)
  $frameGraphics = [Drawing.Graphics]::FromImage($frame)
  $frameGraphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $frameGraphics.DrawImage($canvas,0,0,$size,$size)
  $frameGraphics.Dispose()
  $memory = [IO.MemoryStream]::new()
  $frame.Save($memory,[Drawing.Imaging.ImageFormat]::Png)
  $frames += ,($memory.ToArray())
  $memory.Dispose()
  if ($size -eq 64) {
    $frame.Save((Join-Path $assetDirectory 'plain-browser.png'),[Drawing.Imaging.ImageFormat]::Png)
    $raw = [byte[]]::new(64*64*4)
    for ($y=0; $y -lt 64; $y++) {
      for ($x=0; $x -lt 64; $x++) {
        $pixel = $frame.GetPixel($x,$y)
        $offset = ($y*64+$x)*4
        $raw[$offset]=$pixel.R; $raw[$offset+1]=$pixel.G; $raw[$offset+2]=$pixel.B; $raw[$offset+3]=$pixel.A
      }
    }
    [IO.File]::WriteAllBytes((Join-Path $assetDirectory 'plain-browser.rgba'),$raw)
  }
  $frame.Dispose()
}
$iconFile = [IO.File]::Create((Join-Path $assetDirectory 'plain-browser.ico'))
$writer = [IO.BinaryWriter]::new($iconFile)
$writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$sizes.Count)
$dataOffset = 6+16*$sizes.Count
for ($i=0; $i -lt $sizes.Count; $i++) {
  $dimension = if($sizes[$i] -eq 256) {0} else {$sizes[$i]}
  $writer.Write([byte]$dimension); $writer.Write([byte]$dimension)
  $writer.Write([byte]0); $writer.Write([byte]0)
  $writer.Write([uint16]1); $writer.Write([uint16]32)
  $writer.Write([uint32]$frames[$i].Length); $writer.Write([uint32]$dataOffset)
  $dataOffset += $frames[$i].Length
}
foreach($frameData in $frames) {$writer.Write([byte[]]$frameData)}
$writer.Dispose()
$canvas.Dispose(); $letters.Dispose(); $background.Dispose(); $green.Dispose()
