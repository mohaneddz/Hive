use std::path::Path;

use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_qwen2::ModelWeights;
use tokenizers::Tokenizer;

const MAX_NEW_TOKENS: usize = 400;

/// A small local instruct model (Qwen2.5-1.5B-Instruct, Q4_K_M GGUF) run via `candle` — pure
/// Rust, no C toolchain needed (unlike llama.cpp bindings, which require libclang/bindgen that
/// isn't available in every dev environment). CPU-only; a few seconds per reply on a modern
/// machine for prompts of this size.
pub struct ChatModel {
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
    eos_tokens: Vec<u32>,
}

fn candle_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

impl ChatModel {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let device = Device::Cpu;
        let gguf_path = dir.join("model.gguf");
        let mut file = std::fs::File::open(&gguf_path)?;
        let content = gguf_file::Content::read(&mut file).map_err(candle_err)?;
        let model = ModelWeights::from_gguf(content, &mut file, &device).map_err(candle_err)?;

        let tokenizer = Tokenizer::from_file(dir.join("tokenizer.json"))
            .map_err(|e| anyhow::anyhow!("failed to load chat tokenizer: {e}"))?;

        let eos_tokens = ["<|im_end|>", "<|endoftext|>"]
            .iter()
            .filter_map(|t| tokenizer.token_to_id(t))
            .collect();

        Ok(Self { model, tokenizer, device, eos_tokens })
    }

    /// Runs one turn of a ChatML-formatted conversation and returns the full reply. Blocking —
    /// callers should run this on a background thread.
    pub fn chat(&mut self, system: &str, user: &str) -> anyhow::Result<String> {
        self.model.clear_kv_cache();

        let prompt = format!(
            "<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n"
        );
        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("failed to tokenize prompt: {e}"))?;
        let mut tokens: Vec<u32> = encoding.get_ids().to_vec();
        let prompt_len = tokens.len();

        let mut logits_processor = LogitsProcessor::new(299_792_458, Some(0.6), Some(0.9));
        let mut generated: Vec<u32> = Vec::new();

        for index in 0..MAX_NEW_TOKENS {
            let context_size = if index == 0 { tokens.len() } else { 1 };
            let start_pos = tokens.len() - context_size;
            let ctxt = &tokens[start_pos..];
            let input = Tensor::new(ctxt, &self.device)
                .map_err(candle_err)?
                .unsqueeze(0)
                .map_err(candle_err)?;
            let logits = self.model.forward(&input, start_pos).map_err(candle_err)?;
            let logits = logits.squeeze(0).map_err(candle_err)?;
            let next = logits_processor.sample(&logits).map_err(candle_err)?;

            if self.eos_tokens.contains(&next) {
                break;
            }
            tokens.push(next);
            generated.push(next);
        }

        let _ = prompt_len;
        self.tokenizer
            .decode(&generated, true)
            .map_err(|e| anyhow::anyhow!("failed to decode reply: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ignored by default since it depends on the ~1.1GB GGUF download; run explicitly with
    /// `cargo test -- --ignored ai::llm::tests`.
    #[test]
    #[ignore]
    fn answers_a_simple_question_grounded_in_given_context() {
        let dir = std::path::PathBuf::from(std::env::var("APPDATA").unwrap())
            .join("com.hive")
            .join("models")
            .join("llm");
        let mut model = ChatModel::load(&dir).expect("failed to load chat model");

        let system = "You are a helpful assistant. Answer using only the information given.";
        let user = "The capital of France is Paris. Question: What is the capital of France?";
        let reply = model.chat(system, user).unwrap();

        assert!(!reply.trim().is_empty(), "expected a non-empty reply");
        assert!(
            reply.to_lowercase().contains("paris"),
            "expected the reply to mention Paris, got: {reply:?}"
        );
    }
}
