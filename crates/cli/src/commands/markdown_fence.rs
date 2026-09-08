//! Shared markdown fence scanning helper for plan-text checks.

pub(crate) fn fence_run_length(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let count = trimmed.bytes().take_while(|&b| b == b'`').count();
    if count >= 3 {
        Some(count)
    } else {
        None
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct MarkdownFence {
    pub(crate) open_fence_len: Option<usize>,
}

impl MarkdownFence {
    pub(crate) fn new() -> Self {
        Self {
            open_fence_len: None,
        }
    }

    pub(crate) fn feed(&mut self, line: &str) -> bool {
        if let Some(len) = fence_run_length(line) {
            match self.open_fence_len {
                None => {
                    self.open_fence_len = Some(len);
                    true
                }
                Some(open_len) if open_len == len => {
                    self.open_fence_len = None;
                    true
                }
                Some(_) => false,
            }
        } else {
            false
        }
    }

    pub(crate) fn is_in_fence(&self) -> bool {
        self.open_fence_len.is_some()
    }

    pub(crate) fn is_unterminated(&self) -> bool {
        self.open_fence_len.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_fence_flags_an_unterminated_fence() {
        let doc = "```\ncode block without closing fence\n";
        let mut fence = MarkdownFence::new();
        for line in doc.lines() {
            fence.feed(line);
        }
        assert!(
            fence.is_unterminated(),
            "markdown_fence flags an unterminated fence"
        );
    }

    #[test]
    fn markdown_fence_four_over_three_nesting() {
        let doc = "````\n```\ninner\n```\n````\n";
        let mut fence = MarkdownFence::new();
        for line in doc.lines() {
            fence.feed(line);
        }
        assert!(
            !fence.is_unterminated(),
            "nested fence document should be terminated"
        );
    }

    #[test]
    fn markdown_fence_well_formed_document_terminated() {
        let doc = "```\ncode\n```\n";
        let mut fence = MarkdownFence::new();
        for line in doc.lines() {
            fence.feed(line);
        }
        assert!(
            !fence.is_unterminated(),
            "well-formed document reports terminated"
        );
    }
}
