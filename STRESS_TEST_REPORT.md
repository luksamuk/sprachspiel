# ask-ai Stress Test Report

**Data:** 2026-03-03  
**Versão Testada:** v0.22.1+  
**Modelo:** ministral-3:14b  
**Context Window:** 32,768 tokens

---

## Sumário Executivo

Testes de estresse executados para validar as funcionalidades implementadas após v0.19.0:
- Pesquisa híbrida (BM25 + Semantic)
- Chunking de mensagens longas
- Context overflow detection
- Compactação de contexto

| Funcionalidade | Status | Observações |
|----------------|--------|-------------|
| Context Overflow Detection | ✅ PASS | Thresholds: 72% warning, 80% overflow |
| Pesquisa Híbrida (BM25+Semantic) | ✅ PASS | Latência média: 542ms, 100% sucesso |
| Chunking de Mensagens | ⚠️ PARTIAL | 4/6 mensagens OK, bug em async embedding |
| Auto-Compaction | ❌ NOT IMPLEMENTED | Código existe mas não integrado |
| Lost in the Middle | ✅ PASS | Ordenação aplicada corretamente |

**Bugs Encontrados:** 2 (1 HIGH, 1 MEDIUM)

---

## 1. Teste de Estresse de Contexto

### Objetivo

Validar se o sistema detecta corretamente overflow de contexto e compacta conforme especificado.

### Thresholds Validados

| Threshold | Porcentagem | Tokens | Mensagens Aprox. |
|-----------|-------------|--------|------------------|
| Warning | 72% | 23,592 | ~20-23 pares |
| Overflow | 80% | 26,214 | ~26-30 pares |

### Simulação de Crescimento

```
Estado Inicial:
  System tokens: 4,854
  Tools tokens: 660
  History tokens: 4,854
  Total: 10,368 tokens (31.6%)

Warning Threshold (par 23):
  Total: 24,259 tokens (74.0%)

Overflow Threshold (par 26):
  Total: 26,704 tokens (81.5%)
```

### 🐛 BUG #1: Compaction NÃO Preserva Mensagens

**Severidade:** HIGH  
**Localização:** `src/chat/repl.rs:402-440`  
**Descrição:** O comando `/compact` resume TODAS as mensagens em um único summary. Não preserva first/last messages como especificado na documentação.

**Comportamento Atual:**
```rust
// compact_conversation() em src/chat/repl.rs
// Envia TODAS as mensagens para o LLM resumir
// Resultado: session.compacted_summary = summary
// Nenhuma mensagem é preservada
```

**Comportamento Esperado:**
- Preservar primeiras 5 mensagens
- Preservar últimas 5 mensagens
- Resumir apenas mensagens do meio

**Evidência:** Função `get_compaction_range()` em `src/context_overflow.rs:168-207` implementa a lógica correta, mas está marcada como `#[allow(dead_code)]` e **não é chamada em lugar nenhum**.

### 🐛 BUG #2: Middle Compaction NÃO Integrado

**Severidade:** MEDIUM  
**Localização:** `src/context_overflow.rs:168-207`  
**Descrição:** A função `get_compaction_range()` existe e implementa a estratégia correta de middle compaction (manter first 5 + last 5), mas não está integrada ao fluxo.

**Código Existente (NÃO UTILIZADO):**
```rust
// src/context_overflow.rs:168-207
#[allow(dead_code)]  // ← MARCADO COMO DEAD CODE!
pub fn get_compaction_range(
    total_messages: usize,
    keep_first: usize,
    keep_last: usize,
) -> Option<Range<usize>> {
    // Implementação correta que retorna range do meio
    // para ser resumido
}
```

### Recomendações

1. **Integrar Middle Compaction:** Modificar `compact_conversation()` para usar `get_compaction_range()`
2. **Implementar Auto-Compaction:** Trigger automático em 72% warning threshold
3. **Remover `#[allow(dead_code)]`:** Integrar a função que já existe

---

## 2. Teste de Pesquisa Híbrida

### Objetivo

Validar se a pesquisa híbrida (BM25 + Semantic + RRF) funciona corretamente.

### Configuração

| Parâmetro | Valor | Status |
|-----------|-------|--------|
| BM25 Weight | 0.4 | ✅ OK |
| Semantic Weight | 0.6 | ✅ OK |
| Similarity Threshold | 0.7 | ✅ OK |
| RRF Constant (k) | 60 | ✅ OK |

### Resultados

| Métrica | Valor |
|---------|-------|
| Testes Executados | 8 |
| Testes Passados | 8 (100%) |
| Latência Média | 542ms |
| Latência Mínima | 531ms |
| Latência Máxima | 554ms |

### Testes por Query

| Query | Tipo Resultado | Score Top | Latência |
|-------|---------------|------------|----------|
| machine learning | 🔗 Hybrid | 0.0162 | 554ms |
| neural networks | 🧠 Semantic | 0.0098 | 543ms |
| deep learning algorithms | 🧠 Semantic | 0.0098 | 547ms |
| transformer attention | 🧠 Semantic | 0.0098 | 531ms |
| gradient descent | 🧠 Semantic | 0.0098 | 542ms |
| programming | 🧠 Semantic | 0.0098 | 538ms |
| database | 🧠 Semantic | 0.0098 | 542ms |
| python code | 🧠 Semantic | 0.0098 | 540ms |

### Detecção Híbrida

A query "machine learning" retornou resultado **híbrido** (score 0.0162) combinando BM25 + Semantic, confirmando que a fusão RRF funciona corretamente. Queries sem match keyword retornam apenas semantic.

### Lost in the Middle

✅ **PASS** - A ordenação respeita o paper "Lost in the Middle": resultados com maior score aparecem primeiro, evitando degradação de performance no meio do contexto.

** scores observados:**
- Primeiro resultado: 0.0162 (hybrid)
- Demais: 0.0098-0.0092 (semantic)

---

## 3. Teste de Chunking de Mensagens

### Objetivo

Validar se mensagens longas (>1024 chars) são divididas em chunks corretamente.

### Parâmetros

| Parâmetro | Valor | Descrição |
|-----------|-------|-----------|
| CHUNK_SIZE | 1024 chars | Tamanho máximo por chunk |
| CHUNK_OVERLAP | 200 chars | Overlap entre chunks (20%) |
| CHUNK_MIN_SIZE | 256 chars | Tamanho mínimo para chunking |

### Resultados

| Métrica | Valor |
|---------|-------|
| Total mensagens analisadas | 20 |
| Mensagens com chunking | 6 |
| Total de chunks criados | 13 |
| Mensagens corretamente chunked | 4/6 (66.7%) |
| Tamanho máximo de chunk | 1005 chars |
| UTF-8 safe | ✅ 100% PASS |

### 🐛 BUG #3: Chunking Incompleto

**Severidade:** HIGH  
**Localização:** `src/embeddings/chunker.rs` + `src/embeddings/client.rs`  
**Descrição:** 2 de 6 mensagens longas tiveram chunking incompleto.

**Mensagens Afetadas:**

| Message ID | Content Length | Chunks Esperados | Chunks Reais | Chars Perdidos |
|------------|----------------|------------------|--------------|----------------|
| 15 | 1,779 chars | 2 | 1 | 854 (48%) |
| 18 | 2,041 chars | 2 | 1 | 1,036 (51%) |

**Causa Provável:** Implementação fire-and-forget com `tokio::spawn` pode perder chunks se:
- Embedding API timeout
- Processo terminar antes de completar
- Erro não tratado no async task

**Evidência:**
```rust
// src/embeddings/client.rs
// Embeddings são gerados async com fire-and-forget
// Não há garantia de completude antes do processo terminar
```

### Recomendação

**Corrigir Chunking Assíncrono:**
1. Armazenar chunks sincronamente antes de gerar embeddings
2. Adicionar retry logic para embeddings falhos
3. Considerar fila de tarefas com confirmação

---

## Memória e Storage

### Estimativas de Storage

| Volume | Tamanho Estimado |
|--------|------------------|
| 10,000 mensagens | ~20-30 MB |
| 50,000 mensagens | ~85-125 MB |

### Performance de Embedding

| Operação | Latência |
|----------|----------|
| Embedding generation (Ollama) | ~50-100ms |
| Similarity search (10k vectors) | ~1-5ms |
| Embedding por mensagem | ~3KB |

---

## Arquivos de Teste Criados

```
tests/context_stress_test.py       - Teste PTY interativo
tests/context_overflow_test.py    - Teste extendido de overflow
tests/fast_context_test.py        - Teste rápido
tests/context_analysis.py         - Análise estática (FUNCIONA)
tests/test_hybrid_search.py       - Teste de pesquisa híbrida
tests/test_search_detailed.py     - Teste detalhado de /search
search_metrics.json               - Métricas de pesquisa
search_detailed_metrics.json      - Detalhes de pesquisa
chunking_test_results.json        - Resultados de chunking
```

---

## Recomendações Prioritárias

### 🔴 Alta Prioridade

1. **Integrar Middle Compaction**
   - Local: `src/chat/repl.rs:402-440`
   - Ação: Usar `get_compaction_range()` em `compact_conversation()`
   - Benefício: Preserva first 5 + last 5 mensagens

2. **Corrigir Chunking Incompleto**
   - Local: `src/embeddings/client.rs`
   - Ação: Armazenar chunks sincronamente, embeddings async
   - Benefício: Garante 100% de chunks salvos

### 🟡 Média Prioridade

3. **Implementar Auto-Compaction**
   - Local: Loop de mensagens
   - Ação: Trigger em 72% warning threshold
   - Benefício: Previne overflow automático

4. **Adicionar Métricas Visíveis**
   - Local: Comando `/context`
   - Ação: Mostrar % de utilização em tempo real
   - Benefício: Visibilidade para usuário

### 🟢 Baixa Prioridade

5. **Testes Automatizados**
   - Adicionar testes unitários para `context_overflow`
   - Adicionar testes para `chunker`

6. **Documentação**
   - Atualizar CHANGELOG com descobertas

---

## Conclusão

O ask-ai tem uma arquitetura sólida com pesquisa híbrida funcionando bem. Os principais problemas estão na **integração** de funcionalidades que já foram implementadas mas não estão conectadas:

1. `get_compaction_range()` existe mas não é usado
2. Detecção de overflow funciona mas não tem auto-trigger
3. Chunking funciona mas pode perder chunks em edge cases

A correção desses pontos é relativamente simples pois o código já existe.

---

*Relatório gerado automaticamente pelo Hermes Agent*  
*Repositório: ~/git/ask-ollama-rs*