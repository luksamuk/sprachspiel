# Content Block Stateful Streaming — Arquitetura para Streaming + Tool Calls no TUI

## Resumo Executivo

O problema de **texto pré-tool sumindo durante tool calls no streaming** não é um bug localizado — é uma limitação fundamental da arquitetura atual de streaming. A solução requer uma mudança arquitetural de **"streaming único monolítico"** para **"content blocks com lifecycle próprio"**.

## Diagnóstico das Causas Raiz

### Causa Raiz 1: `process_response()` → `process_next()` mata o streaming

Em `custom_coordinator::chat_stream()` (linhas 653-805), o ciclo de streaming é:

1. OLLAMA envia chunks → `on_token()` acumula → `AssistantStreaming` é exibido
2. Tool calls detectados → `on_tool_call()` notifica
3. `process_response()` armazena conteúdo em `pre_tool_content`
4. Ferramentas executam síncrono no loop
5. `process_next()` faz **nova chamada OLLAMA (não-streaming)**
6. Retorna resultado como `ChatMessageResponse` final

O resultado de `process_next()` contém **APENAS o texto PÓS-tool**. O texto PRÉ-tool que foi exibido via streaming é **descartado do return value** e só existe no `pre_tool_content` (que é salvo no session).

```rust
custom_coordinator::chat_stream() {
    // PASSO 1: Streaming — tokens exibidos incrementalmente
    while let Some(chunk) = stream.next().await {
        on_token(chunk.content);        // ← VAI PARA A TELA (AssistantStreaming)
        full_content.push_str(&chunk.content);  // Acumula interno
    }
    
    // PASSO 2: Tool calls detectadas
    let response = ChatMessageResponse {
        content: full_content,       // ← CONTEÚDO PRÉ-TOOL!
        tool_calls: tool_calls.clone(),
        thinking: full_thinking,      // ← THINKING PRÉ-TOOL!
    };
    
    // PASSO 3: process_response salva pre_tool_content e executa tools
    self.process_response(response).await  // → process_next()
    
    // PASSO 4: process_next RETORNA resposta POST-tool (nova chamada OLLAMA)
    // O conteúdo pré-tool NÃO ESTÁ no return value!
}
```

### Causa Raiz 2: `StreamDone` recebe conteúdo errado

Em `core.rs::send_message_stream()` (linhas 829-844):

```rust
let _ = llm_tx.try_send(LlmEvent::StreamDone {
    content: cleaned_response.clone(),  // ← TEXTO PÓS-TOOL (de process_next())
    thinking: thinking.clone(),          // ← THINKING PÓS-TOOL
    metrics: None,
});
```

O `StreamDone` manda o conteúdo FINAL, que vem do `ChatMessageResponse` retornado por `process_next()`. Esse conteúdo NÃO inclui o texto pré-tool.

### Causa Raiz 3: `finalize_stream()` apaga o streaming

`finalize_stream()` (app.rs:442-469):
- Encontra `AssistantStreaming` na streaming zone
- Remove `Thinking` blocks da streaming zone
- Substitui `AssistantStreaming` por `Assistant` com texto de `StreamDone`

O texto de `StreamDone` é o TEXTO PÓS-TOOL. Então o `AssistantStreaming` que continha texto pré-tool é **sobrescrito** com texto pós-tool. Resultado: **texto pré-tool desaparece da tela**.

### Causa Raiz 4: "Respostas entre tool calls" não existem

O OLLAMA API suporta text-to-tool nas stream chunks, mas nossa implementação:
- Enquanto `chat_stream()` roda, chama `on_token()` para cada chunk
- Quando detecta tool calls, PARA de streamar e entra no modo síncrono
- `process_response()` executa ferramentas em um `for` loop síncrono
- `process_next()` faz uma ÚNICA nova chamada LLM

Não há mecanismo para o LLM gerar texto INTERMEDIÁRIO entre ferramentas durante uma mesma rodada. O texto entre ferramentas só apareceria se `process_next()` retornasse uma resposta interativa, mas ela bloqueia até completar.

## Estado da Arte (State of the Art)

| Projeto | Padrão | Como resolve pré-tool |
|---------|--------|---------------------|
| **assistant-ui** | Content Block Index Tracking | Cada bloco tem `index`. Bloco 0 = texto pré-tool (permanece). Bloco 1 = tool_use. Bloco 2 = texto pós-tool. Renders não sobrescrevem blocos concluídos. |
| **Claude Agent SDK** | `in_tool` flag | Flag previne novo texto durante tool call, mas NÃO preserva texto pré-tool. |
| **aichat** | Buffer flat | Texto acumulado no buffer, não diferenciado pré/pós. |
| **TUUI** | `lastItem` por índice | Tool calls organizadas por índice. Texto associado ao índice do bloco. |

O padrão **Content Block Index Tracking** da `assistant-ui` é o estado da arte e é o que vamos implementar.

## Arquitetura Proposta: Content Block Stateful Streaming

### Conceito Central

Cada "turno" do assistente consiste em múltiplos blocos de conteúdo, cada um com seu próprio lifecycle:

```
Turno do Assistant (rodada N):
  ├─ Block 0: "Vou buscar o clima..." ← streaming, depois FINALIZA
  ├─ Block 1: tool_use: weather        ← tool call
  ├─ Block 2: "São 22°C e sol"         ← tool result
  ├─ Block 3: tool_use: calc           ← tool call
  ├─ Block 4: "42 é a resposta"        ← tool result
  └─ Block 5: "Baseado no clima..."    ← novo streaming, FINALIZA
```

### Tipos de Blocos

```rust
// Enum de tipo de bloco para identificar o que cada bloco representa
#[derive(Debug, Clone, PartialEq)]
pub enum ContentBlockType {
    /// Texto sendo streamado pelo LLM
    Streaming,
    /// Texto finalizado (não mais streamando)
    Finalized,
    /// Chamada de ferramenta
    ToolCall { name: String, arguments: String },
    /// Resultado de ferramenta
    ToolResult { name: String, result: String, truncated: bool },
}
```

### Mensagens no App

Cada bloco é uma mensagem `ChatMessage` no vetor `App.messages`. Quando um bloco é "finalizado", ele permanece no vetor como uma mensagem `Assistant`. Apenas o bloco "ativo" (o último que está sendo streamado) pode ser modificado.

```rust
// Estado desejado após múltiplas tool calls:
App.messages = [
    ChatMessage { msg_type: User, content: "Quanto é 22 + 20?" },
    // Block 0 — Finalizado
    ChatMessage { msg_type: Assistant, content: "Vou calcular isso..." },
    ChatMessage { msg_type: Thinking, content: "Preciso somar 22 e 20" },
    // Block 1 — Tool call
    ChatMessage { msg_type: Tool, content: "🔧 calc(22, 20)" },
    // Block 2 — Tool result
    ChatMessage { msg_type: Tool, content: "Result: 42" },
    // Block 3 — Tool call (outra)
    ChatMessage { msg_type: Tool, content: "🔧 weather()" },
    // Block 4 — Tool result
    ChatMessage { msg_type: Tool, content: "Result: Sunny, 22°C" },
    // Block 5 — Finalizado
    ChatMessage { msg_type: Assistant, content: "22 + 20 = 42. O clima está ensolarado a 22°C." },
]
```

### Estado de Streaming

```rust
/// Estado do streaming no App
pub struct StreamingState {
    /// Índice do bloco ativo (o único que pode receber novos tokens)
    /// None = nenhum bloco ativo (idle)
    pub active_block_index: Option<usize>,
    /// Índice do bloco que será finalizado no próximo StreamBlockDone
    /// Diferente de active_block_index porque após finalizar, 
    /// o active muda para um novo bloco
    pub block_sequence: usize,
}
```

### Eventos LLM Atualizados

```rust
pub enum LlmEvent {
    // ... eventos existentes ...
    
    /// Streaming de token de conteúdo
    /// `block_index` identifica QUAL bloco recebe este token
    /// Se None, usa o bloco ativo atual
    StreamToken { 
        content: String,
        block_index: Option<usize>,  // Se None → bloco ativo
    },
    
    /// Streaming de token de thinking
    StreamThinking {
        content: String,
        block_index: Option<usize>,
    },
    
    /// Finaliza um bloco específico
    /// Quando chega durante tool calls, finaliza o bloco de texto pré-tool
    /// Quando chega no final da resposta, finaliza o último bloco
    StreamBlockDone {
        block_index: usize,
        content: String,
        thinking: Option<String>,
    },
    
    /// Inicia um novo bloco de texto
    /// Usado após tool call para começar streaming de novo texto
    StreamBlockStart {
        block_index: usize,
    },
    
    /// Indica que tool calls foram detectadas
    /// O event loop deveria:
    /// 1. Receber StreamBlockDone (finaliza bloco 0)
    /// 2. Exibir tool calls
    /// 3. Aguardar StreamBlockStart (bloco N) para continuar streaming
    ToolCallStarted,
    
    // ... resto dos eventos ...
}
```

### Ciclo de Vida Completo

```
[User envia mensagem]
→ App adiciona User message
→ LLM inicia streaming
→ StreamToken chega (block_index=0)
→ append_stream_token("Vou buscar...") → encontra bloco ativo ou cria
→ StreamThinking chega (block_index=0)
→ append_stream_thinking("Hmm...") → append no mesmo bloco

[Tool calls detectadas]
→ on_tool_call() → ToolCallStarted → enviado
→ chat_stream() PARA de enviar tokens
→ process_response() executa ferramentas
→ Após tool execution, envia StreamBlockDone { block_index: 0 }
	
[Event loop recebe]
→ ToolCallStarted → finalize_stream_block(0)
	→ AssistantStreaming(0) → Assistant(0) (PERMANECE no vetor)
	→ Thinking(0) → Thinking(0) (PERMANECE, consolidado)
→ ToolCall events → ToolCall(1) messages
→ ToolResult events → ToolResult(2) messages
→ StreamBlockStart { block_index: 3 } → novo AssistantStreaming(3)

[LLM continua]
→ StreamToken chega (block_index=3)
→ append_stream_token("São 22°C...") → append no bloco 3
→ StreamDone → finalize_stream_block(3) → Assistant(3) permanece

[Resultado final]
App.messages = [
	User("Quanto é 22+20?"),
	Assistant("Vou buscar..."),      // ← bloco 0, PRESERVADO
	Thinking("Hmm..."),             // ← thinking do bloco 0, PRESERVADO
	ToolCall("weather", "..."),       // ← bloco 1
	ToolResult("Sunny, 22°C"),        // ← bloco 2
	Assistant("São 22°C..."),        // ← bloco 3, FINAL
]
```

### Mudanças nos Métodos do App

```rust
impl App {
    /// Finaliza um bloco específico por índice
    /// Diferente do finalize_stream() atual que finaliza o ÚLTIMO streaming
    pub fn finalize_stream_block(&mut self, block_index: usize, content: &str, thinking: Option<&str>) {
        // Encontra o AssistantStreaming com block_sequence == block_index
        // Converte para Assistant com o conteúdo fornecido
        // O conteúdo pode ser diferente do que está no AssistantStreaming
        // (ex: pode ter thinking tags removidas, markdown aplicado)
        
        // Se não encontrar o bloco específico, procura o último AssistantStreaming
        // (fallback para compatibilidade com código antigo)
    }
    
    /// Cria um novo bloco de streaming
    /// Chamado quando StreamBlockStart chega
    pub fn start_new_stream_block(&mut self) -> usize {
        let block_id = self.block_sequence;
        self.block_sequence += 1;
        
        self.messages.push(ChatMessage::assistant_streaming(format!(
            "", block_id
        )));
        self.active_block_index = Some(self.messages.len() - 1);
        self.scroll.reset_to_bottom();
        
        block_id
    }
    
    /// Obtém o índice do bloco ativo atual
    /// Se nenhum bloco estiver ativo, cria um novo (compatibilidade)
    fn active_block_index(&self) -> Option<usize> {
        self.active_block_index
    }
}
```

### Mudanças no coordinator

```rust
impl CustomCoordinator {
    pub async fn chat_stream(
        &mut self,
        messages: Vec<ChatMessage>,
        on_token: impl Fn(String) + Send + Sync,
        on_thinking: impl Fn(String) + Send + Sync,
        on_tool_call: impl Fn() + Send + Sync,
        // NOVO: callback para sinalizar fim de bloco
        on_block_done: impl Fn(usize) + Send + Sync,
        // NOVO: callback para iniciar novo bloco
        on_block_start: impl Fn(usize) + Send + Sync,
        cancel_token: Option<CancellationToken>,
    ) -> Result<ChatMessageResponse> {
        let mut block_index: usize = 0;
        
        // ... stream setup ...
        
        while let Some(chunk) = stream.next().await {
            // ... process chunk ...
            
            if !chunk.message.tool_calls.is_empty() {
                // Tool calls detectadas!
                // 1. Sinalizar fim do bloco atual
                on_block_done(block_index);
                
                // 2. Notificar tool call
                on_tool_call();
                
                // 3. Processar response (tools)
                let response = self.process_response(resp).await;
                
                // 4. Se process_next() retornar mais tool calls, repetir
                // 5. Quando retornar texto final:
                block_index += 1;
                on_block_start(block_index);
                
                // 6. Continuar streaming do novo bloco
                // (process_next() não é streaming, mas o texto retornado
                // pode ser exibido como um bloco completo)
                // 
                // ALTERNATIVA: Se quisermos streaming do pós-tool:
                // Usar o accumulated content do process_next() como StreamToken
                // do novo block_index
            }
        }
        
        // Done
        on_block_done(block_index);
    }
}
```

### Mudanças no Event Loop

```rust
// repl_tui.rs
match llm_event {
    LlmEvent::StreamToken { content, block_index } => {
        let idx = block_index.unwrap_or_else(|| app.active_block_index());
        app.append_stream_token_to_block(idx, &content);
    }
    
    LlmEvent::StreamBlockDone { block_index, content, thinking } => {
        app.finalize_stream_block(block_index, &content, thinking.as_deref());
        app.active_block_index = None; // Nenhum bloco ativo
    }
    
    LlmEvent::StreamBlockStart { block_index } => {
        app.start_new_stream_block();
        // O block_index aqui é informativo — o app atribui o próximo
    }
    
    LlmEvent::ToolCallStarted => {
        view.set_llm_state(LlmState::ToolCall);
        // O bloco atual será finalizado por StreamBlockDone
    }
    
    // ... outros eventos ...
}
```

## Estimativa de Implementação

### Fases

| Fase | Descrição | Arquivos | Esforço |
|------|-----------|----------|---------|
| 1 | Refatorar `ChatMessage` para ter `block_id: Option<usize>` | `chat_area.rs` | ~1h |
| 2 | Adicionar `StreamingState` ao `App` | `app.rs` | ~1h |
| 3 | Implementar `finalize_stream_block()`, `start_new_stream_block()`, `append_stream_token_to_block()` | `app.rs` | ~2h |
| 4 | Atualizar `LlmEvent` com `block_index` campos | `llm_event.rs` | ~30min |
| 5 | Atualizar `chat_stream()` para emitir `StreamBlockDone`/`StreamBlockStart` | `custom_coordinator.rs` | ~2h |
| 6 | Atualizar event loop para tratar novos eventos | `repl_tui.rs` | ~1h |
| 7 | Atualizar `StreamDone` no `send_message_stream()` | `core.rs` | ~30min |
| 8 | Escrever testes unitários | `app.rs` | ~2h |
| 9 | `cargo test`, `cargo clippy`, `cargo fmt` | — | ~30min |

**Total estimado**: ~10 horas (~2 dias)

## Decisões de Design (ADRs)

### ADR-1: ChatMessage.block_id é Option<usize>

Nem todas as mensagens têm um bloco. User, System, Error, Banner não são partes de blocos do assistente. Só Assistant, AssistantStreaming, Thinking E ToolCall/ToolResult estão dentro de blocos.

**Alternativa considerada**: Ter um enum separado para blocos, mas isso complexificaria o ChatMessage existente.

### ADR-2: Não usar índice explícito no OLLAMA chunks

A API OLLAMA não envia content_block_start/stop (a API da Anthropic sim). Nosso `block_index` é atribuído pelo coordinator com base no momento (quando tools são detectadas, incrementa).

**Alternativa**: Fazer o OllamaProvider parser identificar transições. Rejeitado porque OLLAMA não tem content block index nativo.

### ADR-3: finalize_stream_block recebe conteúdo em vez de usar o acumulado

Porque o conteúdo final pode ter thinking_tags removidos, markdown processado, etc. O Assistant Streaming é plain text; o Assistant finalizado é markdown-rendered.

## Testes a Implementar

```rust
#[test]
fn test_content_block_lifecycle_single_block() {
    // Um bloco, sem tool calls
    let mut app = test_app();
    app.start_new_stream_block(); // block 0
    app.append_stream_token_to_block(0, "Hello");
    app.append_stream_token_to_block(0, " world");
    app.finalize_stream_block(0, "Hello world", None);
    
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].msg_type, Assistant);
    assert_eq!(app.messages[0].content, "Hello world");
}

#[test]
fn test_content_block_lifecycle_interrupted_by_tool() {
    // Bloco 0 → tool call → Bloco 1
    let mut app = test_app();
    app.start_new_stream_block(); // block 0
    app.append_stream_token_to_block(0, "Let me search");
    app.append_stream_thinking_to_block(0, "I should use weather");
    app.finalize_stream_block(0, "Let me search", Some("I should use weather"));
    
    // Tool call/result (não são blocos, são mensagens Tool)
    app.add_message(ChatMessage::tool("weather: Sunny, 22°C"));
    
    app.start_new_stream_block(); // block 1
    app.append_stream_token_to_block(1, "It is 22°C");
    app.finalize_stream_block(1, "It is 22°C and sunny", None);
    
    // Verificar: Bloco 0 PRESERVADO, Bloco 1 PRESERVADO
    assert_eq!(app.messages.len(), 4); // Assistant(0), Thinking(0), Tool, Assistant(1)
    assert_eq!(app.messages[0].content, "Let me search");
    assert_eq!(app.messages[1].content, "I should use weather");
    assert_eq!(app.messages[3].content, "It is 22°C and sunny");
}

#[test]
fn test_content_block_multiple_tool_calls() {
    // Bloco 0 → tool 1 → Bloco 1 → tool 2 → Bloco 2
    let mut app = test_app();
    
    // Bloco 0
    app.start_new_stream_block();
    app.append_stream_token_to_block(0, "Vou buscar...");
    app.finalize_stream_block(0, "Vou buscar...", None);
    
    // Tool 1
    app.add_message(ChatMessage::tool("weather: Sunny"));
    
    // Bloco 1
    app.start_new_stream_block();
    app.append_stream_token_to_block(1, "Agora calcular...");
    app.finalize_stream_block(1, "Agora calcular...", None);
    
    // Tool 2
    app.add_message(ChatMessage::tool("calc: 42"));
    
    // Bloco 2
    app.start_new_stream_block();
    app.append_stream_token_to_block(2, "Pronto! Está 22°C e 42");
    app.finalize_stream_block(2, "Pronto! Está 22°C e 42", None);
    
    assert_eq!(app.messages.len(), 7);
    // Cada bloco preservado
}
```

## Checklist de Implementação

- [ ] Fase 1: Adicionar `block_id: Option<usize>` a `ChatMessage`
- [ ] Fase 2: Adicionar `StreamingState` ao `App`
- [ ] Fase 3: Refatorar `finalize_stream()` → `finalize_stream_block()`
- [ ] Fase 4: Adicionar `start_new_stream_block()` e mètodos auxiliares
- [ ] Fase 5: Atualizar `LlmEvent` variantes com `block_index`
- [ ] Fase 6: Atualizar `chat_stream()` para emitir novos eventos
- [ ] Fase 7: Atualizar event loop em `repl_tui.rs`
- [ ] Fase 8: Atualizar `send_message_stream()` em `core.rs`
- [ ] Fase 9: Escrever testes
- [ ] Fase 10: `cargo test`, `cargo clippy`, `cargo fmt`
- [ ] Fase 11: Atualizar CHANGELOG
- [ ] Fase 12: Atualizar IMPLEMENTATION.md e PR body

## Riscos

| Risco | Prob. | Impacto | Mitigação |
|-------|-------|---------|-----------|
| Mudança grande em `LlmEvent` quebra compatibilidade com branches paralelos | Média | Alto | Mudanças são aditivas (novos campos + novos métodos). Código antigo que não usa block_index continua funcionando via fallback. |
| OLLAMA API não tem content blocks nativamente | Baixa | Alto | Nosso block_index é semântico (coordinator atribui), não depende da API. |
| Testes existentes quebram com refactoring | Média | Médio | Todos os testes existentes devem passar. Onde `finalize_stream()` é chamado, usar `finalize_stream_block(active_block.unwrap(), ...)` com valor default. |
| Regressão no TerminalView (não-TUI) | Baixa | Alto | O path não-streaming (`send_message()`) não usa LlmEvent, então não é afetado. |