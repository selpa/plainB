param([string]$CrtDirectory)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$packageName = 'PlainBrowser-0.1.0-windows-x64'
$distRoot = Join-Path $projectRoot 'dist'
$packageRoot = Join-Path $distRoot $packageName
if (Test-Path -LiteralPath $packageRoot) { throw 'Package folder already exists. Use a new version or move the previous package first.' }
$null = New-Item -ItemType Directory -Path $packageRoot
Copy-Item -LiteralPath (Join-Path $projectRoot 'target/release/plain-browser.exe') -Destination $packageRoot
if ($CrtDirectory) {
    foreach ($name in @('vcruntime140.dll','vcruntime140_1.dll')) {
        Copy-Item -LiteralPath (Join-Path $CrtDirectory $name) -Destination $packageRoot
    }
}
$licenseRoot = Join-Path $packageRoot 'licenses'
$null = New-Item -ItemType Directory -Path $licenseRoot
foreach ($name in @('NOTICE.txt','CC-BY-SA-3.0.txt','adblock-rust-MPL-2.0.txt')) {
    Copy-Item -LiteralPath (Join-Path $projectRoot "filters/$name") -Destination $licenseRoot
}
$metadata = Get-Content -LiteralPath (Join-Path $projectRoot 'work/portable-metadata.json') -Raw | ConvertFrom-Json
$index = @('Third-party dependency notices (including build and platform dependencies).', '')
foreach ($package in $metadata.packages) {
    if (-not $package.source) { continue }
    $index += "$($package.name) $($package.version) | $($package.license) | $($package.repository)"
    $sourceRoot = Split-Path $package.manifest_path -Parent
    $notices = Get-ChildItem -LiteralPath $sourceRoot -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|AUTHORS|COPYRIGHT)' }
    if ($notices) {
        $destination = Join-Path $licenseRoot "$($package.name)-$($package.version)"
        $null = New-Item -ItemType Directory -Path $destination
        $notices | Copy-Item -Destination $destination
    }
    if ($package.name -eq 'adblock') {
        # Include the exact unmodified MPL dependency sources used for this build.
        Compress-Archive -LiteralPath $sourceRoot -DestinationPath (Join-Path $licenseRoot 'adblock-rust-source.zip')
    }
}
$index | Set-Content -LiteralPath (Join-Path $licenseRoot 'DEPENDENCIES.txt') -Encoding utf8
@'
<!doctype html><html lang="ko"><meta charset="utf-8"><title>Plain Browser 복사 실행 안내</title>
<style>body{max-width:800px;margin:40px auto;padding:0 20px;font:16px/1.8 "Malgun Gothic",sans-serif;color:#25332e}code{background:#edf3ef;padding:2px 5px}a{color:#23755c}</style>
<h1>Plain Browser 0.1.0 · Windows x64</h1>
<p>빌드 날짜: 2026년 11월 17일. 다른 PC로 폴더 전체를 복사하여 사용하는 시험 배포본입니다.</p>
<h2>실행 방법</h2><ol><li>ZIP을 다른 PC의 쓰기 가능한 폴더에 모두 압축 해제합니다. ZIP 안에서 바로 실행하지 마세요.</li><li><code>plain-browser.exe</code>를 실행합니다. 동봉 DLL과 licenses 폴더를 함께 보관하세요.</li></ol>
<h2>필요 조건</h2><p>64비트 Windows용입니다. Windows 11을 권장하며 다른 PC에서의 호환성은 직접 확인이 필요합니다. Rust, Cargo, Visual Studio 설치는 필요하지 않습니다.</p>
<p>Microsoft Edge WebView2 Runtime이 필요합니다. Windows 11에는 기본 포함되지만 설치 상태는 PC마다 확인해야 합니다. 없을 때만 <a href="https://developer.microsoft.com/microsoft-edge/webview2/">Microsoft 공식 다운로드</a>의 Evergreen Runtime x64를 설치하세요. 이 패키지는 설치 프로그램이나 시스템 설정을 자동 실행하지 않습니다.</p>
<p><a href="https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution">WebView2 배포 조건 안내</a></p>
<h2>데이터와 광고 목록</h2><p>첫 실행 시 실행 파일 옆 <code>data/webview2</code> 폴더에 쿠키·캐시 등의 데이터를 생성합니다. Program Files 등 쓰기가 제한된 폴더에는 두지 마세요. 새 PC로 개인 데이터를 옮기고 싶지 않다면 앱을 종료하고 data 폴더를 제외해 복사하세요. 이번 배포본에는 개발 PC의 개인 데이터가 없습니다.</p>
<p>EasyList는 실행 파일에 포함되어 있으므로 별도 easylist.txt가 없어도 동작합니다. 자동 갱신은 아직 지원하지 않습니다. 광고차단 ON/OFF는 하단에 표시합니다.</p>
<h2>현재 기능</h2><p>주소 입력, 이전·다음, 새로고침, 탭 추가·전환·닫기·드래그 순서 변경, 마우스를 올린 링크의 하단 주소 표시를 지원합니다. 서브도메인을 포함해 다른 호스트의 링크는 새 탭에서 엽니다. 자동 팝업은 현재 차단합니다.</p>
<h2>배포 정보</h2><p>코드 서명이 없는 개발 배포본입니다. 보안 정책으로 실행이 차단되면 표시된 오류를 확인하세요. 보안 설정을 자동으로 바꾸지 않습니다. 라이선스와 adblock-rust 원본 소스는 licenses 폴더에 있습니다.</p>
</html>
'@ | Set-Content -LiteralPath (Join-Path $packageRoot '사용안내.html') -Encoding utf8
Get-FileHash -LiteralPath (Join-Path $packageRoot 'plain-browser.exe') -Algorithm SHA256 |
    ForEach-Object { "$($_.Hash)  plain-browser.exe" } |
    Set-Content -LiteralPath (Join-Path $packageRoot 'SHA256.txt') -Encoding ascii
Compress-Archive -LiteralPath $packageRoot -DestinationPath (Join-Path $distRoot "$packageName.zip")
Get-ChildItem -LiteralPath $distRoot | Select-Object Name,Length
