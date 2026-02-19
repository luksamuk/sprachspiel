.PHONY: help models models-essential models-cloud clean-models \
        llama31 lfm translategemma llama32 glm-ocr \
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
	@echo "  make models-cloud      - Install cloud-based models"
	@echo ""
	@echo "Individual essential models:"
	@echo "  make llama31           - Llama 3.1 8B (default for query)"
	@echo "  make translategemma    - TranslateGemma 12B (for translation)"
	@echo "  make glm-ocr           - GLM-OCR (for OCR/text extraction)"
	@echo ""
	@echo "Optional local models:"
	@echo "  make lfm               - LFM 2.5 Thinking (reasoning with think mode)"
	@echo "  make llama32           - Llama 3.2 3B (summarization, tools)"
	@echo ""
	@echo "Cloud models (requires internet):"
	@echo "  make cloud-glm-5       - GLM-5 Cloud"
	@echo "  make cloud-kimi        - Kimi K2.5 Cloud"
	@echo "  make cloud-minimax     - MiniMax M2.5 Cloud"
	@echo "  make cloud-qwen35      - Qwen3.5 Cloud"
	@echo ""
	@echo "Maintenance:"
	@echo "  make clean-models      - Remove temporary model files"
	@echo "  make list              - Show installed Ask-AI models"
	@echo ""
	@echo "Note: Additional models can be defined in ~/.config/ask-ai/models.toml"
	@echo "      Context sizes are configured in models.toml, not model tags."

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

models-essential: llama31 translategemma glm-ocr
	@echo ""
	@echo "=========================================="
	@echo "Essential models installed successfully!"
	@echo "=========================================="
	@echo ""
	@echo "You can now use:"
	@echo "  - ask-ai 'query'         (uses llama3.1)"
	@echo "  - ask-ai translate ...   (uses translategemma)"
	@echo "  - ask-ai ocr ...         (uses glm-ocr)"
	@echo ""
	@echo "For additional models, see:"
	@echo "  make lfm        (reasoning with think mode)"
	@echo "  make llama32    (summarization, tools)"
	@echo "  ~/.config/ask-ai/models.toml"

# Llama 3.1 - Default model for general queries
llama31:
	@echo "==> Pulling llama3.1:8b"
	ollama pull "llama3.1:8b"
	@echo "==> llama3.1:8b installed successfully"

# TranslateGemma - Required for translation command
translategemma:
	@echo "==> Pulling translategemma:12b"
	ollama pull "translategemma:12b"
	@echo "==> translategemma:12b installed successfully"

# GLM-OCR - Required for OCR command
glm-ocr:
	@echo "==> Pulling glm-ocr:bf16"
	ollama pull "glm-ocr:bf16"
	@echo "==> glm-ocr:bf16 installed successfully"

# ============================================================================
# OPTIONAL LOCAL MODELS
# ============================================================================

# LFM 2.5 Thinking - Reasoning model with think mode
lfm:
	$(call BUILD,lfm2.5-thinking:1.2b,lfm2.5-thinking.modelfile)

# Llama 3.2 - For summarization and tools
llama32:
	$(call BUILD,llama3.2:3b,llama3.2.modelfile)

# ============================================================================
# CLOUD MODELS (Requires internet)
# ============================================================================

models-cloud: cloud-glm-5 cloud-kimi cloud-minimax cloud-qwen35
	@echo ""
	@echo "==================================="
	@echo "Cloud models installed!"
	@echo "==================================="
	@echo ""
	@echo "Available cloud models:"
	@echo "  - glm-5:cloud"
	@echo "  - kimi-k2.5:cloud"
	@echo "  - minimax-m2.5:cloud"
	@echo "  - qwen3.5:cloud"

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

# All models (essential + optional + cloud)
models: models-essential lfm llama32 models-cloud
	@echo ""
	@echo "========================================="
	@echo "All models installation complete!"
	@echo "========================================="
	@echo ""
	@echo "Installed essential models:"
	@echo "  - llama3.1:8b          (default)"
	@echo "  - translategemma:12b   (translation)"
	@echo "  - glm-ocr:bf16         (OCR)"
	@echo ""
	@echo "Installed optional models:"
	@echo "  - lfm2.5-thinking:1.2b (reasoning)"
	@echo "  - llama3.2:3b          (summarization, tools)"
	@echo ""
	@echo "For additional models, add them to:"
	@echo "  ~/.config/ask-ai/models.toml"

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
	@ollama list | grep -E "(llama3|translategemma|glm|lfm|kimi|minimax|qwen)" || echo "No ask-ai models found installed"