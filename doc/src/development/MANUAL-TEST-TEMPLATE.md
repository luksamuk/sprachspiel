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

---

## 1. import_document com Parâmetro title

**Objetivo:** Verificar que o parâmetro `title` funciona corretamente.

```bash
echo "This is a test document for PR #55." > /tmp/pr55_title.txt
```

No chat com LLM (modelo com suporte a tools):

- [ ] `import_document("/tmp/pr55_title.txt", None, Some("PR55 Test Document"))` retorna sucesso
- [ ] Resultado mostra `**Title:** PR55 Test Document`
- [ ] Resultado mostra `**Chunks:** N` (N ≥ 1)
- [ ] `remember(query="PR55")` encontra o documento

**Limpeza:**
```bash
rm /tmp/pr55_title.txt
```

---

## 2. Extração Automática de Título

**Objetivo:** Verificar que títulos são extraídos de Markdown/Org.

```bash
echo "# Auto Title Test" > /tmp/pr55_auto.md
echo "" >> /tmp/pr55_auto.md
echo "Content here." >> /tmp/pr55_auto.md
```

No chat com LLM:

- [ ] `import_document("/tmp/pr55_auto.md", None, None)` retorna sucesso
- [ ] Título extraído é "Auto Title Test" (do heading `#`)
- [ ] Resultado mostra `**Chunks:** N` (N ≥ 1)

**Limpeza:**
```bash
rm /tmp/pr55_auto.md
```

---

## 3. Limite de Tamanho (Rejeição > 2.5 MB)

**Objetivo:** Verificar que arquivos > 2.5 MB são rejeitados com erro útil.

```bash
dd if=/dev/zero bs=1M count=3 of=/tmp/pr55_large.txt 2>/dev/null
```

No chat com LLM:

- [ ] `import_document("/tmp/pr55_large.txt", None, Some("Large"))` retorna ERRO
- [ ] Mensagem menciona "2.5 MB" ou "2.500.000 bytes"
- [ ] Mensagem NÃO contém "5 MB" ou "5.000.000 bytes"
- [ ] Mensagem explica que arquivo é grande demais

**Limpeza:**
```bash
rm /tmp/pr55_large.txt
```

---

## 4. Arquivo Próximo ao Limite

**Objetivo:** Verificar que arquivos < 2.5 MB são aceitos.

```bash
dd if=/dev/zero bs=1M count=2 of=/tmp/pr55_medium.txt 2>/dev/null
dd if=/dev/zero bs=400K count=1 >> /tmp/pr55_medium.txt 2>/dev/null
```

No chat com LLM:

- [ ] `import_document("/tmp/pr55_medium.txt", None, Some("Medium"))` retorna sucesso
- [ ] Resultado mostra "indexed and ready for search"
- [ ] Resultado mostra `**Chunks:** N` (N ≥ 1)

**Limpeza:**
```bash
rm /tmp/pr55_medium.txt
```

---

## 5. Embedding Síncrono (Busca Imediata)

**Objetivo:** Verificar que documento é buscável imediatamente após import.

```bash
echo "PR55 unique search term: XYLOQUENT" > /tmp/pr55_sync.txt
```

No chat com LLM:

- [ ] `import_document("/tmp/pr55_sync.txt", None, Some("Sync Test"))` retorna sucesso
- [ ] IMMEDIATAMENTE após, `remember(query="XYLOQUENT")` encontra o documento
- [ ] Resultado da busca mostra "Sync Test"

**Limpeza:**
```bash
rm /tmp/pr55_sync.txt
```

---

## 6. run_command Mensagens de Erro

**Objetivo:** Verificar que mensagens de erro são úteis (não "Some(1)").

No chat com LLM:

- [ ] `run_command("pdftotext /nonexistent_file.pdf -", None, None, None)` retorna erro
- [ ] Mensagem de erro NÃO contém "Some(1)"
- [ ] Mensagem contém sugestões úteis ou explicação do erro
- [ ] Mensagem mostra "exit code 1" ou similar

---

## 7. Proteção de Documentos Grandes sem Chunks

**Objetivo:** Verificar que `remember` protege contra documentos grandes não indexados.

**Pré-condição:** Precisa de um documento > 50 KB importado SEM chunks (bug antigo).

Se não houver documento assim, pule este teste com marcação "N/A".

No chat com LLM:

- [ ] `remember(id="doc:N")` onde N é ID de documento grande sem chunks
- [ ] Mensagem explica que documento é muito grande
- [ ] Mensagem sugere `/doc delete N` e re-import
- [ ] Mensagem menciona "50.000" ou "50 KB"

---

## 8. Unidades Corretas nas Mensagens

**Objetivo:** Verificar que todas as mensagens usam unidades corretas (MB/Mb, KB/Kb).

No chat com LLM (verificar outputs dos testes anteriores):

- [ ] Mensagens de tamanho mostram "MB" ou "KB" (NÃO "Mb" ou "Kb")
- [ ] Valores em bytes são mostrados junto com unidades ("2.500.000 bytes")
- [ ] Constantes não estão hardcoded (valores vêm de MAX_DOCUMENT_SIZE)

---

## Limpeza Final

```bash
# Remover documentos de teste do banco (se necessário)
# /doc list para ver IDs
# /doc delete N para cada documento de teste
```

---

## Resultado

**Data:** _______  
**Modelo usado:** _______  
**Status:** [ ] Aprovado para merge  
**Problemas encontrados:**

_______________________________________

---

## Checklist para Merge

- [ ] Todos os testes acima passaram
- [ ] `cargo test --all-features` passou
- [ ] `cargo clippy --all-features -- -D warnings` passou
- [ ] Smoke test (SMOKE_TEST.md) passou
- [ ] Documentação revisada (CHANGELOG atualizado)
- [ ] PR reviewed e aprovado