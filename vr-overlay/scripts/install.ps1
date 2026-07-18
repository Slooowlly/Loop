# Registra a API layer do iRacer no OpenXR (implícita, por-usuário: sem admin).
#
# Uso:
#   .\install.ps1                         # procura o .dll no build\ padrão
#   .\install.ps1 -DllPath "C:\...\iracer_overlay_layer.dll"
#
# O que faz:
#   1. Gera um manifesto JSON ao lado do .dll apontando pra ele.
#   2. Cria o valor no registro em HKCU\...\OpenXR\1\ApiLayers\Implicit = 0 (ligado).
#
# Layers IMPLÍCITAS carregam sozinhas em qualquer app OpenXR (inclusive o iRacing),
# sem o app pedir. Pra desligar sem desinstalar: setar a env var IRACER_OVERLAY_DISABLE=1.

param(
  [string]$DllPath = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($DllPath)) {
  $candidates = @(
    "$PSScriptRoot\..\build\Release\iracer_overlay_layer.dll",
    "$PSScriptRoot\..\build\iracer_overlay_layer.dll"
  )
  $DllPath = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}

if ([string]::IsNullOrWhiteSpace($DllPath) -or -not (Test-Path $DllPath)) {
  Write-Error "DLL nao encontrada. Compile primeiro (veja README) ou passe -DllPath."
  exit 1
}

$DllPath  = (Resolve-Path $DllPath).Path
$jsonPath = [System.IO.Path]::Combine([System.IO.Path]::GetDirectoryName($DllPath),
                                      "XR_APILAYER_NOVA_iracer_overlay.json")

# JSON exige barras escapadas.
$dllEscaped = $DllPath -replace '\\', '\\'

$manifest = @"
{
  "file_format_version": "1.0.0",
  "api_layer": {
    "name": "XR_APILAYER_NOVA_iracer_overlay",
    "library_path": "$dllEscaped",
    "api_version": "1.0",
    "implementation_version": "1",
    "description": "iRacer timing tower overlay (spike 1)",
    "disable_environment": "IRACER_OVERLAY_DISABLE"
  }
}
"@

# UTF-8 SEM BOM: o parser de manifesto do OpenXR engasga com BOM.
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($jsonPath, $manifest, $utf8NoBom)
Write-Host "Manifesto escrito: $jsonPath"

$key = "HKCU:\Software\Khronos\OpenXR\1\ApiLayers\Implicit"
if (-not (Test-Path $key)) {
  New-Item -Path $key -Force | Out-Null
}
# Valor: nome = caminho do JSON; dado DWORD 0 = habilitada.
New-ItemProperty -Path $key -Name $jsonPath -PropertyType DWord -Value 0 -Force | Out-Null

Write-Host "Registrada em: $key"
Write-Host "  -> $jsonPath = 0 (habilitada)"
Write-Host ""
Write-Host "Pronto. Abra o iRacing em VR (runtime OpenXR) e procure o painel magenta."
Write-Host "Log de diagnostico: $env:TEMP\iracer_overlay_layer.log"
