# Manual Test - PR #55

Execute estes testes após os testes automatizados passarem, antes de finalizar o merge.

**Issue:** #54 - import_document tool missing embedding/chunking + large doc protection

**Branch:** fix/document-import-embedding

---

## Pré-requisitos

```bash
cd /home/alchemist/git/ask-ollama-rs
cargo build --release --features all-tools
```

Confirmar que o build passou:
- [ ] `cargo build --release --features all-tools` completa sem erros
- [ ] Binário gerado em `target/release/ask-ai`

---

## 1. import_document com Parâmetro title

**Objetivo:** Verificar que o parâmetro `title` funciona corretamente.

**Setup:**
```bash
echo "This is a test document for PR #55." > /tmp/pr55_title.txt
```

**Testes no chat com LLM (modelo com suporte a tools):**

- [ ] `import_document("/tmp/pr55_title.txt", None, Some("PR55 Test Document"))` retorna sucesso
- [ ] Resultado mostra `**Title:** PR55 Test Document`
- [ ] Resultado mostra `**Chunks:** N` onde N ≥ 1
- [ ] Resultado mostra "indexed and ready for search"
- [ ] `remember(query="PR55")` encontra o documento imediatamente

**Limpeza:**
```bash
rm /tmp/pr55_title.txt
# Opcional: /doc delete N (para remover do banco)
```

---

## 2. Extração Automática de Título (Markdown)

**Objetivo:** Verificar que títulos são extraídos de heading `#`.

**Setup:**
```bash
echo "# Auto Title Test" > /tmp/pr55_auto.md
echo "" >> /tmp/pr55_auto.md
echo "Content here." >> /tmp/pr55_auto.md
```

**Testes no chat com LLM:**

- [ ] `import_document("/tmp/pr55_auto.md", None, None)` retorna sucesso
- [ ] Título extraído é "Auto Title Test" (do heading `#`)
- [ ] Resultado mostra `**Chunks:** N` onde N ≥ 1

**Limpeza:**
```bash
rm /tmp/pr55_auto.md
```

---

## 3. Limite de Tamanho (Rejeição > 2.5 MB)

**Objetivo:** Verificar que arquivos > 2.5 MB são rejeitados com erro útil.

**Setup:**
```bash
dd if=/dev/zero bs=1M count=3 of=/tmp/pr55_large.txt 2>/dev/null
```

**Testes no chat com LLM:**

- [ ] `import_document("/tmp/pr55_large.txt", None, Some("Large"))` retorna ERRO
- [ ] Mensagem de erro contém "2.5 MB" ou "2,500,000 bytes"
- [ ] Mensagem de erro NÃO contém "5 MB" ou "5,000,000 bytes"
- [ ] Mensagem explica que arquivo excede limite
- [ ] Mensagem sugere alternativa (split ou arquivo menor)

**Limpeza:**
```bash
rm /tmp/pr55_large.txt
```

---

## 4. Arquivo Próximo ao Limite (< 2.5 MB)

**Objetivo:** Verificar que arquivos < 2.5 MB são aceitos.

**Setup:**
```bash
dd if=/dev/zero bs=1M count=2 of=/tmp/pr55_medium.txt 2>/dev/null
dd if=/dev/zero bs=400K count=1 >> /tmp/pr55_medium.txt 2>/dev/null
```

**Testes no chat com LLM:**

- [ ] `import_document("/tmp/pr55_medium.txt", None, Some("Medium Test"))` retorna sucesso
- [ ] Resultado mostra "indexed and ready for search"
- [ ] Resultado mostra `**Chunks:** N` onde N ≥ 1

**Limpeza:**
```bash
rm /tmp/pr55_medium.txt
```

---

## 5. Embedding Síncrono (Busca Imediata)

**Objetivo:** Verificar que documento é buscável IMEDIATAMENTE após import.

**Setup:**
```bash
echo "PR55 unique search term: XYLOQUENT" > /tmp/pr55_sync.txt
```

**Testes no chat com LLM:**

- [ ] `import_document("/tmp/pr55_sync.txt", None, Some("Sync Test"))` retorna sucesso
- [ ] IMEDIATAMENTE após import (sem `/reindex`), `remember(query="XYLOQUENT")` encontra o documento
- [ ] Resultado da busca mostra "Sync Test" como título

**Importante:** Não rodar `/reindex` entre import e search. O embedding deve ser síncrono.

**Limpeza:**
```bash
rm /tmp/pr55_sync.txt
```

---

## 6. run_command Mensagens de Erro

**Objetivo:** Verificar que mensagens de erro não mostram "Some(1)" genérico.

**Testes no chat com LLM:**

- [ ] `run_command("pdftotext /nonexistent_file.pdf -", None, None, None)` retorna erro
- [ ] Mensagem de erro NÃO contém "Some(1)"
- [ ] Mensagem de erro contém sugestões úteis ou causa provável
- [ ] Mensagem mostra formato limpo (ex: "exit code 1")

---

## 7. Proteção de Documentos Grandes sem Chunks

**Objetivo:** Verificar que `remember` protege contra documentos grandes não indexados.

**Pré-condição:** Este teste requer um documento > 50 KB importado ANTES do bug fix (sem chunks).

Se não houver documento assim no banco, pule este teste com marcação "N/A".

**Testes no chat com LLM:**

- [ ] `remember(id="doc:N")` onde N é ID de documento grande sem chunks
- [ ] Mensagem explica que documento é muito grande para exibir
- [ ] Mensagem menciona "50 KB" ou "50,000" 
- [ ] Mensagem sugere `/doc delete N` e re-import
- [ ] Mensagem NÃO retorna o conteúdo completo do documento

**Status se não aplicável:** [ ] N/A - Não há documento grande sem chunks no banco

---

## 8. Consistência de Unidades (MB/Mb, KB/Kb)

**Objetivo:** Verificar que todas as mensagens usam unidades corretas.

**Verificação nos testes anteriores:**

- [ ] Todas as mensagens de tamanho usam "MB" (megabytes), não "Mb" (megabits)
- [ ] Todas as mensagens de tamanho usam "KB" (kilobytes), não "Kb" (kilobits)
- [ ] Valores em bytes são mostrados junto com unidades legíveis
- [ ] Não há valores hardcoded (usam constantes)

**Verificar especificamente:**
- Teste 3 (limite de tamanho): mensagem contém "2.5 MB" e/ou "2,500,000 bytes"
- Teste 7 (proteção): mensagem contém "50 KB" ou similar

---

## Resultado

**Data:** _______  
**Executor:** Hermes Agent  
**Modelo usado para testes:** _______  
**Binário testado:** target/release/ask-ai v0.39.5

**Status:** [ ] Aprovado para merge

**Problemas encontrados:**

_______________________________________
_______________________________________

---

## Checklist Final para Merge

- [ ] Todos os 8 testes acima passaram (ou N/A marcado justificadamente)
- [ ] `cargo test --lib` passou
- [ ] `cargo clippy --all-features -- -D warnings` passou
- [ ] Smoke test (SMOKE_TEST.md) passou
- [ ] Documentação revisada (CHANGELOG, tools.md, PR-PROCESS)
- [ ] Todos os comentários de review foram respondidos
- [ ] PR está "ready for review" (não draft)