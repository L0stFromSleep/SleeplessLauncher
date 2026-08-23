$env:JAVA_HOME = "C:\Program Files\Eclipse Adoptium\jdk-17.0.20.8-hotspot"
$env:PATH += ";$env:USERPROFILE\.cargo\bin;$env:JAVA_HOME\bin"

Set-Location "C:\Users\Cole\Documents\GitHub\SleeplessLauncher"

# Kill leftover dev servers from a previous run before starting a new one.
# Vite (the app-frontend dev server) binds 1420 with strictPort, so a stale
# process there makes `tauri dev` fail outright instead of just reusing it.
function Stop-ProcessOnPort {
	param([int]$Port)

	$connections = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
	foreach ($procId in ($connections.OwningProcess | Select-Object -Unique)) {
		$proc = Get-Process -Id $procId -ErrorAction SilentlyContinue
		if ($proc) {
			Write-Host "Killing leftover process on port ${Port}: $($proc.ProcessName) (PID $procId)"
			Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue
		}
	}
}

Stop-ProcessOnPort -Port 1420

# Also stop any previous debug build of the app itself still running, since
# a stale instance can hold file locks that make the fresh `cargo run` fail.
Get-Process -Name "theseus_gui" -ErrorAction SilentlyContinue | ForEach-Object {
	Write-Host "Killing leftover theseus_gui process (PID $($_.Id))"
	Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
}

# Modrinth local development environment
$env:MODRINTH_URL = "http://localhost:3000/"
$env:MODRINTH_API_BASE_URL = "http://localhost:8000/"
$env:SHARED_INSTANCES_API_BASE_URL = "https://staging-shared-instances.modrinth.com/"
$env:MODRINTH_API_URL = "http://127.0.0.1:8000/v2/"
$env:MODRINTH_API_URL_V3 = "http://127.0.0.1:8000/v3/"
$env:MODRINTH_SOCKET_URL = "ws://127.0.0.1:8000/"
$env:MODRINTH_LAUNCHER_META_URL = "https://launcher-meta.modrinth.com/"

# Debug - verify variables exist before Cargo starts
Write-Host "MODRINTH_API_URL = $env:MODRINTH_API_URL"
Write-Host "MODRINTH_API_URL_V3 = $env:MODRINTH_API_URL_V3"
Write-Host "MODRINTH_SOCKET_URL = $env:MODRINTH_SOCKET_URL"

pnpm app:dev 2>&1