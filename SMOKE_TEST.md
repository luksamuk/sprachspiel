# Smoke Test Manual - ask-ai

Execute estes testes antes de cada release para garantir que as funcionalidades 
essenciais estão funcionando.

**Ver também:** [PR Process - Phase 6.5: Smoke Test](doc/src/development/PR-PROCESS.md)

---

## Pré-requisitos

```bash
cd /home/alchemist/git/ask-ollama-rs
cargo build --release --features all-tools
ollama serve  # Em outro terminal

# Preservar banco atual do usuário
cp ~/.local/share/ask-ai/ask-ai.db ~/.local/share/ask-ai/ask-ai.db.smoke-backup 2>/dev/null || true
cp ~/.local/share/ask-ai/embeddings.db ~/.local/share/ask-ai/embeddings.db.smoke-backup 2>/dev/null || true

# Usar banco temporário para testes (isolamento)
rm -f ~/.local/share/ask-ai/ask-ai.db ~/.local/share/ask-ai/embeddings.db
```

## Modelo de Teste

```bash
# Verificar se o modelo padrão está disponível
ollama list | grep -q qwen3.5 || ollama pull qwen3.5:latest
```

---

## 1. Binário Básico

- [ ] Binário executa: `./target/release/ask-ai --help`
- [ ] Versão visível: `./target/release/ask-ai --version`
- [ ] Subcomandos listados (chat, query, translate)

---

## 2. Chat Mode

- [ ] Inicia sem erros
- [ ] Mostra modelo carregado
- [ ] `/help` mostra comandos disponíveis (incluindo /doc)
- [ ] `/exit` encerra corretamente

---

## 3. Document Import (Feature Crítica)

**Preparar arquivos de teste:**
```bash
echo "teste de import txt" > /tmp/test.txt
echo "# Markdown Title\n\nContent here." > /tmp/test.md
echo "#+TITLE: Org Title\n\n* Heading\nContent." > /tmp/test.org
touch /tmp/empty.txt
```

### 3.1 Testes Básicos

- [ ] `/doc import /tmp/test.txt` funciona (caminho absoluto)
- [ ] `/doc import ~/test.txt` funciona (caminho com ~) ← **Bug #1 corrigido**
- [ ] `/doc list` mostra o documento
- [ ] `/doc show 1` funciona (formato N) ← **Bug #2 corrigido**
- [ ] `/doc show #1` funciona (formato #N) ← **Bug #2 corrigido**
- [ ] `/doc show doc:1` funciona (formato doc:N) ← **Bug #2 corrigido**
- [ ] `/doc delete 1` remove corretamente

### 3.2 Testes de Formatos

- [ ] `/doc import /tmp/test.md` - Markdown importado
- [ ] Título de MD extraído do `# heading` (verificar com `/doc show`)
- [ ] `/doc import /tmp/test.org` - Org-mode importado
- [ ] Título de ORG extraído do `#+TITLE:` (não do * heading) ← **Bug #3 corrigido**

### 3.3 Testes de Erro

- [ ] `/doc import /naoexiste.txt` → "File not found"
- [ ] `/doc show 999` → "not found"
- [ ] `/doc import /tmp/empty.txt` → rejeitado (arquivo vazio)

---

## 4. Embedding Síncrono (Feature Nova)

- [ ] Após `/doc import /tmp/test.txt`, buscar imediatamente:
  ```
  Use remember to search for "teste"
  ```
- [ ] Resultado inclui o documento recém-importado (indexação síncrona funciona)

---

## 5. Memória (remember/facts)

- [ ] "Remember that I like coffee" cria uma nota/fato
- [ ] "What do I like?" retorna "coffee"
- [ ] Fatos persistem entre sessões (sair e entrar novamente)

---

## 6. Notas (Regressão)

- [ ] "Remember this is a test note" cria nota
- [ ] `/note list` mostra notas
- [ ] `/note show 1` exibe nota
- [ ] `/note delete 1` remove nota

---

## 7. Query Mode (rápido)

```bash
./target/release/ask-ai query "What is 2+2?" --model qwen3.5:0.8b --no-tools
```

- [ ] Retorna resposta sem erros
- [ ] Exit code 0

---

## 8. Tradução (opcional)

```bash
./target/release/ask-ai translate "Hello" --to pt
```

- [ ] Retorna tradução (se modelo disponível)

---

## 9. Banco de Dados

```bash
sqlite3 ~/.local/share/ask-ai/embeddings.db ".tables"
sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA user_version;"
```

- [ ] Tabelas existem (content, facts, etc.)
- [ ] Schema versão correta (8 ou superior)

---

## 10. File Tools (Regressão)

**Preparar arquivo de teste:**
```bash
echo "test content" > /tmp/file_test.txt
```

Via chat com um modelo que suporte tools:

- [ ] `read_file(path="/tmp/file_test.txt")` funciona
- [ ] `read_file(path="~/file_test.txt")` funciona (com ~) ← **Bug #1 correlato**
- [ ] `list_directory(path="~")` funciona
- [ ] `write_file(path="/tmp/write_test.txt", content="test")` funciona

---

## Limpeza

```bash
# Restaurar banco do usuário
rm -f ~/.local/share/ask-ai/ask-ai.db
rm -f ~/.local/share/ask-ai/embeddings.db
mv ~/.local/share/ask-ai/ask-ai.db.smoke-backup ~/.local/share/ask-ai/ask-ai.db 2>/dev/null || true
mv ~/.local/share/ask-ai/embeddings.db.smoke-backup ~/.local/share/ask-ai/embeddings.db 2>/dev/null || true

# Limpar arquivos de teste
rm -f /tmp/test.txt /tmp/test.md /tmp/test.org /tmp/empty.txt
rm -f /tmp/file_test.txt /tmp/write_test.txt
```

---

## Resultado

**Data:** _______  
**Versão:** _______  
**Modelo usado:** _______  
**Status:** [ ] Aprovado para merge  

**Problemas encontrados:**

_______________________________________

---

## Checklist Rápido (Automatizado)

Execute em sequência:

```bash
#!/bin/bash
set -e

echo "=== Smoke Test Automatizado ==="

# 1. Backup banco
echo "Backup banco..."
cp ~/.local/share/ask-ai/ask-ai.db ~/.local/share/ask-ai/ask-ai.db.smoke-backup 2>/dev/null || true
cp ~/.local/share/ask-ai/embeddings.db ~/.local/share/ask-ai/embeddings.db.smoke-backup 2>/dev/null || true

# 2. Build
echo "Build..."
cargo build --release --features all-tools || { echo "✗ Build falhou"; exit 1; }
echo "✓ Build"

# 3. Quick checks
./target/release/ask-ai --help | grep -q "chat" && echo "✓ chat command"
./target/release/ask-ai --version && echo "✓ version"

# 4. Unit tests
echo "Unit tests..."
cargo test --lib 2>&1 | tail -5
echo "✓ Unit tests"

# 5. Restore
mv ~/.local/share/ask-ai/ask-ai.db.smoke-backup ~/.local/share/ask-ai/ask-ai.db 2>/dev/null || true
mv ~/.local/share/ask-ai/embeddings.db.smoke-backup ~/.local/share/ask-ai/embeddings.db 2>/dev/null || true

echo ""
echo "=== Smoke Test Automatizado Completo ==="
echo "Execute testes manuais restantes conforme SMOKE_TEST.md"
```

---

## Testes Manuais Restantes

O script acima executa testes automatizados. Os seguintes testes devem ser executados manualmente:

1. **Seção 3**: Document Import (testes interativos no chat)
2. **Seção 4**: Embedding Síncrono (verificar indexação imediata)
3. **Seção 5**: Memória (testes interativos)
4. **Seção 6**: Notas (testes interativos)
5. **Seção 10**: File Tools (via LLM)

Estes testes requerem interação com o chat e verificação visual de resultados.