use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_DATASET: &str = "playground/handwriting/dataset/manifest.json";
const DEFAULT_PROMPT: &str = "playground/handwriting/prompts/transcribe_verbatim.md";
const DEFAULT_RESULTS: &str = "playground/handwriting/results";
const DEFAULT_MODEL: &str = "gpt-4.1-mini-2025-04-14";

#[derive(Debug, Deserialize)]
struct Dataset {
    #[serde(default)]
    name: String,
    cases: Vec<DatasetCase>,
}

#[derive(Debug, Deserialize)]
struct DatasetCase {
    id: String,
    images: Vec<PathBuf>,
    expected_text: PathBuf,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    notes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BenchmarkReport {
    dataset: String,
    pipeline: String,
    generated_at_unix_seconds: u64,
    aggregate: AggregateMetrics,
    cases: Vec<CaseReport>,
}

#[derive(Debug, Serialize)]
struct AggregateMetrics {
    case_count: usize,
    successful_case_count: usize,
    mean_character_error_rate: Option<f64>,
    mean_word_error_rate: Option<f64>,
    total_latency_ms: u128,
    total_input_tokens: u64,
    total_cached_input_tokens: u64,
    total_output_tokens: u64,
    estimated_cost_usd: Option<f64>,
}

#[derive(Debug, Serialize)]
struct CaseReport {
    id: String,
    image_count: usize,
    language: Option<String>,
    notes: Vec<String>,
    prediction_path: Option<PathBuf>,
    ground_truth_available: bool,
    character_error_rate: Option<f64>,
    word_error_rate: Option<f64>,
    exact_match_after_normalization: Option<bool>,
    latency_ms: u128,
    input_tokens: u64,
    cached_input_tokens: u64,
    output_tokens: u64,
    estimated_cost_usd: Option<f64>,
    uncertainties: Vec<UncertainSpan>,
    ocr_words: Vec<OcrWord>,
    disagreements: Vec<OcrDisagreement>,
    error: Option<String>,
}

struct PipelineOutput {
    text: String,
    latency_ms: u128,
    input_tokens: u64,
    cached_input_tokens: u64,
    output_tokens: u64,
    estimated_cost_usd: Option<f64>,
    uncertainties: Vec<UncertainSpan>,
    ocr_words: Vec<OcrWord>,
    disagreements: Vec<OcrDisagreement>,
}

#[derive(Debug, Clone, Serialize)]
struct OcrWord {
    page_number: u64,
    text: String,
    confidence: f64,
    polygon: Vec<OcrPoint>,
}

#[derive(Debug, Clone, Serialize)]
struct OcrPoint {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Serialize)]
struct OcrDisagreement {
    page_number: u64,
    ocr_text: String,
    ocr_confidence: f64,
    gpt_text_present: bool,
    reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct UncertainSpan {
    page_number: u64,
    transcribed_text: String,
    surrounding_context: String,
    confidence: String,
    alternatives: Vec<String>,
    reason: String,
}

#[derive(Deserialize)]
struct ConfidenceOutput {
    transcription: String,
    uncertainties: Vec<UncertainSpan>,
}

#[derive(Clone, Copy)]
enum OutputMode {
    Plain,
    Confidence,
}

impl OutputMode {
    fn from_cli(value: Option<String>) -> Result<Self> {
        match value.as_deref().unwrap_or("plain") {
            "plain" => Ok(Self::Plain),
            "confidence" => Ok(Self::Confidence),
            other => bail!("unknown output mode '{other}' (use plain or confidence)"),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Confidence => "confidence",
        }
    }
}

#[async_trait]
trait TranscriptionPipeline {
    fn name(&self) -> String;

    async fn transcribe(&self, case: &ResolvedCase) -> Result<PipelineOutput>;
}

struct ResolvedCase {
    id: String,
    images: Vec<PathBuf>,
}

struct DirectoryPipeline {
    directory: PathBuf,
}

#[async_trait]
impl TranscriptionPipeline for DirectoryPipeline {
    fn name(&self) -> String {
        format!("directory:{}", self.directory.display())
    }

    async fn transcribe(&self, case: &ResolvedCase) -> Result<PipelineOutput> {
        let started_at = Instant::now();
        let path = self.directory.join(format!("{}.txt", case.id));
        let text = fs::read_to_string(&path)
            .with_context(|| format!("cannot read prediction {}", path.display()))?;
        Ok(PipelineOutput {
            text,
            latency_ms: started_at.elapsed().as_millis(),
            input_tokens: 0,
            cached_input_tokens: 0,
            output_tokens: 0,
            estimated_cost_usd: None,
            uncertainties: Vec::new(),
            ocr_words: Vec::new(),
            disagreements: Vec::new(),
        })
    }
}

struct OpenAiVisionPipeline {
    model: String,
    prompt: String,
    output_mode: OutputMode,
    include_images: bool,
}

#[async_trait]
impl TranscriptionPipeline for OpenAiVisionPipeline {
    fn name(&self) -> String {
        format!("openai_vision:{}:{}", self.model, self.output_mode.as_str())
    }

    async fn transcribe(&self, case: &ResolvedCase) -> Result<PipelineOutput> {
        let api_key = env::var("OPENAI_API_KEY").context("OPENAI_API_KEY is required")?;
        let base_url = env::var("OPENAI_API_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com".to_string());
        let base_url = base_url.trim_end_matches('/');
        let url = if base_url.ends_with("/v1") {
            format!("{base_url}/responses")
        } else {
            format!("{base_url}/v1/responses")
        };

        let mode_instructions = match self.output_mode {
            OutputMode::Plain => "",
            OutputMode::Confidence => "\n\nAlso identify every locally uncertain reading. For each one, give its 1-based page number, the exact text chosen in the transcription, a short surrounding excerpt that uniquely locates it, low or medium confidence, plausible alternative readings, and a concise reason. Do not report high-confidence spans. The transcription must still contain your single best reading.",
        };
        let mut content = vec![json!({
            "type": "input_text",
            "text": format!("{}{}", self.prompt, mode_instructions),
        })];
        if self.include_images {
            for image_path in &case.images {
                let bytes = fs::read(image_path)
                    .with_context(|| format!("cannot read image {}", image_path.display()))?;
                let mime = mime_guess::from_path(image_path)
                    .first_raw()
                    .unwrap_or("application/octet-stream");
                content.push(json!({
                    "type": "input_image",
                    "image_url": format!("data:{mime};base64,{}", encode_base64(&bytes)),
                    "detail": "high",
                }));
            }
        }

        let mut request = json!({
            "model": self.model,
            "instructions": "Transcribe the supplied handwritten journal pages faithfully.",
            "input": [{ "role": "user", "content": content }],
            "max_output_tokens": 12000,
            "store": false,
        });
        if self.model.starts_with("gpt-5.6") {
            request["reasoning"] = json!({ "effort": "low" });
        } else {
            request["temperature"] = json!(0);
        }
        if matches!(self.output_mode, OutputMode::Confidence) {
            request["text"] = json!({
                "format": {
                    "type": "json_schema",
                    "name": "handwriting_transcription_with_uncertainties",
                    "strict": true,
                    "schema": confidence_output_schema()
                }
            });
        }
        let started_at = Instant::now();
        let response = reqwest::Client::new()
            .post(url)
            .bearer_auth(api_key)
            .json(&request)
            .send()
            .await
            .context("failed to call OpenAI Responses API")?;
        let status = response.status();
        let body = response.text().await.context("failed to read response")?;
        if !status.is_success() {
            bail!("OpenAI API returned {status}: {body}");
        }
        let value: serde_json::Value = serde_json::from_str(&body)?;
        let output_text = extract_output_text(&value)
            .ok_or_else(|| anyhow!("response contained no output_text"))?;
        let usage = value.get("usage");
        let (text, uncertainties) = match self.output_mode {
            OutputMode::Plain => (output_text, Vec::new()),
            OutputMode::Confidence => {
                let structured: ConfidenceOutput = serde_json::from_str(&output_text)
                    .context("structured transcription was not valid JSON")?;
                (structured.transcription, structured.uncertainties)
            }
        };
        let input_tokens = token_count(usage, "input_tokens");
        let cached_input_tokens = usage
            .and_then(|usage| usage.get("input_tokens_details"))
            .and_then(|details| details.get("cached_tokens"))
            .and_then(|tokens| tokens.as_u64())
            .unwrap_or(0);
        let output_tokens = token_count(usage, "output_tokens");
        Ok(PipelineOutput {
            text,
            latency_ms: started_at.elapsed().as_millis(),
            input_tokens,
            cached_input_tokens,
            output_tokens,
            estimated_cost_usd: estimate_cost_usd(
                &self.model,
                input_tokens,
                cached_input_tokens,
                output_tokens,
            ),
            uncertainties,
            ocr_words: Vec::new(),
            disagreements: Vec::new(),
        })
    }
}

struct GoogleDocumentAiPipeline {
    project: String,
    location: String,
    processor_id: String,
    low_confidence_threshold: f64,
}

#[async_trait]
impl TranscriptionPipeline for GoogleDocumentAiPipeline {
    fn name(&self) -> String {
        format!("google_document_ai:{}", self.low_confidence_threshold)
    }

    async fn transcribe(&self, case: &ResolvedCase) -> Result<PipelineOutput> {
        let access_token = google_access_token()?;
        let endpoint = format!(
            "https://{}-documentai.googleapis.com/v1/projects/{}/locations/{}/processors/{}:process",
            self.location, self.project, self.location, self.processor_id
        );
        let client = reqwest::Client::new();
        let started_at = Instant::now();
        let mut page_texts = Vec::with_capacity(case.images.len());
        let mut ocr_words = Vec::new();

        for (page_index, image_path) in case.images.iter().enumerate() {
            let bytes = fs::read(image_path)
                .with_context(|| format!("cannot read image {}", image_path.display()))?;
            let mime = mime_guess::from_path(image_path)
                .first_raw()
                .unwrap_or("application/octet-stream");
            let request = json!({
                "rawDocument": {
                    "content": encode_base64(&bytes),
                    "mimeType": mime
                },
                "processOptions": {
                    "ocrConfig": {
                        "hints": { "languageHints": ["fr"] }
                    }
                }
            });
            let response = client
                .post(&endpoint)
                .bearer_auth(&access_token)
                .json(&request)
                .send()
                .await
                .context("failed to call Google Document AI")?;
            let status = response.status();
            let body = response.text().await.context("failed to read response")?;
            if !status.is_success() {
                bail!("Google Document AI returned {status}: {body}");
            }
            let value: serde_json::Value = serde_json::from_str(&body)?;
            let document = value
                .get("document")
                .ok_or_else(|| anyhow!("Google response contained no document"))?;
            let page_text = document
                .get("text")
                .and_then(|text| text.as_str())
                .unwrap_or_default();
            extract_google_words(document, page_text, page_index as u64 + 1, &mut ocr_words);
            page_texts.push(page_text.trim().to_string());
        }

        let uncertainties = ocr_words
            .iter()
            .filter(|word| word.confidence < self.low_confidence_threshold)
            .map(|word| UncertainSpan {
                page_number: word.page_number,
                transcribed_text: word.text.clone(),
                surrounding_context: word.text.clone(),
                confidence: if word.confidence < 0.5 {
                    "low".to_string()
                } else {
                    "medium".to_string()
                },
                alternatives: Vec::new(),
                reason: format!(
                    "Google OCR word confidence: {:.1}%",
                    word.confidence * 100.0
                ),
            })
            .collect();
        Ok(PipelineOutput {
            text: page_texts.join("\n\n"),
            latency_ms: started_at.elapsed().as_millis(),
            input_tokens: 0,
            cached_input_tokens: 0,
            output_tokens: 0,
            estimated_cost_usd: Some(case.images.len() as f64 * 0.0015),
            uncertainties,
            ocr_words,
            disagreements: Vec::new(),
        })
    }
}

struct GoogleThenOpenAiPipeline {
    google: GoogleDocumentAiPipeline,
    model: String,
    prompt: String,
}

struct ParallelConsensusPipeline {
    google: GoogleDocumentAiPipeline,
    model: String,
    prompt: String,
}

struct CandidateAdjudicationPipeline {
    google: GoogleDocumentAiPipeline,
    base_model: String,
    judge_model: String,
    prompt: String,
}

#[async_trait]
impl TranscriptionPipeline for CandidateAdjudicationPipeline {
    fn name(&self) -> String {
        format!("candidate_adjudication:{}", self.judge_model)
    }

    async fn transcribe(&self, case: &ResolvedCase) -> Result<PipelineOutput> {
        let base = OpenAiVisionPipeline {
            model: self.base_model.clone(),
            prompt: self.prompt.clone(),
            output_mode: OutputMode::Plain,
            include_images: true,
        };
        let (google_result, base_result) =
            tokio::join!(self.google.transcribe(case), base.transcribe(case));
        let google = google_result?;
        let base = base_result?;
        let low_confidence_words = google
            .ocr_words
            .iter()
            .filter(|word| word.confidence < self.google.low_confidence_threshold)
            .map(|word| {
                format!(
                    "page {}: {:?} ({:.1}%)",
                    word.page_number,
                    word.text,
                    word.confidence * 100.0
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let judge_prompt = format!(
            "{}\n\nYou are an uncertainty adjudicator, not a re-transcriber. You do not have access to the page images. GPT-4.1's complete transcription is the safest global reading and must remain the canonical text. Compare it with the independent OCR candidate and identify only the most important places where the candidates materially disagree or where OCR confidence is low. Return at most 12 challenges, with short excerpts and reasons, and at most 3 concise alternatives per challenge. Preserve the GPT reading in transcribed_text. Do not rewrite the transcription and do not invent certainty from text alone.\n\n--- GOOGLE OCR CANDIDATE ---\n{}\n\n--- GPT-4.1 CANDIDATE (CANONICAL) ---\n{}\n\n--- GOOGLE LOW-CONFIDENCE TOKENS ---\n{}",
            self.prompt, google.text, base.text, low_confidence_words
        );
        let judge = OpenAiVisionPipeline {
            model: self.judge_model.clone(),
            prompt: judge_prompt,
            output_mode: OutputMode::Confidence,
            include_images: false,
        }
        .transcribe(case)
        .await?;
        let mut disagreements = Vec::new();
        let base_text = normalize(&base.text);
        for word in &google.ocr_words {
            if word.confidence < self.google.low_confidence_threshold
                && !base_text
                    .split_whitespace()
                    .any(|candidate| candidate == normalize(&word.text))
            {
                disagreements.push(OcrDisagreement {
                    page_number: word.page_number,
                    ocr_text: word.text.clone(),
                    ocr_confidence: word.confidence,
                    gpt_text_present: false,
                    reason:
                        "Google OCR and GPT-4.1 candidates disagree; sent to GPT-5.6 adjudicator"
                            .to_string(),
                });
            }
        }
        Ok(PipelineOutput {
            text: base.text,
            latency_ms: google.latency_ms.max(base.latency_ms) + judge.latency_ms,
            input_tokens: base.input_tokens + judge.input_tokens,
            cached_input_tokens: base.cached_input_tokens + judge.cached_input_tokens,
            output_tokens: base.output_tokens + judge.output_tokens,
            estimated_cost_usd: sum_optional_costs(&[
                google.estimated_cost_usd,
                base.estimated_cost_usd,
                judge.estimated_cost_usd,
            ]),
            uncertainties: judge.uncertainties,
            ocr_words: google.ocr_words,
            disagreements,
        })
    }
}

#[async_trait]
impl TranscriptionPipeline for ParallelConsensusPipeline {
    fn name(&self) -> String {
        format!("parallel_consensus:{}", self.model)
    }

    async fn transcribe(&self, case: &ResolvedCase) -> Result<PipelineOutput> {
        let openai = OpenAiVisionPipeline {
            model: self.model.clone(),
            prompt: self.prompt.clone(),
            output_mode: OutputMode::Plain,
            include_images: true,
        };
        let (google_result, openai_result) =
            tokio::join!(self.google.transcribe(case), openai.transcribe(case));
        let google = google_result?;
        let openai = openai_result?;
        let final_text = normalize(&openai.text);
        let disagreements = google
            .ocr_words
            .iter()
            .filter(|word| word.confidence < self.google.low_confidence_threshold)
            .filter_map(|word| {
                let ocr_text = normalize(&word.text);
                if ocr_text.is_empty() {
                    return None;
                }
                let gpt_text_present = final_text
                    .split_whitespace()
                    .any(|candidate| candidate == ocr_text);
                (!gpt_text_present).then(|| OcrDisagreement {
                    page_number: word.page_number,
                    ocr_text: word.text.clone(),
                    ocr_confidence: word.confidence,
                    gpt_text_present,
                    reason: "low-confidence Google OCR token does not occur as the same word in the GPT transcription".to_string(),
                })
            })
            .collect();
        Ok(PipelineOutput {
            text: openai.text,
            latency_ms: google.latency_ms.max(openai.latency_ms),
            input_tokens: openai.input_tokens,
            cached_input_tokens: openai.cached_input_tokens,
            output_tokens: openai.output_tokens,
            estimated_cost_usd: match (google.estimated_cost_usd, openai.estimated_cost_usd) {
                (Some(google), Some(openai)) => Some(google + openai),
                _ => None,
            },
            uncertainties: google.uncertainties,
            ocr_words: google.ocr_words,
            disagreements,
        })
    }
}

#[async_trait]
impl TranscriptionPipeline for GoogleThenOpenAiPipeline {
    fn name(&self) -> String {
        format!("google_then_openai:{}", self.model)
    }

    async fn transcribe(&self, case: &ResolvedCase) -> Result<PipelineOutput> {
        let google = self.google.transcribe(case).await?;
        let low_confidence_words = google
            .ocr_words
            .iter()
            .filter(|word| word.confidence < self.google.low_confidence_threshold)
            .map(|word| {
                format!(
                    "page {}: {:?} ({:.1}%)",
                    word.page_number,
                    word.text,
                    word.confidence * 100.0
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let combined_prompt = format!(
            "{}\n\nA separate OCR engine produced the draft below. It is noisy and is evidence only: do not copy it when the image disagrees. Use it to reconsider difficult letter shapes and short words, while treating the supplied page images as authoritative. Return a complete transcription of the images, not a correction commentary.\n\n--- OCR DRAFT ---\n{}\n\n--- OCR WORDS BELOW {:.0}% CONFIDENCE ---\n{}",
            self.prompt,
            google.text,
            self.google.low_confidence_threshold * 100.0,
            low_confidence_words
        );
        let openai = OpenAiVisionPipeline {
            model: self.model.clone(),
            prompt: combined_prompt,
            output_mode: OutputMode::Plain,
            include_images: true,
        }
        .transcribe(case)
        .await?;

        Ok(PipelineOutput {
            text: openai.text,
            latency_ms: google.latency_ms + openai.latency_ms,
            input_tokens: openai.input_tokens,
            cached_input_tokens: openai.cached_input_tokens,
            output_tokens: openai.output_tokens,
            estimated_cost_usd: match (google.estimated_cost_usd, openai.estimated_cost_usd) {
                (Some(google), Some(openai)) => Some(google + openai),
                _ => None,
            },
            uncertainties: google.uncertainties,
            ocr_words: google.ocr_words,
            disagreements: Vec::new(),
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");
    let dataset_path = option_value(&args, "--dataset")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DATASET));

    match command {
        "validate" => {
            let dataset = load_dataset(&dataset_path)?;
            let cases = validate_dataset(&dataset_path, &dataset)?;
            println!(
                "Dataset '{}' is valid ({} cases).",
                dataset.name,
                cases.len()
            );
        }
        "run" => {
            let pipeline_name = option_value(&args, "--pipeline")
                .ok_or_else(|| anyhow!("run requires --pipeline"))?;
            let dataset = load_dataset(&dataset_path)?;
            let cases = validate_dataset(&dataset_path, &dataset)?;
            if cases.is_empty() {
                bail!("dataset contains no cases; add images and ground-truth entries first");
            }
            let pipeline: Box<dyn TranscriptionPipeline> = match pipeline_name.as_str() {
                "openai_vision" => {
                    let model = option_value(&args, "--model")
                        .or_else(|| env::var("HANDWRITING_MODEL").ok())
                        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
                    let prompt_path = option_value(&args, "--prompt")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from(DEFAULT_PROMPT));
                    let prompt = fs::read_to_string(&prompt_path)
                        .with_context(|| format!("cannot read prompt {}", prompt_path.display()))?;
                    let output_mode = OutputMode::from_cli(option_value(&args, "--output-mode"))?;
                    Box::new(OpenAiVisionPipeline {
                        model,
                        prompt,
                        output_mode,
                        include_images: true,
                    })
                }
                "directory" => {
                    let directory = option_value(&args, "--predictions")
                        .map(PathBuf::from)
                        .ok_or_else(|| anyhow!("directory pipeline requires --predictions"))?;
                    Box::new(DirectoryPipeline { directory })
                }
                "google_document_ai" => {
                    Box::new(google_pipeline_from_args(&args)?)
                }
                "google_then_openai" => {
                    let google = google_pipeline_from_args(&args)?;
                    let model = option_value(&args, "--model")
                        .or_else(|| env::var("HANDWRITING_MODEL").ok())
                        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
                    let prompt_path = option_value(&args, "--prompt")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from(DEFAULT_PROMPT));
                    let prompt = fs::read_to_string(&prompt_path)
                        .with_context(|| format!("cannot read prompt {}", prompt_path.display()))?;
                    Box::new(GoogleThenOpenAiPipeline {
                        google,
                        model,
                        prompt,
                    })
                }
                "parallel_consensus" => {
                    let google = google_pipeline_from_args(&args)?;
                    let model = option_value(&args, "--model")
                        .or_else(|| env::var("HANDWRITING_MODEL").ok())
                        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
                    let prompt_path = option_value(&args, "--prompt")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from(DEFAULT_PROMPT));
                    let prompt = fs::read_to_string(&prompt_path)
                        .with_context(|| format!("cannot read prompt {}", prompt_path.display()))?;
                    Box::new(ParallelConsensusPipeline {
                        google,
                        model,
                        prompt,
                    })
                }
                "candidate_adjudication" => {
                    let google = google_pipeline_from_args(&args)?;
                    let base_model = option_value(&args, "--base-model")
                        .unwrap_or_else(|| "gpt-4.1-mini-2025-04-14".to_string());
                    let judge_model = option_value(&args, "--model")
                        .unwrap_or_else(|| "gpt-5.6".to_string());
                    let prompt_path = option_value(&args, "--prompt")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from(DEFAULT_PROMPT));
                    let prompt = fs::read_to_string(&prompt_path)
                        .with_context(|| format!("cannot read prompt {}", prompt_path.display()))?;
                    Box::new(CandidateAdjudicationPipeline {
                        google,
                        base_model,
                        judge_model,
                        prompt,
                    })
                }
                other => bail!(
                    "unknown pipeline '{other}' (use openai_vision, google_document_ai, google_then_openai, parallel_consensus, candidate_adjudication, or directory)"
                ),
            };
            let results_root = option_value(&args, "--output")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_RESULTS));
            let report_path = run_benchmark(
                &dataset_path,
                &dataset,
                &cases,
                pipeline.as_ref(),
                &results_root,
            )
            .await?;
            println!("Benchmark written to {}", report_path.display());
        }
        _ => print_help(),
    }
    Ok(())
}

fn google_pipeline_from_args(args: &[String]) -> Result<GoogleDocumentAiPipeline> {
    let project = option_value(args, "--google-project")
        .or_else(|| env::var("GOOGLE_CLOUD_PROJECT").ok())
        .ok_or_else(|| anyhow!("provide --google-project"))?;
    let location = option_value(args, "--google-location")
        .or_else(|| env::var("GOOGLE_DOCUMENT_AI_LOCATION").ok())
        .unwrap_or_else(|| "eu".to_string());
    let processor_id = option_value(args, "--google-processor-id")
        .or_else(|| env::var("GOOGLE_DOCUMENT_AI_PROCESSOR_ID").ok())
        .ok_or_else(|| anyhow!("provide --google-processor-id"))?;
    let low_confidence_threshold = option_value(args, "--confidence-threshold")
        .map(|value| value.parse::<f64>())
        .transpose()
        .context("--confidence-threshold must be a number")?
        .unwrap_or(0.8);
    Ok(GoogleDocumentAiPipeline {
        project,
        location,
        processor_id,
        low_confidence_threshold,
    })
}

fn sum_optional_costs(costs: &[Option<f64>]) -> Option<f64> {
    let known: Vec<f64> = costs.iter().flatten().copied().collect();
    (known.len() == costs.len()).then(|| known.iter().sum())
}

async fn run_benchmark(
    dataset_path: &Path,
    dataset: &Dataset,
    cases: &[ResolvedCase],
    pipeline: &dyn TranscriptionPipeline,
    results_root: &Path,
) -> Result<PathBuf> {
    let run_id = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let safe_pipeline_name = sanitize_filename(&pipeline.name());
    let run_dir = results_root.join(format!("{run_id}-{safe_pipeline_name}"));
    let predictions_dir = run_dir.join("predictions");
    fs::create_dir_all(&predictions_dir)?;

    let manifest_dir = dataset_path.parent().unwrap_or_else(|| Path::new("."));
    let mut reports = Vec::with_capacity(cases.len());
    for (case, definition) in cases.iter().zip(&dataset.cases) {
        let expected_path = manifest_dir.join(&definition.expected_text);
        let expected = fs::read_to_string(&expected_path)?;
        let result = pipeline.transcribe(case).await;
        let report = match result {
            Ok(output) => {
                let prediction_path = predictions_dir.join(format!("{}.txt", case.id));
                fs::write(&prediction_path, &output.text)?;
                let metrics =
                    (!expected.trim().is_empty()).then(|| compare_text(&expected, &output.text));
                CaseReport {
                    id: case.id.clone(),
                    image_count: case.images.len(),
                    language: definition.language.clone(),
                    notes: definition.notes.clone(),
                    prediction_path: Some(prediction_path),
                    ground_truth_available: metrics.is_some(),
                    character_error_rate: metrics
                        .as_ref()
                        .map(|metrics| metrics.character_error_rate),
                    word_error_rate: metrics.as_ref().map(|metrics| metrics.word_error_rate),
                    exact_match_after_normalization: metrics
                        .as_ref()
                        .map(|metrics| metrics.exact_match),
                    latency_ms: output.latency_ms,
                    input_tokens: output.input_tokens,
                    cached_input_tokens: output.cached_input_tokens,
                    output_tokens: output.output_tokens,
                    estimated_cost_usd: output.estimated_cost_usd,
                    uncertainties: output.uncertainties,
                    ocr_words: output.ocr_words,
                    disagreements: output.disagreements,
                    error: None,
                }
            }
            Err(error) => CaseReport {
                id: case.id.clone(),
                image_count: case.images.len(),
                language: definition.language.clone(),
                notes: definition.notes.clone(),
                prediction_path: None,
                ground_truth_available: !expected.trim().is_empty(),
                character_error_rate: None,
                word_error_rate: None,
                exact_match_after_normalization: None,
                latency_ms: 0,
                input_tokens: 0,
                cached_input_tokens: 0,
                output_tokens: 0,
                estimated_cost_usd: None,
                uncertainties: Vec::new(),
                ocr_words: Vec::new(),
                disagreements: Vec::new(),
                error: Some(format!("{error:#}")),
            },
        };
        println!(
            "{}: CER={} WER={}{}",
            report.id,
            format_rate(report.character_error_rate),
            format_rate(report.word_error_rate),
            report
                .error
                .as_ref()
                .map(|error| format!(" ERROR={error}"))
                .unwrap_or_default()
        );
        reports.push(report);
    }

    let successful: Vec<&CaseReport> = reports
        .iter()
        .filter(|report| report.error.is_none())
        .collect();
    let aggregate = AggregateMetrics {
        case_count: reports.len(),
        successful_case_count: successful.len(),
        mean_character_error_rate: mean(
            successful
                .iter()
                .filter_map(|report| report.character_error_rate),
        ),
        mean_word_error_rate: mean(
            successful
                .iter()
                .filter_map(|report| report.word_error_rate),
        ),
        total_latency_ms: reports.iter().map(|report| report.latency_ms).sum(),
        total_input_tokens: reports.iter().map(|report| report.input_tokens).sum(),
        total_cached_input_tokens: reports
            .iter()
            .map(|report| report.cached_input_tokens)
            .sum(),
        total_output_tokens: reports.iter().map(|report| report.output_tokens).sum(),
        estimated_cost_usd: sum_known_costs(&reports),
    };
    let report = BenchmarkReport {
        dataset: dataset.name.clone(),
        pipeline: pipeline.name(),
        generated_at_unix_seconds: run_id,
        aggregate,
        cases: reports,
    };
    let report_path = run_dir.join("report.json");
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
    Ok(report_path)
}

fn load_dataset(path: &Path) -> Result<Dataset> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("cannot read dataset manifest {}", path.display()))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("invalid dataset manifest {}", path.display()))
}

fn validate_dataset(path: &Path, dataset: &Dataset) -> Result<Vec<ResolvedCase>> {
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let mut ids = std::collections::HashSet::new();
    let mut resolved = Vec::with_capacity(dataset.cases.len());
    for case in &dataset.cases {
        if case.id.trim().is_empty() || !ids.insert(case.id.clone()) {
            bail!(
                "case IDs must be non-empty and unique (invalid '{}')",
                case.id
            );
        }
        if case.images.is_empty() {
            bail!("case '{}' has no images", case.id);
        }
        let images: Vec<PathBuf> = case.images.iter().map(|image| root.join(image)).collect();
        for image in &images {
            if !image.is_file() {
                bail!("case '{}' image is missing: {}", case.id, image.display());
            }
        }
        let expected = root.join(&case.expected_text);
        if !expected.is_file() {
            bail!(
                "case '{}' ground truth is missing: {}",
                case.id,
                expected.display()
            );
        }
        resolved.push(ResolvedCase {
            id: case.id.clone(),
            images,
        });
    }
    Ok(resolved)
}

struct TextMetrics {
    character_error_rate: f64,
    word_error_rate: f64,
    exact_match: bool,
}

fn compare_text(expected: &str, actual: &str) -> TextMetrics {
    let expected = normalize(expected);
    let actual = normalize(actual);
    let expected_chars: Vec<char> = expected.chars().collect();
    let actual_chars: Vec<char> = actual.chars().collect();
    let expected_words: Vec<&str> = expected.split_whitespace().collect();
    let actual_words: Vec<&str> = actual.split_whitespace().collect();
    TextMetrics {
        character_error_rate: error_rate(&expected_chars, &actual_chars),
        word_error_rate: error_rate(&expected_words, &actual_words),
        exact_match: expected == actual,
    }
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn error_rate<T: Eq>(expected: &[T], actual: &[T]) -> f64 {
    if expected.is_empty() {
        return if actual.is_empty() { 0.0 } else { 1.0 };
    }
    levenshtein(expected, actual) as f64 / expected.len() as f64
}

fn levenshtein<T: Eq>(left: &[T], right: &[T]) -> usize {
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (left_index, left_item) in left.iter().enumerate() {
        let mut current = vec![left_index + 1; right.len() + 1];
        for (right_index, right_item) in right.iter().enumerate() {
            let substitution = previous[right_index] + usize::from(left_item != right_item);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(substitution);
        }
        previous = current;
    }
    previous[right.len()]
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        output.push(TABLE[((value >> 18) & 63) as usize] as char);
        output.push(TABLE[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

fn extract_output_text(response: &serde_json::Value) -> Option<String> {
    response
        .get("output")?
        .as_array()?
        .iter()
        .flat_map(|item| item.get("content").and_then(|value| value.as_array()))
        .flatten()
        .find(|content| content.get("type").and_then(|value| value.as_str()) == Some("output_text"))
        .and_then(|content| content.get("text"))
        .and_then(|text| text.as_str())
        .map(ToOwned::to_owned)
}

fn google_access_token() -> Result<String> {
    let output = Command::new("gcloud")
        .args(["auth", "print-access-token", "--quiet"])
        .output()
        .context("failed to run gcloud; install and authenticate the Google Cloud CLI")?;
    if !output.status.success() {
        bail!(
            "gcloud could not provide an access token: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let token = String::from_utf8(output.stdout)?.trim().to_string();
    if token.is_empty() {
        bail!("gcloud returned an empty access token");
    }
    Ok(token)
}

fn extract_google_words(
    document: &serde_json::Value,
    document_text: &str,
    page_number: u64,
    output: &mut Vec<OcrWord>,
) {
    let Some(pages) = document.get("pages").and_then(|pages| pages.as_array()) else {
        return;
    };
    for token in pages
        .iter()
        .filter_map(|page| page.get("tokens").and_then(|tokens| tokens.as_array()))
        .flatten()
    {
        let Some(layout) = token.get("layout") else {
            continue;
        };
        let text = google_anchor_text(document_text, layout.get("textAnchor"));
        if text.trim().is_empty() {
            continue;
        }
        let confidence = layout
            .get("confidence")
            .and_then(|confidence| confidence.as_f64())
            .unwrap_or(0.0);
        let polygon = layout
            .get("boundingPoly")
            .and_then(|polygon| polygon.get("normalizedVertices"))
            .and_then(|vertices| vertices.as_array())
            .map(|vertices| {
                vertices
                    .iter()
                    .map(|vertex| OcrPoint {
                        x: vertex.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0),
                        y: vertex.get("y").and_then(|y| y.as_f64()).unwrap_or(0.0),
                    })
                    .collect()
            })
            .unwrap_or_default();
        output.push(OcrWord {
            page_number,
            text: text.trim().to_string(),
            confidence,
            polygon,
        });
    }
}

fn google_anchor_text(text: &str, anchor: Option<&serde_json::Value>) -> String {
    let Some(segments) = anchor
        .and_then(|anchor| anchor.get("textSegments"))
        .and_then(|segments| segments.as_array())
    else {
        return String::new();
    };
    segments
        .iter()
        .filter_map(|segment| {
            let start = json_u64(segment.get("startIndex")).unwrap_or(0) as usize;
            let end = json_u64(segment.get("endIndex"))? as usize;
            if let Some(slice) = text.get(start..end) {
                return Some(slice.to_string());
            }
            Some(
                text.chars()
                    .skip(start)
                    .take(end.saturating_sub(start))
                    .collect(),
            )
        })
        .collect()
}

fn json_u64(value: Option<&serde_json::Value>) -> Option<u64> {
    value.and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
    })
}

fn confidence_output_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["transcription", "uncertainties"],
        "properties": {
            "transcription": { "type": "string" },
            "uncertainties": {
                "type": "array",
                "maxItems": 12,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "page_number",
                        "transcribed_text",
                        "surrounding_context",
                        "confidence",
                        "alternatives",
                        "reason"
                    ],
                    "properties": {
                        "page_number": { "type": "integer", "minimum": 1 },
                        "transcribed_text": { "type": "string", "maxLength": 120 },
                        "surrounding_context": { "type": "string", "maxLength": 240 },
                        "confidence": { "type": "string", "enum": ["low", "medium"] },
                        "alternatives": {
                            "type": "array",
                            "maxItems": 3,
                            "items": { "type": "string" }
                        },
                        "reason": { "type": "string", "maxLength": 240 }
                    }
                }
            }
        }
    })
}

fn token_count(usage: Option<&serde_json::Value>, field: &str) -> u64 {
    usage
        .and_then(|usage| usage.get(field))
        .and_then(|tokens| tokens.as_u64())
        .unwrap_or(0)
}

struct TokenPrices {
    input_per_million: f64,
    cached_input_per_million: f64,
    output_per_million: f64,
    long_context_multiplier: bool,
}

fn token_prices(model: &str) -> Option<TokenPrices> {
    if model == "gpt-5.6" || model == "gpt-5.6-sol" || model.starts_with("gpt-5.6-sol-") {
        return Some(TokenPrices {
            input_per_million: 4.0,
            cached_input_per_million: 0.4,
            output_per_million: 20.0,
            long_context_multiplier: true,
        });
    }
    if model == "gpt-5.6-terra" || model.starts_with("gpt-5.6-terra-") {
        return Some(TokenPrices {
            input_per_million: 2.0,
            cached_input_per_million: 0.2,
            output_per_million: 12.0,
            long_context_multiplier: true,
        });
    }
    if model == "gpt-5.6-luna" || model.starts_with("gpt-5.6-luna-") {
        return Some(TokenPrices {
            input_per_million: 0.2,
            cached_input_per_million: 0.02,
            output_per_million: 1.2,
            long_context_multiplier: true,
        });
    }
    if model == "gpt-4.1-mini" || model.starts_with("gpt-4.1-mini-") {
        return Some(TokenPrices {
            input_per_million: 0.4,
            cached_input_per_million: 0.1,
            output_per_million: 1.6,
            long_context_multiplier: false,
        });
    }
    None
}

fn estimate_cost_usd(
    model: &str,
    input_tokens: u64,
    cached_input_tokens: u64,
    output_tokens: u64,
) -> Option<f64> {
    let prices = token_prices(model)?;
    let cached = cached_input_tokens.min(input_tokens);
    let uncached = input_tokens - cached;
    let is_long_context = prices.long_context_multiplier && input_tokens > 272_000;
    let input_multiplier = if is_long_context { 2.0 } else { 1.0 };
    let output_multiplier = if is_long_context { 1.5 } else { 1.0 };
    Some(
        ((uncached as f64 * prices.input_per_million
            + cached as f64 * prices.cached_input_per_million)
            * input_multiplier
            + output_tokens as f64 * prices.output_per_million * output_multiplier)
            / 1_000_000.0,
    )
}

fn sum_known_costs(reports: &[CaseReport]) -> Option<f64> {
    let costs: Vec<f64> = reports
        .iter()
        .filter_map(|report| report.estimated_cost_usd)
        .collect();
    (!costs.is_empty()).then(|| costs.iter().sum())
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|argument| argument == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let values: Vec<f64> = values.collect();
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn format_rate(rate: Option<f64>) -> String {
    rate.map(|value| format!("{:.2}%", value * 100.0))
        .unwrap_or_else(|| "n/a".to_string())
}

fn print_help() {
    println!(
        "Handwriting transcription playground\n\n\
         Validate: cargo run --bin handwriting_playground -- validate [--dataset PATH]\n\
         OpenAI:  cargo run --bin handwriting_playground -- run --pipeline openai_vision [--model MODEL] [--output-mode plain|confidence] [--prompt PATH] [--dataset PATH] [--output DIR]\n\
         Google:  cargo run --bin handwriting_playground -- run --pipeline google_document_ai --google-project PROJECT --google-processor-id ID [--google-location eu] [--confidence-threshold 0.8]\n\
         Hybrid:  cargo run --bin handwriting_playground -- run --pipeline google_then_openai --model MODEL --google-project PROJECT --google-processor-id ID [--confidence-threshold 0.8]\n\
         Compare: cargo run --bin handwriting_playground -- run --pipeline parallel_consensus --model MODEL --google-project PROJECT --google-processor-id ID [--confidence-threshold 0.8]\n\
         Judge:   cargo run --bin handwriting_playground -- run --pipeline candidate_adjudication --base-model gpt-4.1-mini-2025-04-14 --model gpt-5.6 --google-project PROJECT --google-processor-id ID [--confidence-threshold 0.8]\n\
         External: cargo run --bin handwriting_playground -- run --pipeline directory --predictions DIR [--dataset PATH] [--output DIR]"
    );
}

#[cfg(test)]
mod tests {
    use super::{compare_text, encode_base64, estimate_cost_usd, levenshtein};

    #[test]
    fn computes_edit_distance() {
        assert_eq!(levenshtein(&['a', 'b', 'c'], &['a', 'x', 'c']), 1);
        assert_eq!(levenshtein(&["hello", "world"], &["hello"]), 1);
    }

    #[test]
    fn normalizes_spacing_and_case_for_scores() {
        let metrics = compare_text("Bonjour  le\nmonde", "bonjour le monde");
        assert!(metrics.exact_match);
        assert_eq!(metrics.character_error_rate, 0.0);
        assert_eq!(metrics.word_error_rate, 0.0);
    }

    #[test]
    fn encodes_base64_with_padding() {
        assert_eq!(encode_base64(b"f"), "Zg==");
        assert_eq!(encode_base64(b"fo"), "Zm8=");
        assert_eq!(encode_base64(b"foo"), "Zm9v");
    }

    #[test]
    fn estimates_gpt_5_6_sol_cost() {
        let cost = estimate_cost_usd("gpt-5.6", 5_000, 1_000, 500).unwrap();
        assert!((cost - 0.0264).abs() < 1e-12);
    }
}
