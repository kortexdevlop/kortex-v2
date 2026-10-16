# Kortex installer (Windows):  irm https://kortex.site/install.ps1 | iex
#
# Installs `kortex.exe` into %USERPROFILE%\.kortex\bin (or $env:KORTEX_HOME\bin), verifies the
# SHA-256 of the download and adds the folder to the *user* PATH. No administrator rights needed.
#
# Trust model: this script and cli-latest.txt come from kortex.site over TLS. Every tool installed
# afterwards is verified by `kortex` against the Ed25519 signature of the release index.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Index = if ($env:KORTEX_INDEX) { $env:KORTEX_INDEX } else { 'https://kortex.site/registry/index.json' }
$HomeDir = if ($env:KORTEX_HOME) { $env:KORTEX_HOME } else { Join-Path $env:USERPROFILE '.kortex' }
$BinDir = Join-Path $HomeDir 'bin'

if ($Index -notmatch '^https://' -and $Index -notmatch '^http://(localhost|127\.0\.0\.1)') {
  throw 'KORTEX_INDEX doit être en https:// (ou localhost pour le développement)'
}
if (-not [Environment]::Is64BitOperatingSystem) { throw 'Windows 64 bits requis' }
$Target = 'x86_64-pc-windows-msvc'
$Base = $Index.Substring(0, $Index.LastIndexOf('/'))

Write-Host "Kortex — installation pour $Target"
$Tmp = Join-Path ([IO.Path]::GetTempPath()) ("kortex-install-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $Tmp | Out-Null
try {
  $ListFile = Join-Path $Tmp 'cli-latest.txt'
  Invoke-WebRequest -UseBasicParsing "$Base/cli-latest.txt" -OutFile $ListFile
  $Line = Get-Content $ListFile | Where-Object { $_ -like "$Target *" } | Select-Object -First 1
  if (-not $Line) { throw "pas de version de kortex pour $Target dans cli-latest.txt" }
  $Parts = $Line -split ' '
  $Want = $Parts[1].ToLower()
  $Url = $Parts[2]
  if ($Want -notmatch '^[0-9a-f]{64}$') { throw 'empreinte invalide dans cli-latest.txt' }
  if ($Url -notmatch '://') { $Url = "$Base/$Url" }

  Write-Host 'Téléchargement…'
  $Pkg = Join-Path $Tmp 'kortex.tar.gz'
  Invoke-WebRequest -UseBasicParsing $Url -OutFile $Pkg
  $Got = (Get-FileHash $Pkg -Algorithm SHA256).Hash.ToLower()
  if ($Got -ne $Want) { throw "empreinte SHA-256 différente de celle attendue : paquet refusé ($Got)" }

  # Extract only kortex.exe (tar ships with Windows 10+).
  $Out = Join-Path $Tmp 'out'
  New-Item -ItemType Directory -Path $Out | Out-Null
  & tar.exe -xzf $Pkg -C $Out 'kortex.exe'
  $Exe = Join-Path $Out 'kortex.exe'
  if (-not (Test-Path $Exe) -or (Get-Item $Exe).Length -eq 0) { throw "archive invalide (fichier 'kortex.exe' absent)" }
  New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
  Move-Item -Force $Exe (Join-Path $BinDir 'kortex.exe')
  Write-Host "✓ kortex installé dans $BinDir\kortex.exe"

  $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
  if (($UserPath -split ';') -notcontains $BinDir) {
    [Environment]::SetEnvironmentVariable('Path', "$UserPath;$BinDir", 'User')
    Write-Host 'PATH utilisateur mis à jour : ouvre un nouveau terminal.'
  }
  Write-Host ''
  Write-Host 'Essaie :  kortex available'
  Write-Host '          kortex install secure.password-generator'
  Write-Host 'Vérification :  kortex doctor'
}
finally {
  Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}
