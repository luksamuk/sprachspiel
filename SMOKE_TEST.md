# Smoke Test Manual - ask-ai

Execute estes testes antes de cada release para garantir que as funcionalidades 
essenciais estão funcionando.

**Ver também:** [PR Process - Phase 6.5: Smoke Test](doc/src/development/PR-PROCESS.md)

---

## Terminal-Use (Opcional)

Este smoke test foi projetado para ser executado por um agente de IA usando a ferramenta **terminal-use**, que permite controle automatizado do terminal.

**Instalação:** [github.com/flipbit03/terminal-use](https://github.com/flipbit03/terminal-use)

**Configuração obrigatória:** Usar terminal com **80 colunas de largura** para garantir formatação consistente das saídas.

---

## Pré-requisitos

```bash
cd /home/alchemist/git/ask-ollama-rs
cargo build --release --features all-tools
ollama serve  # Em outro terminal

# Preservar banco atual do usuário
cp ~/.local/share/ask-ai/embeddings.db ~/.local/share/ask-ai/embeddings.db.smoke-backup 2>/dev/null || true

# Usar banco temporário para testes (isolamento)
rm -f ~/.local/share/ask-ai/embeddings.db
```

## Modelo de Teste

```bash
# Usar variável de ambiente ou padrão
MODEL=${SMOKE_MODEL:-qwen3.5:4b}
ollama list | grep -q "$MODEL" || ollama pull "$MODEL"
echo "Modelo de teste: $MODEL"
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
# Arquivo para teste de tilde expansion (Bug #1)
echo "teste tilde expansion" > ~/test.txt
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

### 3.4 Testes de Tamanho (Bug #54)

- [ ] Arquivo > 2.5MB é rejeitado com erro claro:
  ```bash
  # Criar arquivo grande
  dd if=/dev/zero bs=1M count=3 of=/tmp/large.txt 2>/dev/null
  # Verificar que /doc import rejeita
  /doc import /tmp/3mb.txt
  ```
- [ ] Mensagem de erro menciona "2.5 MB limit" e sugere dividir arquivo

---

## 4. Embedding Síncrono (Feature Nova)

- [ ] Após `/doc import /tmp/test.txt`, buscar imediatamente:
  ```
  Use remember to search for "teste"
  ```
- [ ] Resultado inclui o documento recém-importado (indexação síncrona funciona)

### 4.1 Tool import_document (via LLM) - Bug #54

**Preparar arquivos de teste:**
```bash
echo "teste de import via tool" > /tmp/tool_test.txt
```

Via chat com modelo que suporta tools:

- [ ] `import_document("/tmp/tool_test.txt", None, Some("Test Document"))` funciona
- [ ] Import retorna "Chunks: N (document indexed and ready for search)"
- [ ] `remember(query="teste")` encontra o documento
- [ ] Tool retorna título correto quando fornecido

### 4.2 Proteção de Documentos Grandes (Bug #54)

**Preparar documento grande sem chunks:**
```bash
# Criar documento grande no banco manualmente (simulando import bugado)
# Depois verificar que remember protege contra retorno completo
```

- [ ] `remember(id="doc:N")` em documento > 50KB sem chunks retorna erro
- [ ] Mensagem explica como re-importar o documento
- [ ] Sugere `/doc delete N` e re-import

---

## 5. Memória (remember/facts)

**Nota:** Usar modelo com suporte a tools (qwen3.5:4b ou maior). Modelos pequenos como 0.8b podem ter dificuldade com tool calling.

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

## 7. Query Mode

**Nota:** Query mode carrega contexto completo (AGENTS.md, SOUL.md, tools). Para teste rápido, usar `--soulless --ignore-agents` ou aumentar timeout.

```bash
# Teste rápido (sem contexto pesado) - flags ANTES do subcomando
timeout 60 ./target/release/ask-ai --soulless --ignore-agents query "2+2"

# Teste completo (com contexto)
timeout 120 ./target/release/ask-ai query "What is 2+2?"
```

- [ ] Retorna resposta sem erros
- [ ] Exit code 0

---

## 8. Tradução (opcional)

```bash
./target/release/ask-ai translate pt "Hello"
```

- [ ] Retorna tradução (se modelo disponível)

---

## 9. Banco de Dados

```bash
sqlite3 ~/.local/share/ask-ai/embeddings.db ".tables"
sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA user_version;"
```

- [ ] Tabelas existem (content, facts, conversations, etc.)
- [ ] Schema versão correta (8 ou superior)

**Verificação explícita:**
```bash
SCHEMA_VER=$(sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA user_version;")
[ "$SCHEMA_VER" -ge 8 ] && echo "✓ schema v$SCHEMA_VER" || echo "✗ schema v$SCHEMA_VER < 8"
```

---

## 10. File Tools (Regressão)

**Preparar arquivos de teste:**
```bash
echo "test content" > /tmp/file_test.txt
# Arquivo para teste de tilde expansion (Bug #1 correlato)
echo "file tools test" > ~/file_test.txt
```

Via chat com um modelo que suporte tools:

- [ ] `read_file(path="/tmp/file_test.txt")` funciona
- [ ] `read_file(path="~/file_test.txt")` funciona (com ~) ← **Bug #1 correlato**
- [ ] `list_directory(path="~")` funciona
- [ ] `write_file(path="/tmp/write_test.txt", content="test")` funciona

---

## 10.5. run_command Error Messages (Bug #54)

Via chat com modelo que suporte tools:

- [ ] `run_command("pdftotext /nonexistent.pdf -")` retorna erro útil
- [ ] Mensagem menciona "file does not exist" ou similar
- [ ] Mensagem NÃO contém "Some(1)" genérico

---

## Limpeza

```bash
# Restaurar banco do usuário
rm -f ~/.local/share/ask-ai/embeddings.db
mv ~/.local/share/ask-ai/embeddings.db.smoke-backup ~/.local/share/ask-ai/embeddings.db 2>/dev/null || true

# Limpar arquivos de teste (/tmp e ~)
rm -f /tmp/test.txt /tmp/test.md /tmp/test.org /tmp/empty.txt
rm -f /tmp/file_test.txt /tmp/write_test.txt
rm -f /tmp/tool_test.txt /tmp/large.txt
rm -f ~/test.txt ~/file_test.txt
```

---

## 11. Performance Básica

```bash
# Tempo aceitável para query simples (sem contexto)
# Nota: flags globais ANTES do subcomando
time (timeout 30 ./target/release/ask-ai --soulless --ignore-agents query "2+2" > /dev/null)
# Deve completar em < 15 segundos em hardware normal
```

- [ ] Query simples completa em tempo razoável (< 15s)

---

## Resultado

**IMPORTANTE:** Os resultados do smoke test devem ser guardados **fora do projeto** (ex: comentário no PR, issue, ou documento externo). **NÃO MODIFIQUE ESTE ARQUIVO** com resultados - ele é um template reutilizável.

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
3. **Seção 5**: Memória (testes interativos com modelo >= 4b)
4. **Seção 6**: Notas (testes interativos)
5. **Seção 10**: File Tools (via LLM)
6. **Seção 11**: Performance (verificar tempo de resposta)

Estes testes requerem interação com o chat e verificação visual de resultados.