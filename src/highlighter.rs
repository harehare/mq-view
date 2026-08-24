use crate::theme::SyntaxPalette;
use tree_sitter_highlight::{Highlight, HighlightConfiguration, HighlightEvent, Highlighter};

/// Syntax highlighter supporting various programming languages and HTML.
///
/// This struct uses tree-sitter to provide syntax highlighting with ANSI color codes
/// for various programming languages and markup formats.
///
/// # Examples
///
/// ```rust
/// use mq_view::{SyntaxHighlighter, Theme};
///
/// let mut highlighter = SyntaxHighlighter::new(Theme::dark().syntax, false);
/// let code = "fn main() { println!(\"Hello\"); }";
/// let highlighted = highlighter.highlight(code, Some("rust"));
/// println!("{}", highlighted);
/// ```
pub struct SyntaxHighlighter {
    highlighter: Highlighter,
    palette: SyntaxPalette,
    no_color: bool,
}

impl SyntaxHighlighter {
    pub fn new(palette: SyntaxPalette, no_color: bool) -> Self {
        Self {
            highlighter: Highlighter::new(),
            palette,
            no_color,
        }
    }

    /// Get the appropriate tree-sitter language and highlight configuration for a given language
    fn get_highlight_config(lang: &str) -> Option<HighlightConfiguration> {
        let (language, query) = match lang.to_lowercase().as_str() {
            #[cfg(feature = "lang-rust")]
            "rust" | "rs" => (
                tree_sitter_rust::LANGUAGE.into(),
                tree_sitter_rust::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-javascript")]
            "javascript" | "js" => (
                tree_sitter_javascript::LANGUAGE.into(),
                tree_sitter_javascript::HIGHLIGHT_QUERY,
            ),
            #[cfg(feature = "lang-typescript")]
            "typescript" | "ts" => (
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                tree_sitter_typescript::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-typescript")]
            "tsx" => (
                tree_sitter_typescript::LANGUAGE_TSX.into(),
                tree_sitter_typescript::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-python")]
            "python" | "py" => (
                tree_sitter_python::LANGUAGE.into(),
                tree_sitter_python::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-go")]
            "go" => (
                tree_sitter_go::LANGUAGE.into(),
                tree_sitter_go::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-html")]
            "html" => (
                tree_sitter_html::LANGUAGE.into(),
                tree_sitter_html::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-css")]
            "css" => (
                tree_sitter_css::LANGUAGE.into(),
                tree_sitter_css::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-json")]
            "json" => (
                tree_sitter_json::LANGUAGE.into(),
                tree_sitter_json::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-bash")]
            "bash" | "sh" => (
                tree_sitter_bash::LANGUAGE.into(),
                tree_sitter_bash::HIGHLIGHT_QUERY,
            ),
            #[cfg(feature = "lang-c")]
            "c" => (
                tree_sitter_c::LANGUAGE.into(),
                tree_sitter_c::HIGHLIGHT_QUERY,
            ),
            #[cfg(feature = "lang-cpp")]
            "cpp" | "c++" | "cxx" => (
                tree_sitter_cpp::LANGUAGE.into(),
                tree_sitter_cpp::HIGHLIGHT_QUERY,
            ),
            #[cfg(feature = "lang-java")]
            "java" => (
                tree_sitter_java::LANGUAGE.into(),
                tree_sitter_java::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-haskell")]
            "hs" | "haskell" => (
                tree_sitter_haskell::LANGUAGE.into(),
                tree_sitter_haskell::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-elm")]
            "elm" => (
                tree_sitter_elm::LANGUAGE.into(),
                tree_sitter_elm::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-mq")]
            "mq" => (
                tree_sitter_mq::LANGUAGE.into(),
                tree_sitter_mq::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-ocaml")]
            "ocaml" | "ml" => (
                tree_sitter_ocaml::LANGUAGE_OCAML.into(),
                tree_sitter_ocaml::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-swift")]
            "swift" => (
                tree_sitter_swift::LANGUAGE.into(),
                tree_sitter_swift::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-elixir")]
            "ex" | "exs" => (
                tree_sitter_elixir::LANGUAGE.into(),
                tree_sitter_elixir::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-toml")]
            "toml" => (
                tree_sitter_toml_ng::LANGUAGE.into(),
                tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-clojure")]
            "clojure" | "clj" => (
                tree_sitter_clojure::LANGUAGE.into(),
                include_str!("../queries/clojure_highlights.scm"),
            ),
            #[cfg(feature = "lang-yaml")]
            "yaml" | "yml" => (
                tree_sitter_yaml::LANGUAGE.into(),
                tree_sitter_yaml::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-ruby")]
            "ruby" | "rb" => (
                tree_sitter_ruby::LANGUAGE.into(),
                tree_sitter_ruby::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-php")]
            "php" => (
                tree_sitter_php::LANGUAGE_PHP.into(),
                tree_sitter_php::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-lua")]
            "lua" => (
                tree_sitter_lua::LANGUAGE.into(),
                tree_sitter_lua::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-kotlin")]
            "kotlin" | "kt" | "kts" => (
                tree_sitter_kotlin_ng::LANGUAGE.into(),
                include_str!("../queries/kotlin_highlights.scm"),
            ),
            #[cfg(feature = "lang-scala")]
            "scala" => (
                tree_sitter_scala::LANGUAGE.into(),
                tree_sitter_scala::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-make")]
            "make" | "makefile" => (
                tree_sitter_make::LANGUAGE.into(),
                tree_sitter_make::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-sql")]
            "sql" => (
                tree_sitter_sequel::LANGUAGE.into(),
                tree_sitter_sequel::HIGHLIGHTS_QUERY,
            ),
            #[cfg(feature = "lang-dockerfile")]
            "dockerfile" | "docker" => (
                tree_sitter_containerfile::LANGUAGE.into(),
                tree_sitter_containerfile::HIGHLIGHTS_QUERY,
            ),
            _ => return None,
        };

        let mut config = HighlightConfiguration::new(language, "", query, "", "").ok()?;

        config.configure(&[
            "attribute",
            "constant",
            "function.builtin",
            "function",
            "keyword",
            "operator",
            "property",
            "punctuation",
            "punctuation.bracket",
            "punctuation.delimiter",
            "string",
            "string.special",
            "tag",
            "type",
            "type.builtin",
            "variable",
            "variable.builtin",
            "variable.parameter",
            "comment",
            "number",
            "boolean",
            "escape",
            "label",
            "namespace",
            "constructor",
            "embedded",
        ]);

        Some(config)
    }

    /// Highlight code and return colored output
    pub fn highlight(&mut self, code: &str, lang: Option<&str>) -> String {
        // If no language specified or config not available, return plain text
        let Some(lang) = lang else {
            return code.to_string();
        };

        let Some(config) = Self::get_highlight_config(lang) else {
            return code.to_string();
        };

        let palette = self.palette;
        let no_color = self.no_color;
        let highlights = match self
            .highlighter
            .highlight(&config, code.as_bytes(), None, |_| None)
        {
            Ok(h) => h,
            Err(_) => return code.to_string(),
        };

        let mut result = String::new();
        let mut current_pos = 0;

        for event in highlights {
            match event {
                Ok(HighlightEvent::Source { start, end }) => {
                    if start > current_pos {
                        // Add unhighlighted text
                        result.push_str(&code[current_pos..start]);
                    }
                    result.push_str(&code[start..end]);
                    current_pos = end;
                }
                Ok(HighlightEvent::HighlightStart(Highlight(idx))) => {
                    result.push_str(&Self::color_for_highlight(&palette, no_color, idx));
                }
                Ok(HighlightEvent::HighlightEnd) => {
                    if !no_color {
                        result.push_str("\x1b[0m");
                    }
                }
                Err(_) => {}
            }
        }

        // Add any remaining text
        if current_pos < code.len() {
            result.push_str(&code[current_pos..]);
        }

        result
    }

    fn color_for_highlight(palette: &SyntaxPalette, no_color: bool, idx: usize) -> String {
        if no_color {
            return String::new();
        }
        match palette.get(idx) {
            Some(&(r, g, b)) => format!("\x1b[38;2;{r};{g};{b}m"),
            None => "\x1b[0m".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[cfg_attr(
        feature = "lang-rust",
        case::rust("rust", r#"fn main() { println!("Hello, world!"); }"#)
    )]
    #[cfg_attr(
        feature = "lang-python",
        case::python("python", r#"def main(): print("Hello, world!")"#)
    )]
    #[cfg_attr(
        feature = "lang-javascript",
        case::js("javascript", r#"function main() { console.log('Hello, world!'); }"#)
    )]
    #[cfg_attr(
        feature = "lang-typescript",
        case::ts(
            "typescript",
            r#"function main(): void { console.log('Hello, world!'); }"#
        )
    )]
    #[cfg_attr(
        feature = "lang-go",
        case::go("go", r#"func main() { fmt.Println("Hello, world!") }"#)
    )]
    #[cfg_attr(feature = "lang-html", case::html("html", r#"<h1>Hello</h1>"#))]
    #[cfg_attr(feature = "lang-css", case::css("css", r#"body { color: red; }"#))]
    #[cfg_attr(feature = "lang-json", case::json("json", r#"{ "hello": "world" }"#))]
    #[cfg_attr(feature = "lang-bash", case::bash("bash", r#"echo 'Hello, world!'"#))]
    #[cfg_attr(
        feature = "lang-c",
        case::c("c", r#"int main() { printf("Hello, world!"); }"#)
    )]
    #[cfg_attr(feature = "lang-java", case::java("java", r#"public class Main { public static void main(String[] args) { System.out.println("Hello, world!"); } }"#))]
    #[cfg_attr(
        feature = "lang-haskell",
        case::haskell("haskell", r#"main = putStrLn "Hello, world!""#)
    )]
    #[cfg_attr(
        feature = "lang-elm",
        case::elm("elm", r#"main = text "Hello, world!""#)
    )]
    #[cfg_attr(feature = "lang-mq", case::mq("mq", r#"fn(): "Hello, world!""#))]
    #[cfg_attr(feature = "lang-mq", case::bool("mq", r#"fn(): true"#))]
    #[cfg_attr(feature = "lang-mq", case::number("mq", r#"fn(): 42"#))]
    #[cfg_attr(
        feature = "lang-toml",
        case::toml("toml", "[package]\nname = \"hello\"\nversion = \"1.0.0\"")
    )]
    #[cfg_attr(
        feature = "lang-clojure",
        case::clojure("clojure", r#"(defn main [] (println "Hello, world!"))"#)
    )]
    #[cfg_attr(
        feature = "lang-yaml",
        case::yaml("yaml", "name: hello\nversion: 1.0.0")
    )]
    #[cfg_attr(
        feature = "lang-ruby",
        case::ruby("ruby", r#"def main; puts "Hello, world!"; end"#)
    )]
    #[cfg_attr(
        feature = "lang-php",
        case::php("php", r#"<?php function main() { echo "Hello, world!"; }"#)
    )]
    #[cfg_attr(feature = "lang-lua", case::lua("lua", r#"print("Hello, world!")"#))]
    #[cfg_attr(
        feature = "lang-kotlin",
        case::kotlin("kotlin", r#"fun main() { println("Hello, world!") }"#)
    )]
    #[cfg_attr(
        feature = "lang-scala",
        case::scala(
            "scala",
            r#"object Main { def main(args: Array[String]): Unit = println("Hello, world!") }"#
        )
    )]
    #[cfg_attr(
        feature = "lang-make",
        case::make("make", "all:\n\techo \"Hello, world!\"")
    )]
    #[cfg_attr(
        feature = "lang-sql",
        case::sql("sql", "SELECT * FROM users WHERE id = 1;")
    )]
    #[cfg_attr(
        feature = "lang-dockerfile",
        case::dockerfile("dockerfile", "FROM rust:latest\nRUN cargo build")
    )]
    fn test_highlighting_for_supported_languages(#[case] lang: &str, #[case] code: &str) {
        let mut highlighter = SyntaxHighlighter::new(crate::theme::Theme::dark().syntax, false);
        let result = highlighter.highlight(code, Some(lang));
        assert!(
            result.contains("\x1b["),
            "Expected ANSI escape codes for language: {}",
            lang
        );
    }

    #[rstest]
    #[case("unknown", "some code")]
    #[case("unsupported", "another code")]
    fn test_highlighting_for_unsupported_languages(#[case] lang: &str, #[case] code: &str) {
        let mut highlighter = SyntaxHighlighter::new(crate::theme::Theme::dark().syntax, false);
        let result = highlighter.highlight(code, Some(lang));
        assert_eq!(
            result, code,
            "Should return original code for unsupported language: {}",
            lang
        );
    }

    #[test]
    fn test_highlighting_empty_code() {
        let mut highlighter = SyntaxHighlighter::new(crate::theme::Theme::dark().syntax, false);
        let result = highlighter.highlight("", Some("rust"));
        assert_eq!(result, "");
    }

    #[test]
    fn test_highlighting_with_invalid_code() {
        let mut highlighter = SyntaxHighlighter::new(crate::theme::Theme::dark().syntax, false);
        // Intentionally malformed code for rust
        let code = "fn {";
        let result = highlighter.highlight(code, Some("rust"));
        // Should not panic, may or may not contain ANSI codes
        assert!(!result.is_empty());
    }
}
