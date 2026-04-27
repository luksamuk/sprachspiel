//! Vision processor
//!
//! Handles image and PDF analysis using vision models via Ollama.
//! Uses /api/generate endpoint with images array for multi-image support.
//! PDF files are converted to PNG pages via pdftoppm and processed page-by-page.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::generation::images::Image;
use ollama_rs::models::ModelOptions;
use ollama_rs::Ollama;

use crate::spinner::{create_spinner, finish_spinner};
use crate::utils::{read_file_as_base64, validate_file_for_vision, validate_image_file, PDF_EXTENSION};

use super::cli::{parse_page_range, VisionArgs};
use super::error::{VisionError, VisionResult};

/// Number of pages between checkpoints when processing PDFs
const CHECKPOINT_INTERVAL: usize = 20;

pub struct VisionProcessor;

impl VisionProcessor {
    pub fn new() -> Self {
        Self
    }

    pub async fn process(
        &self,
        args: &VisionArgs,
        model: &str,
        ollama: &Ollama,
        model_options: ModelOptions,
        show_spinner: bool,
    ) -> VisionResult<VisionOutput> {
        if args.files.is_empty() {
            return Err(VisionError::NoImages);
        }

        // Separate PDF files from image files
        let (pdf_files, image_files): (Vec<_>, Vec<_>) =
            args.files.iter().partition(|f| is_pdf_file(f));

        // Process image files (single batch, existing behavior)
        let mut all_content = String::new();
        let mut all_files: Vec<String> = Vec::new();

        if !image_files.is_empty() {
            for file in &image_files {
                validate_image_file(file).map_err(VisionError::FileNotFound)?;
            }

            let images = self.load_images(&image_files).await?;
            let prompt = args.get_prompt().to_string();
            let model_opts = model_options.clone().num_predict(args.max_tokens as i32);

            let spinner_msg = if image_files.len() == 1 {
                format!("Analyzing {}...", image_files[0].display())
            } else {
                format!("Analyzing {} images...", image_files.len())
            };
            let spinner = if show_spinner {
                Some(create_spinner(&spinner_msg))
            } else {
                None
            };

            let result = self
                .call_vision_model(model, &prompt, images, model_opts, ollama)
                .await?;

            if let Some(sp) = spinner {
                finish_spinner(sp);
            }

            all_content.push_str(&result);
            all_content.push('\n');
            all_files.extend(image_files.iter().map(|p| p.to_string_lossy().to_string()));
        }

        // Process PDF files (page by page with checkpoints)
        for pdf in &pdf_files {
            validate_file_for_vision(pdf).map_err(VisionError::FileNotFound)?;

            let pdf_output = self
                .process_pdf(pdf, args, model, ollama, model_options.clone(), show_spinner)
                .await?;

            all_content.push_str(&pdf_output.content);
            all_content.push('\n');
            all_files.push(pdf.to_string_lossy().to_string());
        }

        let content = all_content.trim().to_string();

        Ok(VisionOutput {
            files: all_files,
            prompt: args.get_prompt(),
            content,
        })
    }

    /// Load multiple image files as base64 Image objects
    async fn load_images(&self, files: &[&PathBuf]) -> VisionResult<Vec<Image>> {
        let mut images = Vec::new();
        for file in files {
            let base64 = read_file_as_base64(file)
                .await
                .map_err(|e| VisionError::ReadFailed {
                    file: file.to_string_lossy().to_string(),
                    error: e,
                })?;
            images.push(Image::from_base64(base64));
        }
        Ok(images)
    }

    /// Call the vision model with images and prompt
    async fn call_vision_model(
        &self,
        model: &str,
        prompt: &str,
        images: Vec<Image>,
        model_options: ModelOptions,
        ollama: &Ollama,
    ) -> VisionResult<String> {
        let mut request = GenerationRequest::new(model.to_string(), prompt.to_string())
            .options(model_options);

        for image in images {
            request = request.add_image(image);
        }

        let response = ollama
            .generate(request)
            .await
            .map_err(|e| VisionError::OllamaError {
                message: format!("Failed to process image(s): {}", e),
            })?;

        Ok(response.response.trim().to_string())
    }

    /// Process a PDF file: convert pages to images, analyze each with vision
    async fn process_pdf(
        &self,
        pdf_path: &Path,
        args: &VisionArgs,
        model: &str,
        ollama: &Ollama,
        model_options: ModelOptions,
        show_spinner: bool,
    ) -> VisionResult<VisionOutput> {
        let pdf_str = pdf_path.to_string_lossy().to_string();

        // Step 1: Get page count via pdfinfo
        let total_pages = get_pdf_page_count(pdf_path).await?;

        // Step 2: Determine which pages to process
        let pages = if let Some(ref page_range) = args.pages {
            let requested = parse_page_range(page_range).map_err(|e| VisionError::PdfSupport {
                message: e,
            })?;
            // Clamp to actual page count
            requested
                .into_iter()
                .filter(|&p| p <= total_pages)
                .collect::<Vec<_>>()
        } else {
            (1..=total_pages).collect()
        };

        if pages.is_empty() {
            return Err(VisionError::PdfConversionError {
                message: format!(
                    "PDF has {} pages, no pages to process after range filtering",
                    total_pages
                ),
            });
        }

        log::debug!(
            "Processing PDF: {} ({} total pages, {} selected)",
            pdf_str,
            total_pages,
            pages.len()
        );

        // Step 3: Create temp directory for page images
        let temp_dir = create_pdf_temp_dir(pdf_path)?;
        let _cleanup = PdfTempCleanup(temp_dir.clone());

        // Step 4: Check for existing checkpoint (resume support)
        let checkpoint_path = temp_dir.join("checkpoint.json");
        let mut processed_pages: Vec<usize> = Vec::new();
        let mut results: Vec<(usize, String)> = Vec::new();

        if checkpoint_path.exists()
            && let Ok(saved) = load_checkpoint(&checkpoint_path)
        {
            log::debug!("Resuming from checkpoint: {} pages already processed", saved.len());
            processed_pages = saved.iter().map(|(p, _)| *p).collect();
            results = saved;
        }

        // Step 5: Convert and process pages
        let prompt = args.get_prompt().to_string();
        let model_opts = model_options.num_predict(args.max_tokens as i32);

        for (idx, &page_num) in pages.iter().enumerate() {
            // Skip already processed pages (from checkpoint)
            if processed_pages.contains(&page_num) {
                continue;
            }

            let spinner = if show_spinner {
                Some(create_spinner(&format!(
                    "Analyzing {} page {}/{}...",
                    pdf_path.display(),
                    idx + 1,
                    pages.len()
                )))
            } else {
                None
            };

            // Convert single page to PNG
            let img_path = convert_pdf_page(pdf_path, page_num, &temp_dir).await?;

            // Read as base64 and call vision model
            let base64 = read_file_as_base64(&img_path)
                .await
                .map_err(|e| VisionError::ReadFailed {
                    file: img_path.to_string_lossy().to_string(),
                    error: e,
                })?;

            let page_prompt = if pages.len() > 1 {
                format!("This is page {} of the document. {}", page_num, prompt)
            } else {
                prompt.clone()
            };

            let images = vec![Image::from_base64(base64)];
            let result = self
                .call_vision_model(model, &page_prompt, images, model_opts.clone(), ollama)
                .await?;

            if let Some(sp) = spinner {
                finish_spinner(sp);
            }

            // Clean up individual page image
            let _ = std::fs::remove_file(&img_path);

            results.push((page_num, result));
            processed_pages.push(page_num);

            // Save checkpoint every CHECKPOINT_INTERVAL pages
            if processed_pages.len().is_multiple_of(CHECKPOINT_INTERVAL) {
                save_checkpoint(&checkpoint_path, &results);
                log::debug!(
                    "Checkpoint saved: {} pages processed",
                    processed_pages.len()
                );
            }
        }

        // Remove checkpoint on successful completion
        let _ = std::fs::remove_file(&checkpoint_path);

        // Sort results by page number and combine
        results.sort_by_key(|(p, _)| *p);
        let content = results
            .iter()
            .map(|(page, text)| {
                if pages.len() > 1 {
                    format!("--- Page {} ---\n{}", page, text)
                } else {
                    text.clone()
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(VisionOutput {
            files: vec![pdf_str],
            prompt,
            content,
        })
    }
}

impl Default for VisionProcessor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PDF utility functions
// ---------------------------------------------------------------------------

/// Check if a file is a PDF based on extension
fn is_pdf_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case(PDF_EXTENSION))
        .unwrap_or(false)
}

/// Get PDF page count using pdfinfo
async fn get_pdf_page_count(pdf_path: &Path) -> VisionResult<usize> {
    let output = tokio::process::Command::new("pdfinfo")
        .arg(pdf_path)
        .output()
        .await
        .map_err(|e| VisionError::PdfSupport {
            message: format!("pdfinfo not found: {}", e),
        })?;

    if !output.status.success() {
        return Err(VisionError::PdfConversionError {
            message: format!(
                "pdfinfo failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.starts_with("Pages:") {
            let count: usize = line
                .trim_start_matches("Pages:")
                .trim()
                .parse()
                .unwrap_or(0);
            if count == 0 {
                return Err(VisionError::PdfConversionError {
                    message: "PDF has 0 pages".to_string(),
                });
            }
            return Ok(count);
        }
    }

    Err(VisionError::PdfConversionError {
        message: "Could not determine page count from pdfinfo output".to_string(),
    })
}

/// Create a temp directory for PDF page images
fn create_pdf_temp_dir(pdf_path: &Path) -> VisionResult<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let stem = pdf_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("pdf");
    let dir_name = format!("ask-ai-pdf-{}-{}", stem, timestamp);
    let temp_dir = std::env::temp_dir().join(dir_name);

    std::fs::create_dir_all(&temp_dir).map_err(|e| VisionError::PdfConversionError {
        message: format!("Failed to create temp directory: {}", e),
    })?;

    Ok(temp_dir)
}

/// Convert a single PDF page to PNG using pdftoppm
async fn convert_pdf_page(pdf_path: &Path, page_num: usize, temp_dir: &Path) -> VisionResult<PathBuf> {
    let output_prefix = temp_dir.join("page");

    let output = tokio::process::Command::new("pdftoppm")
        .arg("-png")
        .arg("-f")
        .arg(page_num.to_string())
        .arg("-l")
        .arg(page_num.to_string())
        .arg("-r")
        .arg("150") // 150 DPI — good balance of quality vs size
        .arg(pdf_path)
        .arg(&output_prefix)
        .output()
        .await
        .map_err(|e| VisionError::PdfSupport {
            message: format!("pdftoppm not found: {}", e),
        })?;

    if !output.status.success() {
        return Err(VisionError::PdfConversionError {
            message: format!(
                "pdftoppm failed for page {}: {}",
                page_num,
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    // pdftoppm outputs: prefix-NNN.png (zero-padded page number)
    let expected = format!("page-{:03}.png", page_num);
    let img_path = temp_dir.join(&expected);

    if img_path.exists() {
        Ok(img_path)
    } else {
        // Try alternate naming (some pdftoppm versions use different padding)
        let alt = temp_dir.join(format!("page-{}.png", page_num));
        if alt.exists() {
            Ok(alt)
        } else {
            Err(VisionError::PdfConversionError {
                message: format!(
                    "pdftoppm produced no output image for page {} (expected {})",
                    page_num,
                    expected
                ),
            })
        }
    }
}

/// Save checkpoint progress to a JSON file
fn save_checkpoint(path: &Path, results: &[(usize, String)]) {
    let json = serde_json::json!({
        "results": results.iter().map(|(p, t)| serde_json::json!({
            "page": p,
            "content": t,
        })).collect::<Vec<_>>()
    });

    if let Ok(data) = serde_json::to_string_pretty(&json) {
        let _ = std::fs::write(path, data);
    }
}

/// Load checkpoint progress from a JSON file
fn load_checkpoint(path: &Path) -> VisionResult<Vec<(usize, String)>> {
    let data = std::fs::read_to_string(path).map_err(|e| VisionError::PdfConversionError {
        message: format!("Failed to read checkpoint: {}", e),
    })?;

    let json: serde_json::Value = serde_json::from_str(&data).map_err(|e| VisionError::PdfConversionError {
        message: format!("Failed to parse checkpoint: {}", e),
    })?;

    let mut results = Vec::new();
    if let Some(arr) = json["results"].as_array() {
        for entry in arr {
            if let (Some(page), Some(content)) = (
                entry["page"].as_u64(),
                entry["content"].as_str(),
            ) {
                results.push((page as usize, content.to_string()));
            }
        }
    }

    Ok(results)
}

/// RAII cleanup for temp directory
struct PdfTempCleanup(PathBuf);

impl Drop for PdfTempCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// Output types and printing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VisionOutput {
    pub files: Vec<String>,
    pub prompt: String,
    pub content: String,
}

pub fn print_results(result: &VisionOutput, json_output: bool) {
    if json_output {
        let json = serde_json::json!({
            "files": result.files,
            "prompt": result.prompt,
            "content": result.content,
        });
        println!("{}", json);
    } else {
        let is_multi = result.files.len() > 1;

        if is_multi {
            println!("Files: {}", result.files.join(", "));
            println!();
        }

        println!("{}", result.content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_validate_image_file() {
        assert!(validate_image_file(Path::new("test.png")).is_err()); // doesn't exist
        assert!(validate_image_file(Path::new("test.jpg")).is_err());
        assert!(validate_image_file(Path::new("test.pdf")).is_err());
    }

    #[test]
    fn test_is_pdf_file() {
        assert!(is_pdf_file(Path::new("doc.pdf")));
        assert!(is_pdf_file(Path::new("doc.PDF")));
        assert!(!is_pdf_file(Path::new("doc.png")));
        assert!(!is_pdf_file(Path::new("doc")));
    }

    #[test]
    fn test_save_and_load_checkpoint() {
        let dir = std::env::temp_dir().join("ask-ai-test-checkpoint");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("checkpoint.json");

        let results = vec![
            (1, "Page 1 content".to_string()),
            (2, "Page 2 content".to_string()),
        ];

        save_checkpoint(&path, &results);
        let loaded = load_checkpoint(&path).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].0, 1);
        assert_eq!(loaded[0].1, "Page 1 content");
        assert_eq!(loaded[1].0, 2);
        assert_eq!(loaded[1].1, "Page 2 content");

        let _ = std::fs::remove_dir_all(&dir);
    }
}