# Remove o registro da API layer do iRacer (desfaz o install.ps1).
# Nao apaga o .dll nem o .json — so tira a layer do OpenXR.

$ErrorActionPreference = "Stop"

$key = "HKCU:\Software\Khronos\OpenXR\1\ApiLayers\Implicit"
if (-not (Test-Path $key)) {
  Write-Host "Nada registrado (chave nao existe)."
  exit 0
}

$removed = 0
$props = Get-ItemProperty -Path $key
foreach ($name in $props.PSObject.Properties.Name) {
  if ($name -like "*XR_APILAYER_NOVA_iracer_overlay.json") {
    Remove-ItemProperty -Path $key -Name $name -Force
    Write-Host "Removido: $name"
    $removed++
  }
}

if ($removed -eq 0) {
  Write-Host "Nenhum registro da layer iRacer encontrado."
} else {
  Write-Host "OK — layer iRacer desregistrada ($removed entrada(s))."
}
