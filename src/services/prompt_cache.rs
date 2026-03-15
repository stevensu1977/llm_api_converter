//! Prompt cache injection service.
//!
//! Automatically injects `cache_control` breakpoints into Anthropic MessageRequests
//! to enable prompt caching and reduce repeated token costs.
//!
//! Anthropic supports up to 4 cache breakpoints per request. Strategy:
//! 1. System prompt (last block)
//! 2. Tools definition (last tool)
//! 3-4. Recent user messages (last content block of most recent user turns)

use crate::schemas::anthropic::{
    CacheControl, ContentBlock, Message, MessageContent, MessageRequest, SystemContent,
    SystemMessage,
};

const MAX_BREAKPOINTS: usize = 4;

/// Injects cache_control breakpoints into a MessageRequest in-place.
///
/// Does not overwrite existing cache_control values.
pub fn inject_cache_breakpoints(request: &mut MessageRequest) {
    let mut breakpoints_used = 0;

    // 1. System prompt — add cache_control to last system block
    if let Some(ref mut system) = request.system {
        breakpoints_used += inject_system_cache(system);
    }

    if breakpoints_used >= MAX_BREAKPOINTS {
        return;
    }

    // 2. Tools — add cache_control to last tool
    if let Some(ref mut tools) = request.tools {
        breakpoints_used += inject_tools_cache(tools);
    }

    if breakpoints_used >= MAX_BREAKPOINTS {
        return;
    }

    // 3-4. Recent user messages — last content block of most recent user turns
    let remaining = MAX_BREAKPOINTS - breakpoints_used;
    inject_user_message_cache(&mut request.messages, remaining);
}

/// Inject cache_control on the last system content block.
/// Returns 1 if a breakpoint was added, 0 otherwise.
fn inject_system_cache(system: &mut SystemContent) -> usize {
    match system {
        SystemContent::Text(text) => {
            // Convert to Messages variant so we can add cache_control
            let msg = SystemMessage {
                message_type: "text".to_string(),
                text: std::mem::take(text),
                cache_control: Some(CacheControl::new()),
            };
            *system = SystemContent::Messages(vec![msg]);
            1
        }
        SystemContent::Messages(messages) => {
            if let Some(last) = messages.last_mut() {
                if last.cache_control.is_none() {
                    last.cache_control = Some(CacheControl::new());
                    return 1;
                }
            }
            0
        }
    }
}

/// Inject cache_control on the last tool definition.
/// Tools are `Vec<serde_json::Value>` — we inject `cache_control` field on the last element.
/// Returns 1 if a breakpoint was added, 0 otherwise.
fn inject_tools_cache(tools: &mut [serde_json::Value]) -> usize {
    if let Some(last_tool) = tools.last_mut() {
        if let Some(obj) = last_tool.as_object_mut() {
            if !obj.contains_key("cache_control") {
                obj.insert(
                    "cache_control".to_string(),
                    serde_json::json!({"type": "ephemeral"}),
                );
                return 1;
            }
        }
    }
    0
}

/// Inject cache_control on the last content block of the most recent user messages.
fn inject_user_message_cache(messages: &mut [Message], max_injections: usize) {
    let mut injected = 0;

    // Iterate messages in reverse, find user messages
    for msg in messages.iter_mut().rev() {
        if injected >= max_injections {
            break;
        }
        if msg.role != "user" {
            continue;
        }

        match &mut msg.content {
            MessageContent::Text(text) => {
                // Convert to Blocks so we can add cache_control
                let block = ContentBlock::Text {
                    text: std::mem::take(text),
                    cache_control: Some(CacheControl::new()),
                };
                msg.content = MessageContent::Blocks(vec![block]);
                injected += 1;
            }
            MessageContent::Blocks(blocks) => {
                if let Some(last_block) = blocks.last_mut() {
                    if inject_block_cache(last_block) {
                        injected += 1;
                    }
                }
            }
        }
    }
}

/// Inject cache_control on a single content block. Returns true if injected.
fn inject_block_cache(block: &mut ContentBlock) -> bool {
    match block {
        ContentBlock::Text { cache_control, .. }
        | ContentBlock::Image { cache_control, .. }
        | ContentBlock::Document { cache_control, .. }
        | ContentBlock::ToolResult { cache_control, .. } => {
            if cache_control.is_none() {
                *cache_control = Some(CacheControl::new());
                return true;
            }
            false
        }
        // These block types don't have cache_control fields
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schemas::anthropic::{ImageSource, ToolResultValue};

    fn make_request(
        system: Option<SystemContent>,
        tools: Option<Vec<serde_json::Value>>,
        messages: Vec<Message>,
    ) -> MessageRequest {
        MessageRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            messages,
            max_tokens: 1024,
            system,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            stream: false,
            tools,
            tool_choice: None,
            thinking: None,
            metadata: None,
            container: None,
        }
    }

    #[test]
    fn test_empty_request() {
        let mut req = make_request(None, None, vec![Message::user("hi")]);
        inject_cache_breakpoints(&mut req);
        // Should add cache_control to the user message
        match &req.messages[0].content {
            MessageContent::Blocks(blocks) => match &blocks[0] {
                ContentBlock::Text { cache_control, .. } => {
                    assert!(cache_control.is_some());
                }
                _ => panic!("Expected text block"),
            },
            _ => panic!("Expected blocks"),
        }
    }

    #[test]
    fn test_system_text_gets_cache() {
        let mut req = make_request(
            Some(SystemContent::Text("You are helpful.".into())),
            None,
            vec![Message::user("hi")],
        );
        inject_cache_breakpoints(&mut req);

        match &req.system {
            Some(SystemContent::Messages(msgs)) => {
                assert_eq!(msgs.len(), 1);
                assert!(msgs[0].cache_control.is_some());
                assert_eq!(msgs[0].text, "You are helpful.");
            }
            _ => panic!("Expected system messages"),
        }
    }

    #[test]
    fn test_system_messages_get_cache_on_last() {
        let mut req = make_request(
            Some(SystemContent::Messages(vec![
                SystemMessage::new("First instruction"),
                SystemMessage::new("Second instruction"),
            ])),
            None,
            vec![Message::user("hi")],
        );
        inject_cache_breakpoints(&mut req);

        match &req.system {
            Some(SystemContent::Messages(msgs)) => {
                assert!(msgs[0].cache_control.is_none());
                assert!(msgs[1].cache_control.is_some());
            }
            _ => panic!("Expected system messages"),
        }
    }

    #[test]
    fn test_tools_get_cache_on_last() {
        let tools = vec![
            serde_json::json!({"name": "tool_a", "description": "A", "input_schema": {"type": "object"}}),
            serde_json::json!({"name": "tool_b", "description": "B", "input_schema": {"type": "object"}}),
        ];
        let mut req = make_request(None, Some(tools), vec![Message::user("hi")]);
        inject_cache_breakpoints(&mut req);

        let tools = req.tools.as_ref().unwrap();
        assert!(!tools[0].as_object().unwrap().contains_key("cache_control"));
        assert!(tools[1].as_object().unwrap().contains_key("cache_control"));
    }

    #[test]
    fn test_existing_cache_control_not_overwritten() {
        let mut req = make_request(
            Some(SystemContent::Messages(vec![SystemMessage {
                message_type: "text".to_string(),
                text: "System".into(),
                cache_control: Some(CacheControl::with_ttl("10m")),
            }])),
            None,
            vec![Message::user("hi")],
        );
        inject_cache_breakpoints(&mut req);

        match &req.system {
            Some(SystemContent::Messages(msgs)) => {
                let cc = msgs[0].cache_control.as_ref().unwrap();
                assert_eq!(cc.ttl, Some("10m".to_string()));
            }
            _ => panic!("Expected system messages"),
        }
    }

    #[test]
    fn test_max_4_breakpoints() {
        let mut req = make_request(
            Some(SystemContent::Text("System".into())),
            Some(vec![
                serde_json::json!({"name": "tool", "description": "T", "input_schema": {"type": "object"}}),
            ]),
            vec![
                Message::user("msg1"),
                Message::assistant("reply1"),
                Message::user("msg2"),
                Message::assistant("reply2"),
                Message::user("msg3"),
            ],
        );
        inject_cache_breakpoints(&mut req);

        // Count breakpoints: 1 system + 1 tool + 2 user messages = 4
        let mut count = 0;

        // System
        if let Some(SystemContent::Messages(msgs)) = &req.system {
            if msgs.last().map_or(false, |m| m.cache_control.is_some()) {
                count += 1;
            }
        }

        // Tools
        if let Some(tools) = &req.tools {
            if tools
                .last()
                .and_then(|t| t.as_object())
                .map_or(false, |o| o.contains_key("cache_control"))
            {
                count += 1;
            }
        }

        // User messages
        for msg in &req.messages {
            if msg.role == "user" {
                if let MessageContent::Blocks(blocks) = &msg.content {
                    if let Some(last) = blocks.last() {
                        match last {
                            ContentBlock::Text { cache_control, .. } => {
                                if cache_control.is_some() {
                                    count += 1;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        assert_eq!(count, 4);
    }

    #[test]
    fn test_user_messages_injected_in_reverse_order() {
        let mut req = make_request(
            None,
            None,
            vec![
                Message::user("old"),
                Message::assistant("reply"),
                Message::user("recent"),
            ],
        );
        inject_cache_breakpoints(&mut req);

        // Most recent user message (index 2) should get cache first
        let has_cache = |msg: &Message| -> bool {
            match &msg.content {
                MessageContent::Blocks(blocks) => blocks.last().map_or(false, |b| match b {
                    ContentBlock::Text { cache_control, .. } => cache_control.is_some(),
                    _ => false,
                }),
                _ => false,
            }
        };

        assert!(has_cache(&req.messages[2])); // "recent"
        assert!(has_cache(&req.messages[0])); // "old" - within 4 breakpoint limit
    }
}
