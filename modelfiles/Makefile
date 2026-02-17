.PHONY: help models models-essential models-optional models-all models-cloud clean-models \
        lfm translategemma llama3.2 glm-ocr \
        mistral-small gpt-oss qwen3-coder pepe \
        devstral-small-2 smollm3 sead zephyr llava \
        glm-4.7-flash qwen3-coder-next deepseek-coder-v2 \
        cloud-glm-5 cloud-kimi cloud-minimax cloud-qwen35

# Default target - show help
help:
	@echo "Ask-AI Model Installation"
	@echo "========================="
	@echo ""
	@echo "Usage: make <target>"
	@echo ""
	@echo "Quick start:"
	@echo "  make models-essential  - Install models required for basic functionality"
	@echo "  make models-optional     - Install additional recommended models"
	@echo "  make models-all          - Install all local models (essential + optional)"
	@echo "  make models-cloud        - Install cloud-based models"
	@echo ""
	@echo "Individual essential models:"
	@echo "  make lfm                 - LFM 2.5 Thinking (default for query)"
	@echo "  make translategemma      - TranslateGemma 12B (for translation)"
	@echo "  make llama3.2            - Llama 3.2 3B (for summarization)"
	@echo "  make glm-ocr             - GLM-OCR (for OCR/text extraction)"
	@echo ""
	@echo "Individual optional models:"
	@echo "  make mistral-small       - Mistral Small 24B (tools support)"
	@echo "  make gpt-oss             - GPT-OSS 20B (tool calling)"
	@echo "  make qwen3-coder         - Qwen3 Coder 30B (code generation)"
	@echo "  make pepe                - Assistant Pepe 8B (character)"
	@echo "  make devstral-small-2    - Devstral 24B (coding)"
	@echo "  make deepseek-coder-v2   - DeepSeek Coder V2 16B (MoE code model)"
	@echo "  make smollm3             - SmolLM3 3B (lightweight)"
	@echo "  make sead                - SEAD 14B (general purpose)"
	@echo "  make zephyr              - Zephyr 7B (general)"
	@echo "  make llava               - LLaVA 7B (vision)"
	@echo "  make glm-4.7-flash       - GLM-4.7 Flash 64K (reasoning)"
	@echo "  make qwen3-coder-next    - Qwen3 Coder Next (experimental)"
	@echo ""
	@echo "Cloud models (requires internet):"
	@echo "  make cloud-glm-5         - GLM-5 Cloud (198K context)"
	@echo "  make cloud-kimi          - Kimi K2.5 Cloud (256K context)"
	@echo "  make cloud-minimax       - MiniMax M2.5 Cloud (198K context)"
	@echo "  make cloud-qwen35        - Qwen3.5 Cloud (256K context)"
	@echo ""
	@echo "Maintenance:"
	@echo "  make clean-models        - Remove temporary model files"
	@echo ""
	@echo "Note: All models except cloud models are optional but recommended"
	@echo "      for the best experience with ask-ai."

# Generic macro for building models from modelfiles
# Usage: $(call BUILD,<model-name>,<modelfile>)
define BUILD
	@set -e; \
	base=$$(awk 'NR==1 && $$1=="FROM" {print $$2}' "$(2)"); \
	test -n "$$base"; \
	echo "==> Pulling base model: $$base"; \
	ollama pull "$$base"; \
	echo "==> Creating $(1)"; \
	ollama create "$(1)" -f "$(2)"; \
	echo "==> Cleaning up base model: $$base"; \
	ollama rm "$$base" || true; \
	echo "==> Model $(1) installed successfully"
endef

# ============================================================================
# ESSENTIAL MODELS (Required for basic functionality)
# ============================================================================

models-essential: lfm translategemma llama3.2 glm-ocr
	@echo ""
	@echo "=========================================="
	@echo "Essential models installed successfully!"
	@echo "=========================================="
	@echo ""
	@echo "You can now use:"
	@echo "  - ask-ai 'query'         (uses lfm)"
	@echo "  - ask-ai translate ...   (uses translategemma)"
	@echo "  - ask-ai summarize ...   (uses llama3.2)"
	@echo "  - ask-ai ocr ...         (uses glm-ocr)"
	@echo ""
	@echo "For more models, run: make models-optional"

# LFM 2.5 Thinking - Default model for general queries
lfm:
	$(call BUILD,lfm2.5-thinking:1.2b-32k,lfm2.5-thinking.modelfile)

# TranslateGemma - Required for translation command
translategemma:
	$(call BUILD,translategemma:12b-32k,translategemma.modelfile)

# Llama 3.2 - Default for summarization
llama3.2:
	$(call BUILD,llama3.2:3b-32k,llama3.2.modelfile)

# GLM-OCR - Required for OCR command (pull-only, no modelfile needed)
glm-ocr:
	@echo "==> Pulling glm-ocr:bf16"
	ollama pull "glm-ocr:bf16"
	@echo "==> glm-ocr installed successfully"

# ============================================================================
# OPTIONAL MODELS (Recommended but not required)
# ============================================================================

models-optional: mistral-small gpt-oss qwen3-coder pepe
	@echo ""
	@echo "============================================"
	@echo "Optional models installed successfully!"
	@echo "============================================"
	@echo ""
	@echo "Additional models available:"
	@echo "  - mistral-small: Tool-capable model"
	@echo "  - gpt-oss: Tool calling model"
	@echo "  - qwen3-coder: Code generation"
	@echo "  - pepe: Character model"
	@echo ""
	@echo "For more optional models:"
	@echo "  make devstral-small-2  # Coding focused"
	@echo "  make smollm3            # Lightweight"
	@echo "  make sead               # General purpose"
	@echo "  make llava              # Vision"

# Mistral Small - Tool-capable model
mistral-small:
	$(call BUILD,mistral-small3.2:24b-32k,mistral-small3.2.modelfile)

# GPT-OSS - Tool calling model
gpt-oss:
	$(call BUILD,gpt-oss:20b-64k,gpt-oss.modelfile)

# Qwen3 Coder - Code generation
qwen3-coder:
	$(call BUILD,qwen3-coder:30b-64k,qwen3-coder.modelfile)

# Assistant Pepe - Character model
pepe:
	$(call BUILD,pepe:8b-64k,assistant-pepe.modelfile)

# Devstral Small 2 - Coding with min_p
devstral-small-2:
	$(call BUILD,devstral-small-2:24b-64k,devstral-small-2.modelfile)

# DeepSeek Coder V2 - MoE code model (16B total, 2.4B active)
deepseek-coder-v2:
	$(call BUILD,deepseek-coder-v2:16b-32k,deepseek-coder-v2.modelfile)

# SmolLM3 - Lightweight model
smollm3:
	$(call BUILD,smollm3:Q8_0-64k,smollm3-3b.modelfile)

# SEAD - General purpose
sead:
	$(call BUILD,sead:14b-32k,sead-14b.modelfile)

# Zephyr - General model
zephyr:
	$(call BUILD,zephyr:7b-32k,zephyr.modelfile)

# LLaVA - Vision model
llava:
	$(call BUILD,llava:7b-32k,llava-7b.modelfile)

# ============================================================================
# SPECIALIZED MODELS
# ============================================================================

# GLM-4.7 Flash - Reasoning model
glm-4.7-flash:
	$(call BUILD,glm-4.7-flash:q4_K_M-64k,glm-4.7-flash.modelfile)

# Qwen3 Coder Next - Experimental coder
qwen3-coder-next:
	$(call BUILD,qwen3-coder-next:q4_K_M-32k,qwen3-coder-next.modelfile)

# ============================================================================
# CLOUD MODELS (Requires internet, large context windows)
# ============================================================================

models-cloud: cloud-glm-5 cloud-kimi cloud-minimax cloud-qwen35
	@echo ""
	@echo "==================================="
	@echo "Cloud models installed!"
	@echo "==================================="
	@echo ""
	@echo "Available cloud models:"
	@echo "  - glm-5:cloud        (198K context)"
	@echo "  - kimi-k2.5:cloud    (256K context)"
	@echo "  - minimax-m2.5:cloud (198K context)"
	@echo "  - qwen3.5:cloud      (256K context)"

cloud-glm-5:
	@echo "==> Pulling glm-5:cloud"
	ollama pull "glm-5:cloud"
	@echo "==> glm-5:cloud installed"

cloud-kimi:
	@echo "==> Pulling kimi-k2.5:cloud"
	ollama pull "kimi-k2.5:cloud"
	@echo "==> kimi-k2.5:cloud installed"

cloud-minimax:
	@echo "==> Pulling minimax-m2.5:cloud"
	ollama pull "minimax-m2.5:cloud"
	@echo "==> minimax-m2.5:cloud installed"

cloud-qwen35:
	@echo "==> Pulling qwen3.5:cloud"
	ollama pull "qwen3.5:cloud"
	@echo "==> qwen3.5:cloud installed"

# ============================================================================
# COMBINED TARGETS
# ============================================================================

# All local models (essential + optional)
models-all: models-essential models-optional
	@echo ""
	@echo "==================================="
	@echo "All local models installed!"
	@echo "==================================="
	@echo ""
	@echo "To install cloud models, run:"
	@echo "  make models-cloud"

# Complete installation (everything)
models: models-all
	@echo ""
	@echo "========================================="
	@echo "All models installation complete!"
	@echo "========================================="
	@echo ""
	@echo "Installed essential models:"
	@echo "  - lfm2.5-thinking:1.2b-32k"
	@echo "  - translategemma:12b-32k"
	@echo "  - llama3.2:3b-32k"
	@echo "  - glm-ocr:bf16"
	@echo ""
	@echo "Installed optional models:"
	@echo "  - mistral-small3.2:24b-32k"
	@echo "  - gpt-oss:20b-64k"
	@echo "  - qwen3-coder:30b-64k"
	@echo "  - pepe:8b-64k"
	@echo ""
	@echo "Note: Run 'make models-cloud' for cloud models"

# ============================================================================
# CLEANUP
# ============================================================================

clean-models:
	@echo "Cleaning up temporary files..."
	@echo "Note: This only removes temporary files, not installed models"
	@echo "To remove installed models, use: ollama rm <model-name>"

# List all installed models
list:
	@echo "Checking installed models..."
	@ollama list | grep -E "(lfm|translategemma|llama3.2|glm-ocr|mistral|gpt-oss|qwen|pepe|devstral|smollm|sead|zephyr|llava|glm-5|kimi|minimax)" || echo "No ask-ai models found installed"
