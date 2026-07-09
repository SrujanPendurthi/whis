//! Local embedding pipeline for `bge-small-en-v1.5` via candle.
//!
//! Loads the model + tokenizer from the HuggingFace hub on first run (cached
//! thereafter), then produces 384-d sentence embeddings using masked mean
//! pooling followed by L2 normalization — the pooling bge/sentence-transformers
//! models expect.
//!
//! Device selection is compile-time gated: the CPU backend is the default so
//! the crate builds under Command Line Tools; `--features metal` runs the
//! forward pass on the Apple GPU (needs the full Xcode toolchain to build
//! candle's Metal kernels).

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

use crate::shared::config::{EMBEDDING_DIM, EMBEDDING_MODEL_ID};
use crate::shared::error::{AppError, AppResult};

/// Max sequence length; bge-small truncates beyond this.
const MAX_LEN: usize = 512;

/// Loaded, ready-to-run embedding model.
pub struct Embedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl Embedder {
    /// Download (if needed) and load the model + tokenizer. Async because the
    /// weight fetch may hit the network on first run.
    pub async fn load() -> AppResult<Self> {
        let (config_path, tokenizer_path, weights_path) = fetch_model_files().await?;

        let device = select_device()?;

        let config: Config = {
            let raw = std::fs::read_to_string(&config_path)?;
            serde_json::from_str(&raw)?
        };

        let mut tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| AppError::VectorStore(format!("tokenizer load failed: {e}")))?;
        configure_tokenizer(&mut tokenizer)?;

        // SAFETY: mmap of a trusted, locally-cached weights file.
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device)
                .map_err(candle_err)?
        };
        let model = BertModel::load(vb, &config).map_err(candle_err)?;

        Ok(Self { model, tokenizer, device })
    }

    /// Embed a single string.
    pub fn embed_one(&self, text: &str) -> AppResult<Vec<f32>> {
        let mut out = self.embed(&[text.to_string()])?;
        out.pop()
            .ok_or_else(|| AppError::VectorStore("embedding produced no output".into()))
    }

    /// Embed a batch of strings, returning one 384-d vector per input.
    pub fn embed(&self, texts: &[String]) -> AppResult<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| AppError::VectorStore(format!("tokenize failed: {e}")))?;

        let (batch, seq) = (encodings.len(), encodings[0].len());
        let mut ids = Vec::with_capacity(batch * seq);
        let mut mask = Vec::with_capacity(batch * seq);
        for enc in &encodings {
            ids.extend(enc.get_ids().iter().map(|&i| i as i64));
            mask.extend(enc.get_attention_mask().iter().map(|&m| m as i64));
        }

        let input_ids = Tensor::from_vec(ids, (batch, seq), &self.device).map_err(candle_err)?;
        let attention_mask =
            Tensor::from_vec(mask, (batch, seq), &self.device).map_err(candle_err)?;
        let token_type_ids = input_ids.zeros_like().map_err(candle_err)?;

        // [batch, seq, hidden]
        let hidden = self
            .model
            .forward(&input_ids, &token_type_ids, Some(&attention_mask))
            .map_err(candle_err)?;

        let pooled = mean_pool(&hidden, &attention_mask)?;
        let normalized = l2_normalize(&pooled)?;

        let vectors = normalized.to_vec2::<f32>().map_err(candle_err)?;
        debug_assert!(vectors.iter().all(|v| v.len() == EMBEDDING_DIM));
        Ok(vectors)
    }
}

/// Masked mean pooling over the sequence dimension.
/// `hidden`: [b, s, h], `attention_mask`: [b, s] -> [b, h].
fn mean_pool(hidden: &Tensor, attention_mask: &Tensor) -> AppResult<Tensor> {
    // [b, s] -> [b, s, 1] float, broadcastable over hidden dim.
    let mask = attention_mask
        .to_dtype(DType::F32)
        .map_err(candle_err)?
        .unsqueeze(2)
        .map_err(candle_err)?;
    let masked = hidden.broadcast_mul(&mask).map_err(candle_err)?;
    let summed = masked.sum(1).map_err(candle_err)?; // [b, h]
    let counts = mask.sum(1).map_err(candle_err)?; // [b, 1]
    // Guard against division by zero for all-padding rows.
    let counts = counts.clamp(1e-9, f32::INFINITY).map_err(candle_err)?;
    summed.broadcast_div(&counts).map_err(candle_err)
}

/// L2-normalize each row of a [b, h] tensor.
fn l2_normalize(t: &Tensor) -> AppResult<Tensor> {
    let norm = t
        .sqr()
        .map_err(candle_err)?
        .sum_keepdim(1)
        .map_err(candle_err)?
        .sqrt()
        .map_err(candle_err)?
        .clamp(1e-12, f32::INFINITY)
        .map_err(candle_err)?;
    t.broadcast_div(&norm).map_err(candle_err)
}

/// Configure padding (to the longest in a batch) and truncation to `MAX_LEN`.
fn configure_tokenizer(tokenizer: &mut Tokenizer) -> AppResult<()> {
    tokenizer.with_padding(Some(PaddingParams {
        strategy: PaddingStrategy::BatchLongest,
        ..Default::default()
    }));
    tokenizer
        .with_truncation(Some(TruncationParams { max_length: MAX_LEN, ..Default::default() }))
        .map_err(|e| AppError::VectorStore(format!("tokenizer truncation config failed: {e}")))?;
    Ok(())
}

/// Resolve config.json / tokenizer.json / model.safetensors, downloading from
/// the hub on first run.
async fn fetch_model_files(
) -> AppResult<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf)> {
    use hf_hub::api::tokio::Api;

    let api = Api::new().map_err(|e| AppError::VectorStore(format!("hf-hub init: {e}")))?;
    let repo = api.model(EMBEDDING_MODEL_ID.to_string());

    let config = repo
        .get("config.json")
        .await
        .map_err(|e| AppError::VectorStore(format!("fetch config.json: {e}")))?;
    let tokenizer = repo
        .get("tokenizer.json")
        .await
        .map_err(|e| AppError::VectorStore(format!("fetch tokenizer.json: {e}")))?;
    let weights = repo
        .get("model.safetensors")
        .await
        .map_err(|e| AppError::VectorStore(format!("fetch model.safetensors: {e}")))?;
    Ok((config, tokenizer, weights))
}

#[cfg(feature = "metal")]
fn select_device() -> AppResult<Device> {
    Device::new_metal(0).map_err(candle_err)
}

#[cfg(not(feature = "metal"))]
fn select_device() -> AppResult<Device> {
    Ok(Device::Cpu)
}

/// Map a candle error into the app error type.
fn candle_err(e: candle_core::Error) -> AppError {
    AppError::VectorStore(format!("candle: {e}"))
}
