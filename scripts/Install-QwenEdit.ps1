[CmdletBinding(SupportsShouldProcess)]
param([Parameter(Mandatory)][string]$ModelsDirectory)
$ErrorActionPreference = 'Stop'
# Explicit destination: use the models folder belonging to the active ComfyUI.
$modelsRoot = [System.IO.Path]::GetFullPath($ModelsDirectory)
$files = @(
    @{ Folder='diffusion_models'; Name='qwen_image_edit_2509_fp8_e4m3fn.safetensors'; Min=18000000000L; Url='https://huggingface.co/Comfy-Org/Qwen-Image-Edit_ComfyUI/resolve/main/split_files/diffusion_models/qwen_image_edit_2509_fp8_e4m3fn.safetensors' },
    @{ Folder='text_encoders'; Name='qwen_2.5_vl_7b_fp8_scaled.safetensors'; Min=8000000000L; Url='https://huggingface.co/Comfy-Org/Qwen-Image_ComfyUI/resolve/main/split_files/text_encoders/qwen_2.5_vl_7b_fp8_scaled.safetensors' },
    @{ Folder='vae'; Name='qwen_image_vae.safetensors'; Min=200000000L; Url='https://huggingface.co/Comfy-Org/Qwen-Image_ComfyUI/resolve/main/split_files/vae/qwen_image_vae.safetensors' }
)
Write-Host "Qwen Edit downloads about 28 GB into: $modelsRoot"
Get-Command curl.exe -ErrorAction Stop | Out-Null
foreach ($model in $files) {
    $folder = Join-Path $modelsRoot $model.Folder
    $destination = Join-Path $folder $model.Name
    if (Test-Path -LiteralPath $destination) {
        if ((Get-Item -LiteralPath $destination).Length -lt $model.Min) { throw "Existing file is unexpectedly small: $destination. Inspect it before retrying." }
        Write-Host "Already present: $destination"
        continue
    }
    if ($PSCmdlet.ShouldProcess($destination, 'Download official Comfy-Org model file')) {
        New-Item -ItemType Directory -Force -Path $folder | Out-Null
        $partial = "$destination.part"
        & curl.exe --fail --location --retry 3 --continue-at - --output $partial $model.Url
        if ($LASTEXITCODE -ne 0) { throw "Download failed; partial file retained for resuming: $partial" }
        if ((Get-Item -LiteralPath $partial).Length -lt $model.Min) { throw "Download unexpectedly small: $partial" }
        Move-Item -LiteralPath $partial -Destination $destination
    }
}
Write-Host 'Restart ComfyUI after installation. Select Qwen-Image-Edit 2509 (FP8) under Changes prompt in PixelForge.'
