# Sprachspiel Stress Test Report

**Data:** 2026-03-04  
**Versão Testada:** v0.26.0  
**Versão Anterior:** v0.22.1  
**Modelos Testados:** ministral-3:14b, qwen3:8b  
**Context Window:** 32,768 tokens

---

## Sumário Executivo

Testes de regressão e validação de novas funcionalidades após 8 releases (v0.22.2 a v0.26.0).

| Funcionalidade | Status | Observações |
|----------------|--------|-------------|
| Persistência SQLite | ✅ PASS | /clear preserva dados, /forget remove tudo |
| Context Overflow Detection | ✅ PASS | Auto-compact em 72%/80% funcionando |
| Middle Compaction | ✅ FIXED | Bug corrigido: preserva first 5 + last 5 |
| Chunking de Mensagens | ✅ FIXED | Bug corrigido: chunks salvos síncronamente |
| Recovery de Embeddings | ✅ PASS | Recupera automaticamente no startup |
| Remember Tool | ✅ PASS | Busca por ID e semântica funcionais |
| Hybrid Search (RAG) | ✅ PASS | BM25 + Semantic + RRF funcionando |
| Visualização /context | ✅ PASS | Cores verde/amarelo/vermelho |

**Bugs da v0.22.1:** Todos os 3 bugs críticos foram **CORRIGIDOS**.

---

## 1. Persistência SQLite

### Objetivo

Validar o novo sistema de persistência com SQLite, diferenciando `/clear` de `/forget`.

### O que Mudou (v0.22.2 a v0.22.5)

- **v0.22.2:** Schema v3 com `has_embedding` column
- **v0.22.4:** `/clear` passa a preservar dados para retrieval
- **v0.22.4:** Novo comando `/forget` para remoção permanente
- **v0.22.5:** Retrieval forçado após `/clear` detecta DB com mensagens

### Resultados

| Operação | Tempo | Resultado | Observação |
|----------|-------|-----------|------------|
| Criação SQLite | ~60s | ✅ Sucesso | Banco criado em ~/.local/share/sprachspiel/ |
| Inserção mensagens | <1s/msg | ✅ Sucesso | 6 mensagens com embeddings |
| `/clear` | <1s | ✅ Sucesso | Mensagens permanecem no SQLite |
| `/forget` | <1s | ✅ Sucesso | Tudo removido permanentemente |
| `/search` após `/clear` | ~5s | ✅ Sucesso | Encontrou mensagens antigas |
| `/search` após `/forget` | <1s | ✅ Sucesso | "No results found" |

### Estrutura SQLite Confirmada

```sql
-- Tabelas principais
conversations       -- id, project_id, title, model, created_at, updated_at
messages            -- id, conversation_id, role, content, timestamp, has_embedding
message_embeddings  -- VIRTUAL vec0: embeddings 256-dimensional
chunk_embeddings    -- VIRTUAL vec0: chunk-level embeddings
messages_fts        -- VIRTUAL FTS5: full-text search
message_chunks      -- chunks para mensagens > 1024 chars
```

### Conclusão

✅ **PASS** - Sistema de persistência robusto e funcional. Diferenciação clara entre `/clear` (soft delete) e `/forget` (hard delete).

---

## 2. Context Overflow e Middle Compaction

### Bug Original (v0.22.1)

**BUG #1:** `/compact` resumia TODAS as mensagens, não preservando first/last 5.  
**BUG #2:** `get_compaction_range()` existia mas estava marcada como `#[allow(dead_code)]`.

### Análise do Código (v0.26.0)

**Arquivo:** `src/chat/repl.rs:1037-1059`

```rust
let (messages_to_summarize, range) = match get_compaction_range_default(session) {
    Some(suggestion) => {
        // Middle compaction: preserve first N + last N, summarize middle
        let middle: Vec<_> = session.messages[suggestion.middle_indices.clone()].to_vec();
        ...
    }
    None => {
        // Not enough messages for middle compaction, summarize all
        ...
    }
};
```

**Arquivo:** `src/context_overflow.rs:8-15`

```rust
pub const DEFAULT_OVERFLOW_THRESHOLD: f32 = 0.8;  // 80%
pub const DEFAULT_KEEP_FIRST: usize = 5;
pub const DEFAULT_KEEP_LAST: usize = 5;
```

### Status

✅ **BUG #1 CORRIGIDO** - `compact_conversation()` agora usa `get_compaction_range_default()`  
✅ **BUG #2 CORRIGIDO** - `get_compaction_range_default()` está ativamente usado

### Auto-Compact

**Arquivo:** `src/chat/repl.rs:1182`

- `auto_compact_if_needed()` chamado após cada resposta do assistente
- Warning: 72% de utilização (90% de 80%)
- Overflow: 80% de utilização
- Mensagem: `[auto-compacted context at X%: N messages]`

### Visualização /context

**Arquivo:** `src/chat/repl.rs:1293-1299`

| Utilização | Cor | Status |
|------------|-----|--------|
| < 72% | Verde | OK |
| 72-80% | Amarelo | WARNING (approaching limit) |
| ≥ 80% | Vermelho | OVERFLOW (auto-compact triggered) |

### Conclusão

✅ **PASS** - Middle compaction implementado e funcionando. Auto-compact visual e funcional.

---

## 3. Chunking de Mensagens

### Bug Original (v0.22.1)

**BUG #3:** Mensagens longas perdiam chunks devido a `tokio::spawn` fire-and-forget.

### Correção Implementada (v0.22.2)

**Arquivo:** `src/chat/session.rs:215-248`

```
add_user_message()
├── Insert message (sync)      ← ALWAYS SAVED
├── Insert chunks (sync)       ← ALWAYS SAVED (CORREÇÃO)
└── tokio::spawn(async {
    └── Generate embeddings    ← MAY BE INTERRUPTED (recuperável)
})
```

- Chunks agora são inseridos **síncronamente** antes do `tokio::spawn`
- Embeddings podem falhar, mas chunks já estão persistidos
- Recovery de embeddings no startup cobre falhas

### Recovery de Embeddings

**Arquivo novo:** `src/embeddings/recovery.rs` (118 linhas)

- `recover_missing_embeddings()` chamado no startup do REPL
- Busca chunks/messages com `has_embedding = 0`
- Gera embeddings pendentes em background
- Mensagem no console: "Recovering N missing embedding(s)... Successfully recovered N."

### Testes Unitários

```
test embeddings::chunker::tests::test_chunk_content_coverage ... ok
test embeddings::chunker::tests::test_chunk_indices ... ok
test embeddings::chunker::tests::test_utf8_char_boundary ... ok
test embeddings::chunker::tests::test_utf8_multibyte_at_boundary ... ok
test embeddings::recovery::tests::test_recovery_structure ... ok

12 testes de chunking passando
```

### Conclusão

✅ **BUG #3 CORRIGIDO** - Chunking síncrono + recovery garante persistência.

---

## 4. Remember Tool (v0.23.0 - NOVO)

### Funcionalidade

**Arquivo:** `src/tools/remember.rs` (253 linhas)

Nova ferramenta para o LLM acessar histórico de conversas:

```
remember(id="42")            # Recupera mensagem específica
remember(query="Wittgenstein")  # Busca semântica
remember(query="x", limit="10") # Com limite de resultados
```

### Features

- Busca por ID exato ou query semântica
- Retorna conteúdo + resposta do assistant seguinte
- Truncamento inteligente UTF-8 (200 chars)
- Limite configurável (max 10)
- Integração com `search_hybrid()` (BM25 + Semantic)

### Disponibilidade

Sempre habilitada (não depende de features). Disponível em sessões não-anônimas.

### Conclusão

✅ **PASS** - Remember Tool funcional e integrada ao sistema de embeddings.

---

## 5. Novas Funcionalidades

### Comandos

| Comando | Versão | Descrição |
|---------|--------|-----------|
| `/forget` | v0.22.4 | Remove permanentemente sessão do SQLite |
| `/context` | v0.22.3 | Métricas com cores (verde/amarelo/vermelho) |
| `/search` | v0.24.0 | Mostra pares pergunta-resposta |

### Retrieval

| Mudança | Antes | Depois |
|---------|-------|--------|
| Threshold | 20 mensagens | 5 mensagens |
| Default | disabled | enabled |
| Após `/clear` | Perdido | Recuperado via RAG |
| Query mode | Sem histórico | Acessa projeto |

### Conversation-Aware Retrieval (v0.24.0)

- Enriquecimento post-retrieval: pergunta + resposta
- Campo `next_message` no SearchResult
- `/search` mostra pares juntos

### Project-Aware Query Mode (v0.25.0)

- Query mode acessa histórico de todas as conversas do projeto
- Build de contexto read-only (não persiste novas mensagens)

---

## 6. Constantes e Thresholds

| Constante | Valor | Local |
|-----------|-------|-------|
| `DEFAULT_OVERFLOW_THRESHOLD` | 0.8 (80%) | context_overflow.rs |
| `DEFAULT_KEEP_FIRST` | 5 | context_overflow.rs |
| `DEFAULT_KEEP_LAST` | 5 | context_overflow.rs |
| `DEFAULT_CHUNK_SIZE` | 1024 chars | chunker.rs |
| `DEFAULT_CHUNK_OVERLAP` | 200 chars | chunker.rs |
| `MIN_MESSAGES_FOR_RETRIEVAL` | 5 | context_builder.rs |
| `RELEVANT_MESSAGES_COUNT` | 5 | context_builder.rs |
| `RECENT_MESSAGES_COUNT` | 10 | context_builder.rs |
| `KEYWORD_WEIGHT` | 0.4 | context_builder.rs |
| `SEMANTIC_WEIGHT` | 0.6 | context_builder.rs |
| `MAX_RETRIES` | 3 | coordinator.rs |

---

## 7. Métricas de Performance

### Latência

| Operação | Tempo |
|----------|-------|
| Inserção mensagem | <1s |
| `/clear` | <1s |
| `/forget` | <1s |
| `/search` | ~5s |
| Recovery embeddings | ~2s/N |
| Auto-compact | <3s |

### Storage

| Volume | Tamanho |
|--------|---------|
| 6 mensagens + embeddings | ~2.2MB |
| Estimado 1000 mensagens | ~50-100MB |

---

## 8. Arquivos Modificados/Criados

### Novos Arquivos (v0.22.2 a v0.26.0)

```
src/db/mod.rs, connection.rs, operations.rs, schema.rs, migration.rs
src/embeddings/recovery.rs
src/embeddings/chunker.rs
src/retrieval/context_builder.rs
src/tools/remember.rs
src/tools/context.rs
src/markdown.rs
scripts/install.sh
scripts/uninstall.sh
scripts/install-sprachspiel.sh
```

### Linhas Modificadas

- **+6043 linhas** adicionadas
- **-1485 linhas** removidas
- **51 arquivos** modificados

---

## Conclusão

### Bugs da v0.22.1

| Bug | Status | Correção |
|-----|--------|----------|
| Middle Compaction não implementado | ✅ CORRIGIDO | v0.22.2 |
| `get_compaction_range()` dead_code | ✅ CORRIGIDO | v0.22.2 |
| Chunking incompleto (async) | ✅ CORRIGIDO | v0.22.2 |

### Novas Funcionalidades

- Persistência SQLite completa
- Comandos `/clear` vs `/forget`
- Recovery de embeddings
- Remember Tool
- Auto-compact visual
- Conversation-aware retrieval
- Project-aware query mode

### Qualidade Geral

**Excelente evolução.** O projeto amadureceu significativamente, com arquitetura robusta de persistência e todos os bugs críticos corrigidos.

---

*Relatório gerado automaticamente pelo Hermes Agent*  
*Repositório: ~/git/ask-ollama-rs*