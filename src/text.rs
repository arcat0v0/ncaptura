pub fn remove_line_breaks(input: &str) -> String {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                paragraphs.push(join_lines(&current));
                current.clear();
            }
        } else {
            current.push(line);
        }
    }

    if !current.is_empty() {
        paragraphs.push(join_lines(&current));
    }

    paragraphs.join("\n\n")
}

fn join_lines(lines: &[&str]) -> String {
    let mut output = String::new();

    for line in lines {
        if output.is_empty() {
            output.push_str(line);
            continue;
        }

        let prev_is_cjk = output.chars().last().is_some_and(is_cjk_char);
        let next_is_cjk = line.chars().next().is_some_and(is_cjk_char);
        if !prev_is_cjk && !next_is_cjk {
            output.push(' ');
        }
        output.push_str(line);
    }

    output
}

fn is_cjk_char(c: char) -> bool {
    matches!(c,
        '\u{3400}'..='\u{4DBF}'
        | '\u{4E00}'..='\u{9FFF}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{3000}'..='\u{303F}'
        | '\u{FF00}'..='\u{FFEF}'
        | '\u{3040}'..='\u{30FF}'
    )
}

#[cfg(test)]
mod tests {
    use super::remove_line_breaks;

    #[test]
    fn joins_cjk_lines_without_space() {
        let input = "截取屏幕中的内容，\n即可快速识别文字。";
        assert_eq!(
            remove_line_breaks(input),
            "截取屏幕中的内容，即可快速识别文字。"
        );
    }

    #[test]
    fn joins_latin_lines_with_space() {
        let input = "hello\nworld";
        assert_eq!(remove_line_breaks(input), "hello world");
    }

    #[test]
    fn keeps_blank_line_paragraphs() {
        let input = "第一段第一行，\n第一段第二行。\n\n第二段第一行，\n第二段第二行。";
        assert_eq!(
            remove_line_breaks(input),
            "第一段第一行，第一段第二行。\n\n第二段第一行，第二段第二行。"
        );
    }

    #[test]
    fn cjk_boundary_joins_without_space() {
        let input = "translate 翻译\n结果 output";
        assert_eq!(remove_line_breaks(input), "translate 翻译结果 output");
    }

    #[test]
    fn latin_cjk_boundary_uses_no_space_on_cjk_side() {
        let input = "word\n中文";
        assert_eq!(remove_line_breaks(input), "word中文");
    }

    #[test]
    fn trims_line_whitespace() {
        let input = "  内容甲，  \n\t内容乙。\n\n   \n内容丙。";
        assert_eq!(remove_line_breaks(input), "内容甲，内容乙。\n\n内容丙。");
    }

    #[test]
    fn collapses_multiple_blank_lines_into_one_break() {
        let input = "甲。\n\n\n\n乙。";
        assert_eq!(remove_line_breaks(input), "甲。\n\n乙。");
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(remove_line_breaks(""), "");
        assert_eq!(remove_line_breaks("  \n \n"), "");
    }
}
