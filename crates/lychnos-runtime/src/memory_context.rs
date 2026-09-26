use std::{
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};

use lychnos_core::{
    interaction::{ConversationContext, ConversationMemoryContext, ConversationRequest},
    memory::{
        MemoryContent, MemoryId, MemoryKind, MemoryProvenance, MemoryRecord, MemoryScope,
        MemorySensitivity, MemoryStore, MemorySyncMetadata, MemoryTimestamp, MemoryValue,
    },
};

use crate::memory_store::JsonFileMemoryStore;

const MAX_RECALLED_MEMORIES: usize = 5;
const MAX_RECALLED_TOTAL_CHARS: usize = 1_500;
const MAX_SINGLE_MEMORY_CHARS: usize = 500;

pub fn prepare_conversation_context(
    store: Option<&mut JsonFileMemoryStore>,
    request: &ConversationRequest,
) -> Result<ConversationContext, String> {
    let mut context = ConversationContext::default();

    let Some(store) = store else {
        if explicit_memory_text(&request.text).is_some() {
            context.runtime_notices.push(
                "Persistent Lychnos memory is unavailable, so the explicit memory request was not saved. Do not claim that it was saved."
                    .into(),
            );
        }
        return Ok(context);
    };

    if let Some(text) = explicit_memory_text(&request.text) {
        if looks_like_secret(text) {
            context.runtime_notices.push(
                "The explicit memory request was not saved because it appears to contain a credential or secret, and encrypted-at-rest memory is not available yet. Do not claim that it was saved."
                    .into(),
            );
        } else if text.is_empty() {
            context
                .runtime_notices
                .push("The memory request contained no text to save.".into());
        } else {
            let already_stored = store.list()?.iter().any(|record| {
                !record.sync.tombstone
                    && record.scope.as_str() == "identity"
                    && record.sensitivity == MemorySensitivity::Standard
                    && matches!(
                        record.content.get("text"),
                        Some(MemoryValue::Text(existing))
                            if existing.trim().eq_ignore_ascii_case(text.trim())
                    )
            });

            if already_stored {
                context.runtime_notices.push(
                    "That explicit memory is already stored locally in Lychnos memory; no duplicate record was created."
                        .into(),
                );
            } else {
                let record = explicit_user_memory(request, text)?;
                let memory_id = record.id.as_str().to_string();
                store.upsert(record)?;
                context.runtime_notices.push(format!(
                    "The user's explicit memory request was saved locally in Lychnos memory as {memory_id}."
                ));
            }
        }
    }

    let records = store.list()?;
    context.memories = select_relevant_memories(&records, &request.text);

    Ok(context)
}

fn explicit_user_memory(request: &ConversationRequest, text: &str) -> Result<MemoryRecord, String> {
    let now = unix_millis()?;
    let id = MemoryId::new(format!("memory-{}-{}", std::process::id(), unique_nanos()?));

    Ok(MemoryRecord::new(
        id,
        MemoryKind::new("user.explicit"),
        MemoryTimestamp::from_unix_millis(now),
        MemoryProvenance::new("conversation.explicit_memory"),
        MemoryScope::new("identity"),
        MemorySensitivity::Standard,
    )
    .with_content(
        MemoryContent::new()
            .with_field("text", MemoryValue::Text(text.trim().to_string()))
            .with_field(
                "source_interaction_id",
                MemoryValue::Text(request.id.as_str().to_string()),
            ),
    )
    .with_sync_metadata(MemorySyncMetadata {
        revision: 1,
        tombstone: false,
    }))
}

fn select_relevant_memories(
    records: &[MemoryRecord],
    query: &str,
) -> Vec<ConversationMemoryContext> {
    let recall_all = is_memory_recall_query(query);
    let query_tokens = content_tokens(query);
    let mut ranked = Vec::new();

    for record in records {
        if record.sync.tombstone
            || record.sensitivity != MemorySensitivity::Standard
            || record.scope.as_str() != "identity"
        {
            continue;
        }

        let Some(MemoryValue::Text(text)) = record.content.get("text") else {
            continue;
        };

        let memory_tokens = content_tokens(text);
        let overlap = query_tokens.intersection(&memory_tokens).count();

        if !recall_all && overlap == 0 {
            continue;
        }

        ranked.push((
            if recall_all { 1 } else { overlap },
            record.updated_at.as_unix_millis(),
            record.id.as_str().to_string(),
            record.kind.as_str().to_string(),
            text.clone(),
        ));
    }

    ranked.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| left.2.cmp(&right.2))
    });

    let mut total_chars = 0;
    let mut selected = Vec::new();

    for (_, _, memory_id, kind, text) in ranked {
        if selected.len() >= MAX_RECALLED_MEMORIES || total_chars >= MAX_RECALLED_TOTAL_CHARS {
            break;
        }

        let remaining = MAX_RECALLED_TOTAL_CHARS - total_chars;
        let max_chars = remaining.min(MAX_SINGLE_MEMORY_CHARS);
        let text = truncate_chars(&text, max_chars);
        if text.is_empty() {
            continue;
        }

        total_chars += text.chars().count();
        selected.push(ConversationMemoryContext {
            memory_id,
            kind,
            text,
        });
    }

    selected
}

fn explicit_memory_text(input: &str) -> Option<&str> {
    let trimmed = input.trim();
    let lower = trimmed.to_ascii_lowercase();

    const PREFIXES: [&str; 5] = [
        "please remember that ",
        "please remember: ",
        "remember this: ",
        "remember that ",
        "remember: ",
    ];

    PREFIXES
        .iter()
        .find(|prefix| lower.starts_with(**prefix))
        .map(|prefix| trimmed[prefix.len()..].trim())
}

fn is_memory_recall_query(input: &str) -> bool {
    let lower = input.trim().to_ascii_lowercase();
    [
        "what do you remember",
        "what have you remembered",
        "what do you know about me",
        "show me what you remember",
        "list what you remember",
        "remember about me",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn looks_like_secret(input: &str) -> bool {
    let lower = input.to_ascii_lowercase();
    [
        "password",
        "passcode",
        "api key",
        "secret key",
        "private key",
        "recovery phrase",
        "seed phrase",
        "mnemonic phrase",
        "one-time code",
        "2fa code",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn content_tokens(input: &str) -> BTreeSet<String> {
    const STOPWORDS: [&str; 27] = [
        "about", "also", "and", "are", "but", "can", "did", "does", "for", "from", "have", "how",
        "into", "just", "me", "remember", "that", "the", "this", "was", "what", "when", "where",
        "who", "with", "you", "your",
    ];

    input
        .split(|character: char| !character.is_alphanumeric())
        .filter_map(|token| {
            let token = token.trim().to_lowercase();
            if token.len() < 3 || STOPWORDS.contains(&token.as_str()) {
                None
            } else {
                Some(token)
            }
        })
        .collect()
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.trim().to_string();
    }

    let mut truncated: String = input.chars().take(max_chars.saturating_sub(1)).collect();
    truncated.push('…');
    truncated.trim().to_string()
}

fn unix_millis() -> Result<u64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_millis();
    u64::try_from(millis).map_err(|_| "current time does not fit in u64 milliseconds".to_string())
}

fn unique_nanos() -> Result<u128, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_nanos())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lychnos_core::{
        interaction::{InteractionId, InteractionSource},
        memory::MemoryConfidence,
    };

    fn request(text: &str) -> ConversationRequest {
        ConversationRequest::new(
            InteractionId::new("interaction-1"),
            InteractionSource::Typed,
            text,
        )
    }

    fn memory(
        id: &str,
        text: &str,
        sensitivity: MemorySensitivity,
        tombstone: bool,
    ) -> MemoryRecord {
        MemoryRecord::new(
            MemoryId::new(id),
            MemoryKind::new("user.explicit"),
            MemoryTimestamp::from_unix_millis(100),
            MemoryProvenance::new("test"),
            MemoryScope::new("identity"),
            sensitivity,
        )
        .with_content(MemoryContent::new().with_field("text", MemoryValue::Text(text.into())))
        .with_confidence(MemoryConfidence::from_percent(100).expect("valid confidence"))
        .with_sync_metadata(MemorySyncMetadata {
            revision: 1,
            tombstone,
        })
    }

    #[test]
    fn explicit_memory_requires_an_unambiguous_prefix() {
        assert_eq!(
            explicit_memory_text("Remember that I prefer dark mode."),
            Some("I prefer dark mode.")
        );
        assert_eq!(
            explicit_memory_text("please remember: tea, no sugar"),
            Some("tea, no sugar")
        );
        assert_eq!(explicit_memory_text("Do you remember that thing?"), None);
        assert_eq!(explicit_memory_text("I remember that day."), None);
    }

    #[test]
    fn obvious_credentials_are_not_memory_candidates() {
        assert!(looks_like_secret("my password is swordfish"));
        assert!(looks_like_secret("API key ABC"));
        assert!(!looks_like_secret("I prefer dark mode"));
    }

    #[test]
    fn relevance_filter_excludes_sensitive_tombstoned_and_unrelated_records() {
        let records = vec![
            memory(
                "dark",
                "I prefer dark mode for desktop apps.",
                MemorySensitivity::Standard,
                false,
            ),
            memory(
                "secret",
                "My dark mode account password is secret.",
                MemorySensitivity::Restricted,
                false,
            ),
            memory(
                "old",
                "I used to prefer dark mode.",
                MemorySensitivity::Standard,
                true,
            ),
            memory(
                "bike",
                "I like touring motorcycles.",
                MemorySensitivity::Standard,
                false,
            ),
        ];

        let selected = select_relevant_memories(&records, "Which dark mode do I prefer?");
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].memory_id, "dark");
    }

    #[test]
    fn broad_memory_question_returns_bounded_standard_identity_memories() {
        let records: Vec<_> = (0..8)
            .map(|index| {
                memory(
                    &format!("memory-{index}"),
                    &format!("Preference number {index}"),
                    MemorySensitivity::Standard,
                    false,
                )
                .with_updated_at(MemoryTimestamp::from_unix_millis(index))
            })
            .collect();

        let selected = select_relevant_memories(&records, "What do you remember about me?");
        assert_eq!(selected.len(), MAX_RECALLED_MEMORIES);
        assert_eq!(selected[0].memory_id, "memory-7");
    }

    #[test]
    fn repeated_explicit_memory_does_not_create_duplicates() {
        let path = std::env::temp_dir().join(format!(
            "lychnos-memory-context-dedupe-{}-{}.json",
            std::process::id(),
            unique_nanos().expect("clock should work")
        ));
        let mut store = JsonFileMemoryStore::open(&path).expect("store should open");
        let request = request("Remember that I prefer dark mode.");

        prepare_conversation_context(Some(&mut store), &request)
            .expect("first context should assemble");
        let second = prepare_conversation_context(Some(&mut store), &request)
            .expect("second context should assemble");

        assert_eq!(store.len(), 1);
        assert!(
            second
                .runtime_notices
                .iter()
                .any(|notice| notice.contains("no duplicate record"))
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn explicit_memory_is_persisted_and_recalled() {
        let path = std::env::temp_dir().join(format!(
            "lychnos-memory-context-test-{}-{}.json",
            std::process::id(),
            unique_nanos().expect("clock should work")
        ));
        let mut store = JsonFileMemoryStore::open(&path).expect("store should open");

        let context = prepare_conversation_context(
            Some(&mut store),
            &request("Remember that I prefer dark mode."),
        )
        .expect("context should assemble");

        assert_eq!(store.len(), 1);
        assert_eq!(context.memories.len(), 1);
        assert!(context.runtime_notices[0].contains("saved locally"));

        drop(store);
        let reopened = JsonFileMemoryStore::open(&path).expect("store should reopen");
        assert_eq!(reopened.len(), 1);

        let _ = std::fs::remove_file(path);
    }
}
