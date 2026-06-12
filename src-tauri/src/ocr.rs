use std::process::Command;

// async: the PowerShell call takes 1-2s — a sync command would run it on the
// main thread and freeze every window's event loop meanwhile.
#[tauri::command]
pub async fn ocr_image(path: String, language: String) -> Result<String, String> {
    #[cfg(windows)]
    {
        // Use PowerShell to call WinRT OCR — avoids windows crate version conflicts
        let script = format!(
            r#"
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$null = [Windows.Media.Ocr.OcrEngine, Windows.Foundation, ContentType=WindowsRuntime]
$null = [Windows.Graphics.Imaging.BitmapDecoder, Windows.Foundation, ContentType=WindowsRuntime]
$null = [Windows.Storage.Streams.RandomAccessStream, Windows.Foundation, ContentType=WindowsRuntime]

function Await($WinRtTask, $ResultType) {{
    $asTaskGeneric = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {{ $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' }})[0]
    $asTask = $asTaskGeneric.MakeGenericMethod($ResultType)
    $netTask = $asTask.Invoke($null, @($WinRtTask))
    $netTask.Wait(-1) | Out-Null
    $netTask.Result
}}

$path = '{}'
$lang = [Windows.Globalization.Language]::new('{}')

$stream = [System.IO.File]::OpenRead($path)
$decoder = Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync([Windows.Storage.Streams.InMemoryRandomAccessStream]::new())) ([Windows.Graphics.Imaging.BitmapDecoder])

$fileStream = [Windows.Storage.Streams.RandomAccessStream]::new()
$inputStream = [System.IO.WindowsRuntimeStreamExtensions]::AsRandomAccessStream($stream)
$decoder2 = Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($inputStream)) ([Windows.Graphics.Imaging.BitmapDecoder])
$bitmap = Await ($decoder2.GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])

$engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage($lang)
if (-not $engine) {{
    $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages()
}}

$result = Await ($engine.RecognizeAsync($bitmap)) ([Windows.Media.Ocr.OcrResult])
$stream.Close()
Write-Output $result.Text
"#,
            path.replace('\'', "''"),
            language.replace('\'', "''")
        );

        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .map_err(|e| format!("Failed to run PowerShell OCR: {}", e))?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(text)
        } else {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(format!("OCR failed: {}", err))
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (path, language);
        Err("OCR is only supported on Windows".into())
    }
}
