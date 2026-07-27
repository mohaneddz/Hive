//! Image captioning using a lightweight vision-to-text model.
//!
//! ViT encoder + GPT-2 decoder (Xenova/vit-gpt2-image-captioning, ONNX). The
//! encoder turns the picture into visual features once; the decoder then writes
//! the sentence one token at a time, each token chosen from everything generated
//! so far.
//!
//! Pipeline: resize 224×224 → normalise → encoder → greedy decode → text
//!
//! The decoder ships in two shapes and this module handles both. The plain export
//! takes `input_ids` + `encoder_hidden_states` and recomputes the whole prefix at
//! every step. The *merged* export additionally declares `use_cache_branch` and
//! 24 `past_key_values.*` inputs, and ONNX Runtime refuses to run it unless every
//! one of them is supplied — which is what used to happen here: the download
//! fetched the merged file, the run was given two inputs, and every single image
//! failed. The job still walked to 100%, because the error was swallowed, and
//! wrote nothing.
//!
//! The cache is supplied but deliberately never reused: this export declares no
//! `position_ids` input and derives each token's position from the length of
//! `input_ids` alone. Feeding one token at a time would therefore place every
//! word at position 0, and the model loops — "a close up of a close up of a
//! close up". Handing it the whole sentence each step costs a few extra passes
//! and is the only way it stays coherent.

use std::collections::HashMap;
use std::path::Path;

use ort::memory::Allocator;
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::{DynValue, Tensor};

/// Must match the model's own `preprocessor_config.json`, not ImageNet defaults.
/// ViT-GPT2 was trained with mean 0.5 / std 0.5; feeding it ImageNet statistics
/// does not fail, it just quietly produces captions about the wrong picture.
const IMAGE_SIZE: u32 = 224;
const PIXEL_MEAN: [f32; 3] = [0.5, 0.5, 0.5];
const PIXEL_STD: [f32; 3] = [0.5, 0.5, 0.5];

/// `generation_config.json` gives this same id as `bos_token_id`, `eos_token_id`
/// and `pad_token_id`, so it both starts and ends the sentence.
const EOS_TOKEN_ID: i64 = 50256;

/// Captions are one short sentence; the reference implementation stops at 16.
const MAX_LENGTH: usize = 20;

/// Greedy decoding falls into loops — "a man riding a man riding a man". Refusing
/// any token that would repeat a three-token run is the same guard the reference
/// implementation applies as `no_repeat_ngram_size`.
const NO_REPEAT_NGRAM: usize = 3;

/// The decoder half is GPT-2 small: 12 heads of 64 features. This only shapes the
/// empty cache the merged export insists on being handed.
const NUM_HEADS: usize = 12;
const HEAD_DIM: usize = 64;

const PAST_PREFIX: &str = "past_key_values.";

fn ort_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

/// GPT-2 keeps its vocabulary readable by remapping every raw byte to a printable
/// character. Undoing that mapping is what turns "Ġa" back into " a" and
/// reassembles accented letters, which arrive as two separate symbols.
fn byte_decoder() -> HashMap<char, u8> {
    let printable: Vec<u32> = (0x21..=0x7E).chain(0xA1..=0xAC).chain(0xAE..=0xFF).collect();
    let mut table = HashMap::with_capacity(256);
    let mut next_spare = 0u32;
    for byte in 0..=255u32 {
        // Bytes that are already printable stand for themselves; the rest borrow
        // an unused symbol above 255, handed out in ascending byte order.
        let symbol = if printable.contains(&byte) {
            byte
        } else {
            next_spare += 1;
            255 + next_spare
        };
        if let Some(symbol) = char::from_u32(symbol) {
            table.insert(symbol, byte as u8);
        }
    }
    table
}

/// The tokens that would repeat an n-gram this sentence already contains.
fn banned_next(generated: &[i64], n: usize) -> Vec<i64> {
    if generated.len() < n {
        return Vec::new();
    }
    let prefix = &generated[generated.len() + 1 - n..];
    generated
        .windows(n)
        .filter(|window| &window[..n - 1] == prefix)
        .map(|window| window[n - 1])
        .collect()
}

/// An empty per-layer cache, which is all this decoder is ever given.
///
/// `from_array` rejects a zero-length dimension — it describes data that exists —
/// so the tensor has to be asked of the allocator instead.
fn empty_cache_tensor() -> anyhow::Result<Tensor<f32>> {
    Tensor::new(&Allocator::default(), vec![1usize, NUM_HEADS, 0, HEAD_DIM]).map_err(ort_err)
}

pub struct CaptionModel {
    encoder: Session,
    decoder: Session,
    /// Token ID → token text, loaded from vocab.json.
    vocab: Vec<String>,
    byte_decoder: HashMap<char, u8>,
    /// The decoder's `past_key_values.*` inputs, empty for a plain export.
    past_inputs: Vec<String>,
    /// Whether the decoder wants to be told which branch to take.
    wants_cache_flag: bool,
}

impl CaptionModel {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let session = |file: &str| -> anyhow::Result<Session> {
            Session::builder()
                .map_err(ort_err)?
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(ort_err)?
                .with_intra_threads(2)
                .map_err(ort_err)?
                .commit_from_file(dir.join(file))
                .map_err(ort_err)
        };
        let encoder = session("encoder.onnx")?;
        let decoder = session("decoder.onnx")?;

        // Ask the model what it needs rather than assuming, so either export runs.
        let past_inputs: Vec<String> = decoder
            .inputs()
            .iter()
            .map(|input| input.name().to_string())
            .filter(|name| name.starts_with(PAST_PREFIX))
            .collect();
        let wants_cache_flag = decoder
            .inputs()
            .iter()
            .any(|input| input.name() == "use_cache_branch");

        // vocab.json maps token text → id; invert it to read generated ids back.
        let vocab_path = dir.join("vocab.json");
        let vocab = if vocab_path.is_file() {
            let raw = std::fs::read_to_string(&vocab_path)?;
            let map: HashMap<String, i64> = serde_json::from_str(&raw)?;
            let max_id = map.values().copied().max().unwrap_or(0) as usize;
            let mut table = vec![String::new(); max_id + 1];
            for (token, id) in map {
                table[id as usize] = token;
            }
            table
        } else {
            Vec::new()
        };

        Ok(Self {
            encoder,
            decoder,
            vocab,
            byte_decoder: byte_decoder(),
            past_inputs,
            wants_cache_flag,
        })
    }

    /// Generates a text caption for the given image.
    pub fn caption(&mut self, path: &Path) -> anyhow::Result<String> {
        let ids = self.generate(path)?;
        Ok(self.decode(&ids))
    }

    /// Turns the image into the encoder's visual features, plus their shape.
    fn encode(&mut self, path: &Path) -> anyhow::Result<(Vec<usize>, Vec<f32>)> {
        let img = image::open(path)?
            .resize_exact(IMAGE_SIZE, IMAGE_SIZE, image::imageops::FilterType::Triangle)
            .to_rgb8();

        let (w, h) = (IMAGE_SIZE as usize, IMAGE_SIZE as usize);
        let mut pixels = vec![0.0f32; 3 * w * h];
        for (x, y, pixel) in img.enumerate_pixels() {
            let (x, y) = (x as usize, y as usize);
            for c in 0..3 {
                let value = pixel[c] as f32 / 255.0;
                pixels[c * w * h + y * w + x] = (value - PIXEL_MEAN[c]) / PIXEL_STD[c];
            }
        }

        let tensor = Tensor::from_array(([1usize, 3, h, w], pixels)).map_err(ort_err)?;
        let output = self.encoder.run(ort::inputs![tensor]).map_err(ort_err)?;

        let shape: Vec<usize> = output[0].shape().iter().map(|&d| d as usize).collect();
        let data: Vec<f32> = output[0]
            .try_extract_array::<f32>()
            .map_err(ort_err)?
            .iter()
            .copied()
            .collect();
        Ok((shape, data))
    }

    /// Greedy decoding: at every step the sentence so far goes in, and the single
    /// most likely next word comes out, until the model says it is done.
    fn generate(&mut self, path: &Path) -> anyhow::Result<Vec<i64>> {
        let (encoder_shape, encoder_data) = self.encode(path)?;

        // Cloned once so the loop can borrow `self.decoder` mutably.
        let past_inputs = self.past_inputs.clone();
        let wants_cache_flag = self.wants_cache_flag;

        let mut generated = vec![EOS_TOKEN_ID];

        for _ in 0..MAX_LENGTH {
            let feed_ids = generated.clone();

            let mut feeds: Vec<(String, DynValue)> = Vec::with_capacity(past_inputs.len() + 3);
            feeds.push((
                "input_ids".to_string(),
                Tensor::from_array(([1usize, feed_ids.len()], feed_ids))
                    .map_err(ort_err)?
                    .into_dyn(),
            ));
            feeds.push((
                "encoder_hidden_states".to_string(),
                Tensor::from_array((encoder_shape.clone(), encoder_data.clone()))
                    .map_err(ort_err)?
                    .into_dyn(),
            ));
            // Always the recompute-everything branch, with the empty cache that
            // goes with it. The merged export refuses to run without these, even
            // when it is about to ignore them.
            if wants_cache_flag {
                feeds.push((
                    "use_cache_branch".to_string(),
                    Tensor::from_array(([1usize], vec![false]))
                        .map_err(ort_err)?
                        .into_dyn(),
                ));
            }
            for name in &past_inputs {
                feeds.push((name.clone(), empty_cache_tensor()?.into_dyn()));
            }

            let outputs = self.decoder.run(feeds).map_err(ort_err)?;

            // Shape is [batch, positions, vocab]; the last row predicts what comes
            // next. Reading the width off the model beats inferring it from the
            // buffer length, which silently mislabels every token when it is wrong.
            let vocab_size = *outputs[0]
                .shape()
                .last()
                .ok_or_else(|| anyhow::anyhow!("decoder returned logits with no shape"))?
                as usize;
            let logits: Vec<f32> = outputs[0]
                .try_extract_array::<f32>()
                .map_err(ort_err)?
                .iter()
                .copied()
                .collect();
            let last_logits = &logits[logits.len() - vocab_size..];

            let banned = banned_next(&generated, NO_REPEAT_NGRAM);
            let next_id = last_logits
                .iter()
                .enumerate()
                .filter(|(id, _)| !banned.contains(&(*id as i64)))
                .max_by(|(_, a), (_, b)| a.total_cmp(b))
                .map(|(id, _)| id as i64)
                .unwrap_or(EOS_TOKEN_ID);

            if next_id == EOS_TOKEN_ID {
                break;
            }
            generated.push(next_id);
        }

        Ok(generated)
    }

    /// Token ids → sentence, undoing GPT-2's byte remapping.
    fn decode(&self, ids: &[i64]) -> String {
        if self.vocab.is_empty() {
            return String::new();
        }
        // The first id is the start marker, which stands for no word.
        let symbols: String = ids
            .iter()
            .skip(1)
            .filter_map(|&id| self.vocab.get(id as usize))
            .flat_map(|token| token.chars())
            .collect();
        let bytes: Vec<u8> = symbols
            .chars()
            .filter_map(|symbol| self.byte_decoder.get(&symbol).copied())
            .collect();
        String::from_utf8_lossy(&bytes).trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_byte_table_covers_every_byte_exactly_once() {
        let table = byte_decoder();
        assert_eq!(table.len(), 256);
        let mut bytes: Vec<u8> = table.values().copied().collect();
        bytes.sort_unstable();
        bytes.dedup();
        assert_eq!(bytes.len(), 256);
    }

    #[test]
    fn the_space_marker_decodes_back_to_a_space() {
        let table = byte_decoder();
        // "Ġ" is how GPT-2 writes a leading space; "a" stands for itself.
        assert_eq!(table.get(&'Ġ').copied(), Some(b' '));
        assert_eq!(table.get(&'a').copied(), Some(b'a'));
    }

    #[test]
    fn a_repeated_trigram_bans_the_token_that_would_close_it() {
        // "a man riding a man": proposing "riding" again would close the loop, so
        // it is the one token refused. The ban keys on the two tokens just said.
        let generated = [1, 2, 3, 1, 2];
        assert_eq!(banned_next(&generated, NO_REPEAT_NGRAM), vec![3]);
    }

    #[test]
    fn a_pair_that_never_recurred_bans_nothing() {
        // Ends on "riding a", which has not been followed by anything yet.
        assert!(banned_next(&[1, 2, 3, 1], NO_REPEAT_NGRAM).is_empty());
    }

    #[test]
    fn nothing_is_banned_before_the_first_ngram_exists() {
        assert!(banned_next(&[1, 2], NO_REPEAT_NGRAM).is_empty());
    }

    /// Needs the downloaded model, so it is skipped by default. Run it with
    /// `cargo test --lib captioning -- --include-ignored --nocapture` after
    /// downloading in Settings.
    ///
    /// This is the test that would have caught the original bug: the decoder was
    /// being handed 2 of its 26 inputs, so every run failed and the caller threw
    /// the error away.
    #[test]
    #[ignore]
    fn a_real_image_produces_a_readable_english_caption() {
        let Some(dir) = std::env::var_os("APPDATA").map(|base| {
            Path::new(&base)
                .join("com.hive")
                .join("models")
                .join("caption")
        }) else {
            return;
        };
        if !dir.join("decoder.onnx").is_file() {
            return;
        }

        // A synthetic picture: what it depicts does not matter, only that the
        // pipeline runs end to end and comes back with words.
        let image = image::RgbImage::from_fn(320, 240, |x, y| {
            image::Rgb([(x / 2) as u8, (y / 2) as u8, 128])
        });
        let path = std::env::temp_dir().join("hive_caption_test.png");
        image.save(&path).unwrap();

        let mut model = CaptionModel::load(&dir).expect("model loads");
        let caption = model.caption(&path).expect("captioning runs");
        println!("caption: {caption:?}");

        assert!(!caption.is_empty(), "the model wrote nothing");
        assert!(
            caption.split_whitespace().count() >= 3,
            "expected a sentence, got {caption:?}"
        );
        // Greedy decoding without the n-gram guard loops; a caption where every
        // other word repeats is the symptom.
        let words: Vec<&str> = caption.split_whitespace().collect();
        let distinct: std::collections::HashSet<&&str> = words.iter().collect();
        assert!(
            distinct.len() * 2 > words.len(),
            "caption is looping: {caption:?}"
        );
    }
}
