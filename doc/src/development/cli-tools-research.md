# CLI Tools Research

**Status:** Research Complete  
**Created:** 2026-03-09  
**Purpose:** Reference for external CLI tools that can be integrated with sprachspiel

## Executive Summary

This document catalogs CLI tools available on Linux distributions that can be used by sprachspiel for PDF processing, OCR, image manipulation, and other tasks. Using CLI tools instead of Rust crates offers:

- **Smaller binary**: -2 to -10MB vs embedding lopdf/pdfium
- **Better OCR support**: tesseract + pdftoppm for scanned PDFs
- **Delegated maintenance**: System package managers handle updates
- **Termux support**: Most tools available on Android/Termux

## Research Methodology

- Web research on Poppler, Ghostscript, Tesseract documentation
- Package search on Arch, Debian, Fedora, Termux repositories
- Security considerations from CVE databases
- Integration patterns from similar projects

---

# 1. PDF Processing Tools

## 1.1 pdftotext (Poppler)

### Overview
Text extraction from PDF files. Primary tool for native (non-scanned) PDFs.

### Package Names

| Distribution | Package |
|-------------|---------|
| Arch Linux | `poppler` |
| Debian/Ubuntu | `poppler-utils` |
| Fedora | `poppler-utils` |
| Termux | `poppler` |

### Detection
```bash
which pdftotext || command -v pdftotext
pdftotext -v
```

### Command-Line Usage

```bash
# Extract text to stdout
pdftotext file.pdf -

# Extract to file
pdftotext file.pdf output.txt

# Extract specific pages
pdftotext -f 1 -l 10 file.pdf output.txt

# Preserve layout
pdftotext -layout file.pdf output.txt

# Higher resolution (better for complex layouts)
pdftotext -r 300 file.pdf output.txt

# Fixed-pitch text mode (tables)
pdftotext -fixed 10 file.pdf output.txt

# HTML output with structure
pdftotext -htmlmeta file.pdf output.html

# Bounding box information
pdftotext -bbox file.pdf output.html
```

### Key Options

| Option | Description |
|--------|-------------|
| `-f <n>` | First page |
| `-l <n>` | Last page |
| `-r <dpi>` | Resolution (default: 72) |
| `-layout` | Preserve original layout |
| `-fixed <n>` | Fixed-pitch mode |
| `-nodiag` | Discard diagonal text |
| `-enc <name>` | Output encoding (default: UTF-8) |
| `-nopgbrk` | No page breaks |

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error opening PDF |
| 2 | Error opening output file |
| 3 | PDF permissions error |
| 99 | Other error |

### Limitations

1. **Cannot read scanned PDFs**: Requires OCR (use tesseract)
2. **Font encoding issues**: Some PDFs have corrupted encodings
3. **Complex layouts**: Tables may not preserve well
4. **No image extraction**: Use `pdfimages` for that

### Rust Integration

```rust
fn extract_pdf_text(path: &str) -> Result<String, String> {
    let output = Command::new("pdftotext")
        .args([path, "-"])
        .output()
        .map_err(|e| format!("Failed to execute pdftotext: {}", e))?;
    
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
```

---

## 1.2 pdftoppm / pdftocairo (Poppler)

### Overview
Convert PDF pages to images. Essential for OCR pipeline on scanned PDFs.

### Package Names
Same as pdftotext (part of `poppler-utils`)

### Detection
```bash
which pdftoppm || command -v pdftoppm
pdftoppm -v
```

### Command-Line Usage

```bash
# Convert to PNG (page-1.png, page-2.png, etc.)
pdftoppm -png file.pdf page

# Convert to JPEG
pdftoppm -jpeg file.pdf page

# Specific page range
pdftoppm -png -f 1 -l 5 file.pdf page

# Higher resolution
pdftoppm -png -r 300 file.pdf page

# Scale to fit width
pdftoppm -png -scale-to 1024 file.pdf page

# Single file output
pdftoppm -png -singlefile file.pdf output

# Use pdftocairo for better quality
pdftocairo -png file.pdf output
```

### Key Options

| Option | Description |
|--------|-------------|
| `-png` | PNG output |
| `-jpeg` | JPEG output |
| `-tiff` | TIFF output |
| `-f <n>` | First page |
| `-l <n>` | Last page |
| `-r <dpi>` | Resolution (default: 150) |
| `-scale-to <n>` | Scale long side to n pixels |
| `-gray` | Grayscale output |
| `-singlefile` | Single file without numbering |

### Limitations

1. **Memory usage**: High-resolution conversion needs RAM
2. **Large PDFs**: Consider page-by-page processing
3. **No OCR**: Images still need text extraction

### OCR Pipeline

```bash
# For scanned PDFs
pdftoppm -png -r 300 scanned.pdf page
for f in page-*.png; do
    tesseract "$f" stdout
done > extracted_text.txt
```

---

## 1.3 pdfinfo (Poppler)

### Overview
Extract PDF metadata: pages, author, title, creation date, etc.

### Package Names
Same as pdftotext (part of `poppler-utils`)

### Detection
```bash
which pdfinfo || command -v pdfinfo
```

### Command-Line Usage

```bash
# Get all metadata
pdfinfo file.pdf

# Output includes:
# - Title, Author, Subject, Keywords
# - Creator, Producer
# - Creation date, Modification date
# - Page count, Page size
# - PDF version, Encrypted status

# ISO date format
pdfinfo -isodates file.pdf

# Page size details
pdfinfo -box file.pdf

# Page range info
pdfinfo -f 1 -l 10 file.pdf

# URL extraction
pdfinfo -url file.pdf

# Raw metadata
pdfinfo -meta file.pdf
```

### Rust Integration

```rust
use std::process::Command;

struct PdfInfo {
    title: Option<String>,
    author: Option<String>,
    pages: u32,
    page_size: String,
    encrypted: bool,
}

fn get_pdf_info(path: &str) -> Result<PdfInfo, String> {
    let output = Command::new("pdfinfo")
        .arg(path)
        .output()
        .map_err(|e| e.to_string())?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    let mut info = PdfInfo::default();
    for line in stdout.lines() {
        if line.starts_with("Pages:") {
            info.pages = line.split(':').nth(1).unwrap().trim().parse().unwrap_or(0);
        }
        // ... parse other fields
    }
    
    Ok(info)
}
```

---

## 1.4 qpdf

### Overview
PDF manipulation: merge, split, rotate, encrypt, decrypt. No text extraction.

### Package Names

| Distribution | Package |
|-------------|---------|
| Arch Linux | `qpdf` |
| Debian/Ubuntu | `qpdf` |
| Fedora | `qpdf` |

### Detection
```bash
which qpdf || command -v qpdf
qpdf --version
```

### Command-Line Usage

```bash
# Merge PDFs
qpdf --empty --pages file1.pdf file2.pdf -- merged.pdf

# Split into pages
qpdf --split-pages input.pdf output-prefix

# Extract pages
qpdf --empty --pages input.pdf 1-3,5,7-9 -- extract.pdf

# Rotate pages
qpdf input.pdf --rotate=+90:1,3,5 -- rotated.pdf

# Encrypt
qpdf --encrypt userpass ownerpass 128 -- input.pdf encrypted.pdf

# Decrypt
qpdf --decrypt --password=pass encrypted.pdf decrypted.pdf

# Linearize (web optimization)
qpdf --linearize input.pdf output.pdf

# Compress
qpdf --compress-streams=y input.pdf compressed.pdf

# Inspect structure
qpdf --show-xref input.pdf
```

### Limitations

1. **No text extraction**: Use pdftotext
2. **No rendering**: Use pdftoppm or ghostscript
3. **Content preservation**: Cannot edit content, only structure

---

## 1.5 Ghostscript (gs)

### Overview
Comprehensive PDF/PostScript interpreter. Can convert, render, extract, and manipulate.

### Package Names

| Distribution | Package |
|-------------|---------|
| Arch Linux | `ghostscript` |
| Debian/Ubuntu | `ghostscript` |
| Fedora | `ghostscript` |

### Detection
```bash
which gs || command -v gs
gs --version
```

### Command-Line Usage

```bash
# PDF to PNG
gs -dSAFER -dBATCH -dNOPAUSE -sDEVICE=png16m \
   -r300 -sOutputFile=page-%d.png input.pdf

# PDF to JPEG
gs -dSAFER -dBATCH -dNOPAUSE -sDEVICE=jpeg \
   -r300 -sOutputFile=output-%d.jpg input.pdf

# Extract text
gs -sDEVICE=txtwrite -sOutputFile=output.txt -dNOPAUSE -dBATCH input.pdf

# Compress PDF
gs -sDEVICE=pdfwrite -dPDFSETTINGS=/ebook \
   -dNOPAUSE -dBATCH -sOutputFile=compressed.pdf input.pdf

# Merge PDFs
gs -dBATCH -dNOPAUSE -sDEVICE=pdfwrite -sOutputFile=merged.pdf file1.pdf file2.pdf

# PDF/A conversion
gs -dPDFA -dBATCH -dNOPAUSE -sDEVICE=pdfwrite \
   -sOutputFile=output-pdfa.pdf input.pdf
```

### PDFSETTINGS Presets

| Preset | DPI | Use Case |
|--------|-----|----------|
| `/screen` | 72 | Web, small file |
| `/ebook` | 150 | E-books |
| `/printer` | 300 | Printing |
| `/prepress` | 300 | Maximum quality |

### Security Considerations

Ghostscript has had many CVEs. Always use `-dSAFER` flag for untrusted files:

```bash
gs -dSAFER ...  # Always use SAFER mode
```

### Limitations

1. **Complex command-line**: Verbose flags required
2. **Memory intensive**: High DPI needs RAM
3. **AGPL license**: Commercial use may require license

---

## 1.6 Comparison Table

| Tool | Text Extract | Image Convert | Metadata | Manipulate |
|------|-------------|---------------|----------|------------|
| `pdftotext` | ✅ Excellent | ❌ | ❌ | ❌ |
| `pdftoppm` | ❌ | ✅ Excellent | ❌ | ❌ |
| `pdfinfo` | ❌ | ❌ | ✅ Complete | ❌ |
| `qpdf` | ❌ | ❌ | ❌ | ✅ Complete |
| `ghostscript` | ⚠️ Basic | ✅ Complete | ⚠️ Basic | ✅ Complete |

---

# 2. OCR Tools

## 2.1 Tesseract

### Overview
Open-source OCR engine. Supports 100+ languages. Industry standard for OSS OCR.

### Package Names

| Distribution | Package | Language Data |
|-------------|---------|---------------|
| Arch Linux | `tesseract` | `tesseract-data-eng`, etc. |
| Debian/Ubuntu | `tesseract-ocr` | `tesseract-ocr-eng`, etc. |
| Fedora | `tesseract` | `tesseract-langpack-eng` |
| Termux | `tesseract` | In unstable repo |

### Detection
```bash
which tesseract || command -v tesseract
tesseract --version
tesseract --list-langs
```

### Command-Line Usage

```bash
# Basic OCR to stdout
tesseract image.png stdout

# OCR to file
tesseract image.png output

# Specify language
tesseract image.png stdout -l eng
tesseract image.png stdout -l eng+spa  # Multiple languages

# Page segmentation modes
tesseract image.png stdout --psm 3   # Auto (default)
tesseract image.png stdout --psm 6   # Single block
tesseract image.png stdout --psm 7   # Single line
tesseract image.png stdout --psm 8   # Single word
tesseract image.png stdout --psm 13  # Raw line

# Output formats
tesseract image.png output hocr    # hOCR (HTML)
tesseract image.png output pdf     # Searchable PDF
tesseract image.png output tsv     # TSV (bounding boxes)

# LSTM mode
tesseract image.png stdout --oem 1  # LSTM only
```

### Page Segmentation Modes (PSM)

| PSM | Description |
|-----|-------------|
| 0 | Orientation and script detection only |
| 1 | Auto with OSD |
| 2 | Auto (no OSD or OCR) |
| 3 | Fully automatic (default) |
| 4 | Single column |
| 5 | Single vertical text block |
| 6 | Single uniform block |
| 7 | Single line |
| 8 | Single word |
| 9 | Single word in circle |
| 10 | Single character |
| 11 | Sparse text |
| 12 | Sparse text with OSD |
| 13 | Raw line (no preprocessing) |

### OCR Engine Modes (OEM)

| OEM | Description |
|-----|-------------|
| 0 | Legacy engine only |
| 1 | Neural net LSTM engine only |
| 2 | Legacy + LSTM |
| 3 | Default (whatever is available) |

### Language Codes

| Code | Language |
|------|----------|
| `eng` | English |
| `por` | Portuguese |
| `spa` | Spanish |
| `fra` | French |
| `deu` | German |
| `jpn` | Japanese |
| `chi_sim` | Chinese Simplified |
| `chi_tra` | Chinese Traditional |

Full list: `tesseract --list-langs`

### Security Considerations

1. **Image parsing**: Tesseract parses various image formats
2. **Memory usage**: Large images can consume RAM
3. **No network access**: Safe from remote exploitation

### Rust Integration

```rust
fn ocr_image(path: &str, lang: Option<&str>) -> Result<String, String> {
    let mut cmd = Command::new("tesseract");
    cmd.arg(path).arg("stdout");
    
    if let Some(l) = lang {
        cmd.args(["-l", l]);
    }
    
    let output = cmd.output()
        .map_err(|e| format!("Failed to run tesseract: {}", e))?;
    
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
```

---

# 3. Image Processing Tools

## 3.1 ImageMagick (magick/convert)

### Overview
Comprehensive image manipulation suite. Format conversion, resizing, effects, etc.

### Package Names

| Distribution | Package |
|-------------|---------|
| Arch Linux | `imagemagick` |
| Debian/Ubuntu | `imagemagick` |
| Fedora | `ImageMagick` |
| Termux | `imagemagick` |

### Detection
```bash
which magick || which convert || command -v magick
magick --version
```

### Command-Line Usage

```bash
# Convert format
magick input.png output.jpg
magick convert input.png output.jpg  # Legacy command

# Resize
magick input.png -resize 50% output.png
magick input.png -resize 800x600 output.png

# Crop
magick input.png -crop 100x100+50+50 output.png

# Rotate
magick input.png -rotate 90 output.png

# Get info
magick identify image.png
magick identify -verbose image.png

# Batch convert
magick mogrify -resize 50% *.png  # Overwrites originals

# Convert PDF page to image
magick convert -density 300 input.pdf[0] output.png

# Grayscale
magick input.png -colorspace Gray output.png

# Quality (JPEG)
magick input.png -quality 85 output.jpg

# Thumbnail
magick input.png -thumbnail 200x200 output.png
```

### Security Considerations

⚠️ **ImageMagick has a history of CVEs** (ImageTragick, etc.)

#### Mitigations

1. **Disable dangerous coders** in `/etc/ImageMagick-7/policy.xml`:

```xml
<policy domain="coder" rights="none" pattern="EPHEMERAL" />
<policy domain="coder" rights="none" pattern="URL" />
<policy domain="coder" rights="none" pattern="HTTPS" />
<policy domain="coder" rights="none" pattern="HTTP" />
<policy domain="coder" rights="none" pattern="FTP" />
<policy domain="coder" rights="none" pattern="MVG" />
<policy domain="coder" rights="none" pattern="MSL" />
```

2. **Resource limits**:

```xml
<policy domain="resource" name="memory" value="256MiB"/>
<policy domain="resource" name="map" value="512MiB"/>
<policy domain="resource" name="width" value="16KP"/>
<policy domain="resource" name="height" value="16KP"/>
<policy domain="resource" name="area" value="128MB"/>
<policy domain="resource" name="disk" value="1GiB"/>
```

3. **Use `-sandbox` flag**:

```bash
magick -sandbox input.png -resize 100x100 output.png
```

4. **Avoid URLs** - Never process images from untrusted URLs

### Recommendations for sprachspiel

1. **Default: disabled** in `tools.toml` (opt-in)
2. **If enabled: sandbox = true**
3. **Validate input**: Check file magic bytes before processing
4. **Set timeout**: Limit maximum execution time

---

## 3.2 ExifTool

### Overview
Metadata extraction and manipulation. Supports EXIF, GPS, IPTC, XMP, etc.

### Package Names

| Distribution | Package |
|-------------|---------|
| Arch Linux | `perl-image-exiftool` |
| Debian/Ubuntu | `libimage-exiftool-perl` |
| Fedora | `perl-Image-ExifTool` |
| Termux | `exiftool` (unstable) |

### Detection
```bash
which exiftool || command -v exiftool
exiftool -ver
```

### Command-Line Usage

```bash
# Get all metadata
exiftool image.jpg

# Specific tags
exiftool -Artist -Creator -FileType image.jpg

# GPS coordinates
exiftool -GPSLatitude -GPSLongitude image.jpg

# JSON output
exiftool -json image.jpg

# CSV output (recursive)
exiftool -csv -r . > metadata.csv

# Extract thumbnail
exiftool -b -ThumbnailImage image.jpg > thumb.jpg

# Remove all metadata (privacy)
exiftool -all= image.jpg

# Remove GPS only
exiftool -gps:all= image.jpg

# Set copyright
exiftool -Artist="John Doe" image.jpg

# Rename by date
exiftool "-filename<CreateDate" -d "%Y-%m-%d_%H-%M-%S.%%e" image.jpg
```

### Security Considerations

1. **Generally safe**: Primarily a file reader
2. **No network access**: Safe for sensitive data
3. **Memory usage**: May consume RAM on very large files
4. **Backup files**: Creates `_original` backup by default

### Rust Integration

```rust
fn get_image_metadata(path: &str) -> Result<String, String> {
    let output = Command::new("exiftool")
        .arg("-json")
        .arg(path)
        .output()
        .map_err(|e| format!("Failed to run exiftool: {}", e))?;
    
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
```

---

# 4. Video Tools (Optional)

## 4.1 FFmpeg

### Overview
Video and audio processing. Frame extraction, format conversion, etc.

### Package Names

| Distribution | Package |
|-------------|---------|
| Arch Linux | `ffmpeg` |
| Debian/Ubuntu | `ffmpeg` |
| Fedora | `ffmpeg` (rpmfusion) |
| Termux | `ffmpeg` |

### Detection
```bash
which ffmpeg || command -v ffmpeg
ffmpeg -version
```

### Command-Line Usage

```bash
# Extract frame from video
ffmpeg -i video.mp4 -vf "select=eq(n\,0)" -frames:v 1 frame.png
ffmpeg -i video.mp4 -ss 00:00:05 -frames:v 1 frame.png  # At 5s

# Extract frames at 1 fps
ffmpeg -i video.mp4 -vf "fps=1" out%d.png

# Convert video format
ffmpeg -i input.mp4 output.webm

# Resize
ffmpeg -i input.mp4 -vf scale=1280:720 output.mp4

# Extract audio
ffmpeg -i video.mp4 -vn -acodec copy audio.aac

# Get video info
ffprobe -v quiet -print_format json -show_format -show_streams video.mp4
```

### Security Considerations

1. **CVE history**: Multiple vulnerabilities in codec parsers
2. **Resource intensive**: Long videos consume CPU/RAM
3. **Network protocols**: Supports HTTP, RTMP, etc.

### Recommendations

1. **Default: disabled** in `tools.toml`
2. **If enabled**: Set strict timeout
3. **Disable network**: Use `-protocol_whitelist file,pipe`
4. **Consider sandbox**: firejail or container

---

# 5. Integration Patterns

## 5.1 Detection Pattern

```rust
use which::which;

pub struct ToolRegistry {
    tools: HashMap<String, bool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        
        // Check all known tools
        for tool in &["pdftotext", "pdfinfo", "pdftoppm", "tesseract", "exiftool", "magick", "ffmpeg"] {
            registry.tools.insert(tool.to_string(), which(tool).is_ok());
        }
        
        registry
    }
    
    pub fn is_available(&self, tool: &str) -> bool {
        self.tools.get(tool).copied().unwrap_or(false)
    }
    
    pub fn list_available(&self) -> Vec<&str> {
        self.tools.iter()
            .filter(|(_, &available)| available)
            .map(|(name, _)| name.as_str())
            .collect()
    }
}
```

## 5.2 Execution Pattern

```rust
use std::process::Command;
use std::time::Duration;
use tokio::time::timeout;

pub async fn execute_with_timeout(
    program: &str,
    args: &[&str],
    timeout_secs: u64,
) -> Result<String, String> {
    // Check program exists
    let program_path = which::which(program)
        .map_err(|e| format!("Program '{}' not found: {}", program, e))?;
    
    // Build command
    let mut cmd = Command::new(&program_path);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    // Execute with timeout
    let output_future = async { cmd.output() };
    
    let result = timeout(Duration::from_secs(timeout_secs), output_future)
        .await
        .map_err(|_| format!("Command timed out after {} seconds", timeout_secs))?
        .map_err(|e| format!("Execution failed: {}", e))?;
    
    if result.status.success() {
        Ok(String::from_utf8_lossy(&result.stdout).to_string())
    } else {
        Err(format!(
            "Command failed ({:?}): {}",
            result.status.code(),
            String::from_utf8_lossy(&result.stderr)
        ))
    }
}
```

## 5.3 Safe Argument Pattern

```rust
use shell_words::quote;

/// Safely build command arguments
pub fn safe_args(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
    // Note: When using Command::new().args(), arguments are already safe
    // No shell interpretation occurs
    // quote() is only needed if passing to shell - which we DON'T do
}
```

## 5.4 Error Classification Pattern

```rust
#[derive(Debug)]
pub enum ToolError {
    NotInstalled(String),
    Timeout(String),
    ExecutionFailed(String),
    InvalidOutput(String),
    Disabled(String),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled(tool) => write!(f, 
                "Tool '{}' is not installed. Install with your package manager.", tool),
            Self::Timeout(tool) => write!(f, 
                "Tool '{}' timed out. Try increasing timeout or reducing input size.", tool),
            Self::ExecutionFailed(msg) => write!(f, "{}", msg),
            Self::InvalidOutput(msg) => write!(f, "Tool returned invalid output: {}", msg),
            Self::Disabled(tool) => write!(f, 
                "Tool '{}' is disabled in tools.toml. Enable it to use.", tool),
        }
    }
}
```

---

# 6. Installation Summary

## Quick Install Commands

### Arch Linux
```bash
sudo pacman -S poppler tesseract imagemagick exiftool
# Optional: ghostscript qpdf
```

### Debian/Ubuntu
```bash
sudo apt install poppler-utils tesseract-ocr imagemagick exiftool
# Optional: ghostscript qpdf
```

### Fedora
```bash
sudo dnf install poppler-utils tesseract ImageMagick perl-Image-ExifTool
# Optional: ghostscript qpdf
```

### Termux (Android)
```bash
pkg install poppler tesseract imagemagick exiftool
# Some packages may be in unstable repo
```

---

# 7. Recommended Tool Availability

For sprachspiel to work optimally with external tools:

| Tool | Priority | Feature |
|------|----------|---------|
| `pdftotext` | High | PDF text extraction |
| `pdfinfo` | High | PDF metadata |
| `tesseract` | High | OCR |
| `exiftool` | Medium | Image metadata |
| `pdftoppm` | Medium | PDF to image (for OCR) |
| `imagemagick` | Low | Image manipulation |
| `ffmpeg` | Optional | Video frame extraction |
| `ghostscript` | Optional | Advanced PDF |
| `qpdf` | Optional | PDF manipulation |

---

# 8. Security Summary

## High-Risk Tools

| Tool | Risk Level | Mitigation |
|------|-------------|-------------|
| `imagemagick` | HIGH | Sandbox + policy.xml |
| `ghostscript` | HIGH | `-dSAFER` flag + sandbox |
| `ffmpeg` | MEDIUM | Timeout + no network |
| `tesseract` | LOW | Validate input |
| `pdftotext` | LOW | None needed |
| `pdfinfo` | LOW | None needed |
| `exiftool` | LOW | None needed |
| `qpdf` | LOW | None needed |

## Recommended Defaults in tools.toml

```toml
[pdftotext]
enabled = true
timeout = 30

[pdfinfo]
enabled = true
timeout = 5

[tesseract]
enabled = true
timeout = 120

[exiftool]
enabled = true
timeout = 10

[pdftoppm]
enabled = true
timeout = 60
sandbox = false  # Safe with timeout

[imagemagick]
enabled = false  # Opt-in only
timeout = 60
sandbox = true    # REQUIRED if enabled

[ffmpeg]
enabled = false   # Opt-in only
timeout = 300
sandbox = true
```

---

# 9. See Also

- [Skills System Design](./skills-system-design.md) - How skills use these tools
- [Roadmap](./roadmap.md) - Implementation plan
- Poppler documentation: https://poppler.freedesktop.org/
- Tesseract documentation: https://tesseract-ocr.github.io/