use regex::Regex;
use tracing::{info, debug};

use super::Formula;

pub struct FormulaExtractor {
    patterns: Vec<(Regex, &'static str)>,
}

impl FormulaExtractor {
    pub fn new() -> Self {
        // PDF提取的文本不保留LaTeX语法，公式会变成Unicode数学符号
        // 需要匹配的是渲染后的数学表达式特征
        let patterns = vec![
            // 含多个数学运算符的行（如 x = a + b, f(x) = ...）
            (Regex::new(r"(?m)^[^\n]{0,200}[a-zA-Z]\s*[=≡≈≤≥<>]\s*[^\n]{3,}$").unwrap(), "equation"),

            // Unicode 数学符号密集区域：积分、求和、乘积等
            (Regex::new(r"[∫∑∏∂∇∆√∞±∓≠≡≈≤≥⊂⊃∈∉∀∃∧∨¬⟨⟩⊗⊕⊙]{1}[^\n]{2,100}").unwrap(), "math_symbol"),

            // 上下标模式：含多个数字和字母混合（如 xi, x1, Rn）
            // 分数表达式特征（PDF提取后常见模式）
            (Regex::new(r"(?m)^[^\n]{0,20}[a-zA-Z]\d+[^\n]{0,5}[=+\-*/][^\n]{3,}$").unwrap(), "subscript_expr"),

            // 希腊字母密集行（α, β, γ, θ, λ, μ, σ, ω 等 + 运算符）
            (Regex::new(r"[^\n]{0,50}[αβγδεζηθικλμνξπρστυφχψωΓΔΘΛΞΠΣΦΨΩ][^\n]{0,10}[=+\-<>≤≥≈][^\n]{2,}").unwrap(), "greek_expr"),

            // argmin/argmax, min, max, log, exp, lim, sup, inf 等数学函数
            (Regex::new(r"(?i)(?:arg\s*(?:min|max)|(?:min|max|sup|inf|lim|log|exp|det|tr|diag)\s*[({⟨])").unwrap(), "math_func"),

            // 矩阵/向量表示 (常见如 ||x||, |A|, L(θ))
            (Regex::new(r"(?:\|\|[^\n|]{1,30}\|\||\x{2016}[^\n]{1,30}\x{2016}|[LJEP\x{2112}]\s*\([^\n)]{1,50}\))").unwrap(), "norm_or_loss"),

            // 仍然尝试LaTeX（有些PDF能保留部分LaTeX命令）
            (Regex::new(r"\\(?:frac|int|sum|prod|partial|nabla|lim|infty|alpha|beta|theta|lambda|mathbb|mathcal)\b").unwrap(), "latex_cmd"),
            (Regex::new(r"\$[^\$]{2,}?\$").unwrap(), "inline_latex"),
            (Regex::new(r"\$\$[\s\S]+?\$\$").unwrap(), "display_latex"),
        ];
        Self { patterns }
    }

    /// 从全文中提取公式
    pub fn extract(&self, full_text: &str) -> Vec<Formula> {
        let mut formulas: Vec<Formula> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for (pattern, kind) in &self.patterns {
            for mat in pattern.find_iter(full_text) {
                let raw = mat.as_str().trim().to_string();

                if seen.contains(&raw) {
                    continue;
                }

                // Skip very short or very long matches
                if raw.len() < 4 || raw.len() > 500 {
                    continue;
                }

                // Extract context (up to 50 chars before and after)
                let start = mat.start().saturating_sub(50);
                let end = (mat.end() + 50).min(full_text.len());
                // Ensure we don't split a multi-byte character
                let start = full_text.floor_char_boundary(start);
                let end = full_text.ceil_char_boundary(end);
                let context = full_text[start..end].trim().to_string();

                debug!("公式匹配 [{}]: {}", kind, &raw[..raw.len().min(80)]);

                seen.insert(raw.clone());
                formulas.push(Formula { raw, context });
            }
        }

        info!("公式提取完成，共 {} 个", formulas.len());
        formulas
    }
}
