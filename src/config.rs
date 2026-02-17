use std::collections::HashMap;
use std::sync::LazyLock;

/// Default model name - LFM 2.5 Thinking
#[allow(dead_code)]
pub const DEFAULT_MODEL: &str = "lfm";

static CONFIGS: LazyLock<HashMap<&'static str, ModelConfig>> = LazyLock::new(|| {
    let mut configs = HashMap::new();

    // lfm: LFM 2.5 Thinking model, 1.2B parameters, 32K context (DEFAULT MODEL)
    // Source: ollama.com/library/lfm2.5-thinking
    // Optimized for: Fast reasoning with thinking mode
    configs.insert(
        "lfm",
        ModelConfig {
            model_id: "lfm2.5-thinking:1.2b-32k".to_string(),
            num_ctx: 32768,
            temperature: 0.1, // Low temp for deterministic reasoning
            top_k: 50,
            top_p: 0.1, // Low top_p for focused output
            repeat_penalty: 1.05,
        },
    );

    // gpt-oss: 20B parameters, 64K context
    // Source: hf.co/unsloth/gpt-oss-20b-GGUF:Q4_K_M
    // Tool-optimized: Lower temperature for reliable JSON/tool output
    // Note: GPT-OSS has reasoning effort (low/medium/high) separate from temperature
    configs.insert(
        "gpt-oss",
        ModelConfig {
            model_id: "gpt-oss:20b-64k".to_string(),
            num_ctx: 65535,
            temperature: 0.2, // Lowered for tool reliability (was 1.0)
            top_k: 40,        // Re-enabled for tool precision (was 0)
            top_p: 0.9,       // Narrowed for structured output (was 1.0)
            repeat_penalty: 1.0,
        },
    );

    // mistral-small: 24B parameters, 32K context
    // Source: hf.co/unsloth/Mistral-Small-3.2-24B-Instruct-2506-GGUF:Q4_K_M
    // Optimized for: Agentic tasks with native tool support
    configs.insert(
        "mistral-small",
        ModelConfig {
            model_id: "mistral-small3.2:24b-32k".to_string(),
            num_ctx: 32768,
            temperature: 0.2, // Lower for tool accuracy
            top_k: 40,
            top_p: 0.9,
            repeat_penalty: 1.1,
        },
    );

    // smollm3: 3B parameters quantized Q8_0, 64K context
    // Source: hf.co/ggml-org/SmolLM3-3B-GGUF:Q8_0
    // Optimized for: Edge deployment, efficient inference
    configs.insert(
        "smollm3",
        ModelConfig {
            model_id: "smollm3:Q8_0-64k".to_string(),
            num_ctx: 65536,
            temperature: 0.2,
            top_k: 40,
            top_p: 0.9,
            repeat_penalty: 1.1,
        },
    );

    // sead: 14B parameters, 32K context
    // Source: hf.co/mradermacher/SEAD-14B-GGUF:Q4_K_M
    // Optimized for: General purpose tasks
    configs.insert(
        "sead",
        ModelConfig {
            model_id: "sead:14b-32k".to_string(),
            num_ctx: 32768,
            temperature: 0.2,
            top_k: 40,
            top_p: 0.9,
            repeat_penalty: 1.1,
        },
    );

    // qwen3-coder: 30B parameters (3.3B active), 64K context
    // Source: ollama.com/library/qwen3-coder
    // Tool-optimized: Lower temperature for reliable tool use (0.3 vs 0.7 for pure coding)
    // Note: Qwen team recommends 0.7 for coding, 0.3 for agentic tool workflows
    configs.insert(
        "qwen3-coder",
        ModelConfig {
            model_id: "qwen3-coder:30b-64k".to_string(),
            num_ctx: 65536,
            temperature: 0.3, // Lowered for tool reliability (was 0.7)
            top_k: 20,        // Good for precision
            top_p: 0.80,
            repeat_penalty: 1.05,
        },
    );

    // devstral-small-2: 24B parameters, 64K context
    // Source: devstral-small-2:24b (Mistral-based)
    // Optimized for: Coding with min_p sampling
    configs.insert(
        "devstral-small-2",
        ModelConfig {
            model_id: "devstral-small-2:24b-64k".to_string(),
            num_ctx: 65536,
            temperature: 0.15, // Very low for code accuracy
            top_k: 40,
            top_p: 0.9,
            repeat_penalty: 1.1,
        },
    );

    // deepseek-coder-v2: 16B parameters, 32K context
    // Source: deepseek-coder-v2:16b (MoE architecture)
    // Optimized for: Code generation with minimal explanation
    // MoE: 16B total, 2.4B active params per token
    // 7.5x faster than devstral-small-2 for code queries
    configs.insert(
        "deepseek-coder-v2",
        ModelConfig {
            model_id: "deepseek-coder-v2:16b-32k".to_string(),
            num_ctx: 32768,
            temperature: 0.15, // Low for deterministic code
            top_k: 40,
            top_p: 0.85,
            repeat_penalty: 1.05,
        },
    );

    // llama3.2: 3B parameters, 32K context (DEFAULT for summarization)
    // Source: llama3.2:3b with 32K context via modelfile
    // Optimized for: Fast summarization and general tasks with tool support
    // From: ~/git/ai-dotfiles/modelfiles/llama3.2.modelfile
    configs.insert(
        "llama3.2",
        ModelConfig {
            model_id: "llama3.2:3b-32k".to_string(),
            num_ctx: 32768,
            temperature: 0.2, // Lower for reliable output
            top_k: 40,
            top_p: 0.9,
            repeat_penalty: 1.1,
        },
    );

    // glm-5: 744B total parameters (40B active), 198K context
    // Source: glm-5:cloud (Z.ai GLM-5 model)
    // Cloud model optimized for complex reasoning, coding, and agentic tasks
    // Supports tools, thinking, and long context (198K)
    configs.insert(
        "glm-5",
        ModelConfig {
            model_id: "glm-5:cloud".to_string(),
            num_ctx: 197632,  // 198K context window (197,632 tokens)
            temperature: 0.7, // Optimized for agentic tasks per Z.ai benchmarks
            top_k: 40,
            top_p: 0.95, // As per SWE-bench and Terminal-Bench 2.0 evaluation settings
            repeat_penalty: 1.0,
        },
    );

    // kimi-k2.5: Native multimodal agentic model, 256K context
    // Source: kimi-k2.5:cloud (Moonshot AI)
    // Cloud model with vision, tools, thinking capabilities
    // Supports text and image inputs
    configs.insert(
        "kimi-k2.5",
        ModelConfig {
            model_id: "kimi-k2.5:cloud".to_string(),
            num_ctx: 262144, // 256K context window
            temperature: 0.7,
            top_k: 40,
            top_p: 0.95,
            repeat_penalty: 1.0,
        },
    );

    // minimax-m2.5: State-of-the-art LLM for coding and agentic tasks, 198K context
    // Source: minimax-m2.5:cloud (MiniMax)
    // Cloud model with tools and thinking capabilities
    // Trained with large-scale RL for real-world productivity
    configs.insert(
        "minimax-m2.5",
        ModelConfig {
            model_id: "minimax-m2.5:cloud".to_string(),
            num_ctx: 197632, // 198K context window
            temperature: 0.7,
            top_k: 40,
            top_p: 0.95,
            repeat_penalty: 1.0,
        },
    );

    // qwen3.5: 397B-A17B vision-language model, 256K context
    // Source: qwen3.5:cloud (Alibaba Qwen)
    // Cloud model with vision, tools, thinking capabilities
    // Hybrid architecture with linear attention and sparse MoE
    configs.insert(
        "qwen3.5",
        ModelConfig {
            model_id: "qwen3.5:cloud".to_string(),
            num_ctx: 262144, // 256K context window
            temperature: 0.7,
            top_k: 40,
            top_p: 0.95,
            repeat_penalty: 1.0,
        },
    );

    // translate: 12B parameters, 32K context
    // Source: translategemma:12b (Gemma 3 based)
    // Optimized for: Translation tasks
    configs.insert(
        "translate",
        ModelConfig {
            model_id: "translategemma:12b-32k".to_string(),
            num_ctx: 32768,
            temperature: 0.2,
            top_k: 40,
            top_p: 0.9,
            repeat_penalty: 1.1,
        },
    );

    // pepe: 8B parameters, 64K context (Assistant Pepe)
    // Source: hf.co/SicariusSicariiStuff/Assistant_Pepe_8B_GGUF:Q4_K_M
    // Character model with sarcastic personality
    configs.insert(
        "pepe",
        ModelConfig {
            model_id: "pepe:8b-64k".to_string(),
            num_ctx: 65536,
            temperature: 0.7, // Higher for personality
            top_k: 40,
            top_p: 0.9,
            repeat_penalty: 1.1,
        },
    );

    configs
});

/// Model configuration for different model presets
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub model_id: String,
    pub num_ctx: u32,
    pub temperature: f32,
    pub top_k: u32,
    pub top_p: f32,
    pub repeat_penalty: f32,
}

impl ModelConfig {
    /// Get a specific model configuration by name
    pub fn get(name: &str) -> Option<ModelConfig> {
        CONFIGS.get(name).cloned()
    }

    /// Get the default model configuration (lfm)
    #[allow(dead_code)]
    pub fn get_default() -> ModelConfig {
        CONFIGS.get(DEFAULT_MODEL).cloned().unwrap()
    }

    /// List all available model names
    pub fn list_names() -> Vec<&'static str> {
        vec![
            "lfm",               // Default - LFM 2.5 Thinking
            "gpt-oss",           // GPT-OSS 20B
            "mistral-small",     // Mistral Small 3.2 24B
            "smollm3",           // SmolLM3 3B Q8_0
            "sead",              // SEAD 14B
            "qwen3-coder",       // Qwen3 Coder 30B
            "devstral-small-2",  // Devstral 24B
            "deepseek-coder-v2", // DeepSeek Coder V2 16B (MoE, default for code)
            "llama3.2",          // Llama 3.2 3B (default for summarization)
            "glm-5",             // GLM-5 744B cloud model
            "kimi-k2.5",         // Kimi K2.5 256K cloud model
            "minimax-m2.5",      // MiniMax M2.5 198K cloud model
            "qwen3.5",           // Qwen3.5 397B 256K cloud model
            "translate",         // TranslateGemma 12B
            "pepe",              // Assistant Pepe 8B
        ]
    }

    /// Check if a model name is valid
    pub fn is_valid(name: &str) -> bool {
        CONFIGS.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_model_is_lfm() {
        let default = ModelConfig::get_default();
        assert_eq!(default.model_id, "lfm2.5-thinking:1.2b-32k");
    }

    #[test]
    fn test_all_models_exist() {
        let names = ModelConfig::list_names();
        assert_eq!(names.len(), 14);

        for name in names {
            assert!(ModelConfig::is_valid(name), "Model {} should exist", name);
            assert!(
                ModelConfig::get(name).is_some(),
                "Model {} should be retrievable",
                name
            );
        }
    }

    #[test]
    fn test_invalid_model() {
        assert!(!ModelConfig::is_valid("nonexistent"));
        assert!(ModelConfig::get("nonexistent").is_none());
    }

    #[test]
    fn test_lfm_parameters() {
        let lfm = ModelConfig::get("lfm").unwrap();
        assert_eq!(lfm.num_ctx, 32768);
        assert_eq!(lfm.temperature, 0.1);
        assert_eq!(lfm.top_p, 0.1);
        assert_eq!(lfm.repeat_penalty, 1.05);
    }

    #[test]
    fn test_gpt_oss_parameters() {
        let gpt = ModelConfig::get("gpt-oss").unwrap();
        assert_eq!(gpt.num_ctx, 65535);
        assert_eq!(gpt.temperature, 0.2);
        assert_eq!(gpt.top_k, 40);
        assert_eq!(gpt.top_p, 0.9);
    }

    #[test]
    fn test_qwen3_coder_parameters() {
        let qwen = ModelConfig::get("qwen3-coder").unwrap();
        assert_eq!(qwen.num_ctx, 65536);
        assert_eq!(qwen.temperature, 0.3);
        assert_eq!(qwen.top_k, 20);
        assert_eq!(qwen.top_p, 0.80);
        assert_eq!(qwen.repeat_penalty, 1.05);
    }
}
