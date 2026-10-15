use crate::helper::{char_width, split_str_at};
use ratatui_core::text::Span;
use std::borrow::Cow;

#[derive(Default)]
pub(crate) struct LineWrapper;

impl LineWrapper {
    /// Splits a given line width into multiple smaller widths, ensuring each width
    /// is no larger than the specified maximum width.
    pub(crate) fn determine_split(line_width: usize, max_width: usize) -> Vec<usize> {
        if line_width == 0 {
            return vec![0];
        }

        let mut remaining_width = line_width;
        let mut split_widths = Vec::new();

        while remaining_width > 0 {
            let current_chunk = std::cmp::min(remaining_width, max_width);
            split_widths.push(current_chunk);
            remaining_width = remaining_width.saturating_sub(max_width);
        }

        split_widths
    }

    /// Number of characters on each wrapped row of `line`. Rows break
    /// after the last space that fits; words longer than a row are split.
    /// Every character is kept, so cursor positions map one to one.
    pub(crate) fn split_lengths(line: &[char], max_width: usize, tab_width: usize) -> Vec<usize> {
        if line.is_empty() || max_width == 0 {
            return vec![line.len()];
        }
        let mut out = Vec::new();
        let mut start = 0;
        while start < line.len() {
            let mut width = 0;
            let mut end = start;
            let mut last_space = None;
            while end < line.len() {
                let w = char_width(line[end], tab_width);
                if width + w > max_width {
                    break;
                }
                width += w;
                if line[end] == ' ' {
                    last_space = Some(end);
                }
                end += 1;
            }
            if end >= line.len() {
                out.push(line.len() - start);
                break;
            }
            let cut = match last_space {
                // The next character is a space: keep it at the end of this
                // row (invisible there) so the next row starts with a word.
                _ if line[end] == ' ' => end + 1,
                Some(s) if s > start => s + 1,
                _ => end.max(start + 1),
            };
            out.push(cut - start);
            start = cut;
        }
        out
    }

    pub(crate) fn wrap_line(line: &[char], max_width: usize, tab_width: usize) -> Vec<Vec<char>> {
        let mut out = Vec::new();
        let mut start = 0;
        for len in Self::split_lengths(line, max_width, tab_width) {
            if len == 0 {
                continue;
            }
            out.push(line[start..start + len].to_vec());
            start += len;
        }
        out
    }

    pub(crate) fn wrap_spans(
        spans: Vec<Span<'_>>,
        max_width: usize,
        tab_width: usize,
    ) -> Vec<Vec<Span<'_>>> {
        // Each character with the index of the span it came from.
        let chars: Vec<(char, usize)> = spans
            .iter()
            .enumerate()
            .flat_map(|(i, s)| s.content.chars().map(move |c| (c, i)))
            .collect();
        let plain: Vec<char> = chars.iter().map(|(c, _)| *c).collect();
        let mut out = Vec::new();
        let mut start = 0;
        for len in Self::split_lengths(&plain, max_width, tab_width) {
            if len == 0 {
                continue;
            }
            // Regroup the row's characters into pieces of the original spans.
            let mut row: Vec<Span<'_>> = Vec::new();
            let mut text = String::new();
            let mut span = chars[start].1;
            for &(c, i) in &chars[start..start + len] {
                if i != span {
                    row.push(Span::styled(std::mem::take(&mut text), spans[span].style));
                    span = i;
                }
                text.push(c);
            }
            row.push(Span::styled(text, spans[span].style));
            out.push(row);
            start += len;
        }
        out
    }

    fn split_str_at(s: Cow<'_, str>, split_at: usize, tab_width: usize) -> (String, String) {
        let mut current_width = 0;
        for (i, ch) in s.chars().enumerate() {
            current_width += char_width(ch, tab_width);
            if current_width > split_at {
                let (a, b) = split_str_at(s, i);
                return (a.to_string(), b.to_string());
            }
        }

        (s.to_string(), String::new())
    }

    fn split_span_at(span: Span, split_at: usize, tab_width: usize) -> (Span, Span) {
        let (a, b) = Self::split_str_at(span.content, split_at, tab_width);
        (Span::styled(a, span.style), Span::styled(b, span.style))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_wrap() {
        let chars: Vec<char> = "hello world foo".chars().collect();
        assert_eq!(LineWrapper::split_lengths(&chars, 8, 4), vec![6, 6, 3]);
        let rows: Vec<String> = LineWrapper::wrap_line(&chars, 8, 4).into_iter().map(|r| r.into_iter().collect()).collect();
        assert_eq!(rows, vec!["hello ", "world ", "foo"]);
        // A word longer than the row is split.
        let long: Vec<char> = "abcdefghij".chars().collect();
        assert_eq!(LineWrapper::split_lengths(&long, 4, 4), vec![4, 4, 2]);
        // Exactly fitting a word before a space.
        let fit: Vec<char> = "abcd efgh".chars().collect();
        assert_eq!(LineWrapper::split_lengths(&fit, 4, 4), vec![5, 4]);
    }

    #[test]
    fn test_wrap_spans() {
        let spans = vec![Span::raw("Hello"), Span::raw("World")];
        let wrapped_spans = LineWrapper::wrap_spans(spans, 3, 0);

        assert_eq!(wrapped_spans[0], vec![Span::raw("Hel")]);
        assert_eq!(wrapped_spans[1], vec![Span::raw("lo"), Span::raw("W")]);
        assert_eq!(wrapped_spans[2], vec![Span::raw("orl")]);
        assert_eq!(wrapped_spans[3], vec![Span::raw("d")]);
    }

    #[test]
    fn test_wrap_spans_with_emoji() {
        let spans = vec![Span::raw("Hell🙂!")];
        let wrapped_spans = LineWrapper::wrap_spans(spans, 4, 0);

        assert_eq!(wrapped_spans[0], vec![Span::raw("Hell")]);
        assert_eq!(wrapped_spans[1], vec![Span::raw("🙂!")]);
    }

    #[test]
    fn test_split_span_at_with_emoji() {
        let span = Span::raw("🙂!");
        let (left, right) = LineWrapper::split_span_at(span, 2, 0);

        assert_eq!(left, Span::raw("🙂"));
        assert_eq!(right, Span::raw("!"));
    }

    fn test_line_wrapper_determine_split() {
        let line_widths = LineWrapper::determine_split(5, 3);

        assert_eq!(line_widths[0], 3);
        assert_eq!(line_widths[1], 2);

        let line_widths = LineWrapper::determine_split(6, 3);

        assert_eq!(line_widths[0], 3);
        assert_eq!(line_widths[1], 3);
    }
}
