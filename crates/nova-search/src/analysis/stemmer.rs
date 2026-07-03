pub struct PorterStemmer;

impl PorterStemmer {
    pub fn stem(word: &str) -> String {
        if word.len() <= 2 {
            return word.to_lowercase();
        }

        let word = word.to_lowercase();
        let chars: Vec<char> = word.chars().collect();
        if chars.is_empty() {
            return word;
        }

        let mut stem = word;
        stem = Self::step_1a(&stem);
        stem = Self::step_1b(&stem);
        stem = Self::step_1c(&stem);
        stem = Self::step_2(&stem);
        stem = Self::step_3(&stem);
        stem = Self::step_4(&stem);
        stem = Self::step_5a(&stem);
        stem = Self::step_5b(&stem);
        stem
    }

    fn is_consonant(ch: char) -> bool {
        match ch {
            'a' | 'e' | 'i' | 'o' | 'u' => false,
            'y' => true,
            _ => !ch.is_alphabetic() || !"aeiou".contains(ch),
        }
    }

    fn measure(stem: &str) -> usize {
        if stem.is_empty() {
            return 0;
        }
        let chars: Vec<char> = stem.chars().collect();
        let len = chars.len();
        let mut count = 0;
        let mut i = 0;

        while i < len && Self::is_consonant(chars[i]) {
            i += 1;
        }
        if i >= len {
            return 0;
        }

        while i < len {
            while i < len && !Self::is_consonant(chars[i]) {
                i += 1;
            }
            if i >= len {
                break;
            }
            while i < len && Self::is_consonant(chars[i]) {
                i += 1;
            }
            count += 1;
        }

        count
    }

    fn contains_vowel(stem: &str) -> bool {
        stem.chars().any(|c| !Self::is_consonant(c))
    }

    #[allow(dead_code)]
    fn ends_with_double_consonant(stem: &str) -> bool {
        let chars: Vec<char> = stem.chars().collect();
        chars.len() >= 2
            && Self::is_consonant(chars[chars.len() - 1])
            && chars[chars.len() - 1] == chars[chars.len() - 2]
    }

    fn ends_with_cvc(stem: &str) -> bool {
        let chars: Vec<char> = stem.chars().collect();
        if chars.len() < 3 {
            return false;
        }
        let len = chars.len();
        Self::is_consonant(chars[len - 3])
            && !Self::is_consonant(chars[len - 2])
            && Self::is_consonant(chars[len - 1])
            && chars[len - 1] != 'w'
            && chars[len - 1] != 'x'
            && chars[len - 1] != 'y'
    }

    fn step_1a(word: &str) -> String {
        if word.ends_with("sses") {
            return word[..word.len() - 2].to_string();
        }
        if word.ends_with("ies") {
            return word[..word.len() - 2].to_string();
        }
        if word.ends_with("ss") {
            return word.to_string();
        }
        if word.ends_with('s') && !word.ends_with("us") && !word.ends_with("ss") {
            return word[..word.len() - 1].to_string();
        }
        word.to_string()
    }

    fn step_1b(word: &str) -> String {
        let word = word.to_string();
        let eed = word.strip_suffix("eed");
        if let Some(stem) = eed {
            if Self::measure(stem) > 0 {
                return format!("{}ee", stem);
            }
            return word;
        }

        let ed = word.strip_suffix("ed");
        if let Some(stem) = ed {
            if Self::contains_vowel(stem) {
                return Self::step_1b_helper(stem);
            }
            return word;
        }

        let ing = word.strip_suffix("ing");
        if let Some(stem) = ing {
            if Self::contains_vowel(stem) {
                return Self::step_1b_helper(stem);
            }
        }

        word
    }

    fn step_1b_helper(stem: &str) -> String {
        let mut s = stem.to_string();
        let chars: Vec<char> = s.chars().collect();

        if s.ends_with("at") || s.ends_with("bl") || s.ends_with("iz") {
            s.push('e');
            return s;
        }

        if chars.len() >= 2
            && chars[chars.len() - 1] == chars[chars.len() - 2]
            && Self::is_consonant(chars[chars.len() - 1])
            && chars[chars.len() - 1] != 'l'
            && chars[chars.len() - 1] != 's'
            && chars[chars.len() - 1] != 'z'
        {
            s.pop();
            return s;
        }

        if Self::measure(&s) == 1 && Self::ends_with_cvc(&s) {
            s.push('e');
        }

        s
    }

    fn step_1c(word: &str) -> String {
        let chars: Vec<char> = word.chars().collect();
        if chars.len() >= 2
            && chars[chars.len() - 1] == 'y'
            && Self::is_consonant(chars[chars.len() - 2])
        {
            let stem = &word[..word.len() - 1];
            return format!("{}i", stem);
        }
        word.to_string()
    }

    fn step_2(word: &str) -> String {
        let replacements = [
            ("ational", "ate"),
            ("tional", "tion"),
            ("enci", "ence"),
            ("anci", "ance"),
            ("izer", "ize"),
            ("abli", "able"),
            ("alli", "al"),
            ("entli", "ent"),
            ("eli", "e"),
            ("ousli", "ous"),
            ("ization", "ize"),
            ("ation", "ate"),
            ("ator", "ate"),
            ("alism", "al"),
            ("iveness", "ive"),
            ("fulness", "ful"),
            ("ousness", "ous"),
            ("aliti", "al"),
            ("iviti", "ive"),
            ("biliti", "ble"),
        ];

        for (suffix, replacement) in &replacements {
            if let Some(stem) = word.strip_suffix(suffix) {
                if Self::measure(stem) > 0 {
                    return format!("{}{}", stem, replacement);
                }
                return word.to_string();
            }
        }

        word.to_string()
    }

    fn step_3(word: &str) -> String {
        let replacements = [
            ("icate", "ic"),
            ("ative", ""),
            ("alize", "al"),
            ("iciti", "ic"),
            ("ical", "ic"),
            ("ful", ""),
            ("ness", ""),
        ];

        for (suffix, replacement) in &replacements {
            if let Some(stem) = word.strip_suffix(suffix) {
                if Self::measure(stem) > 0 {
                    return format!("{}{}", stem, replacement);
                }
                return word.to_string();
            }
        }

        word.to_string()
    }

    fn step_4(word: &str) -> String {
        let suffixes = [
            "al", "ance", "ence", "er", "ic", "able", "ible", "ant", "ement", "ment", "ent",
            "ism", "ate", "iti", "ous", "ive", "ize",
        ];

        for suffix in &suffixes {
            if let Some(stem) = word.strip_suffix(suffix) {
                if Self::measure(stem) > 1 {
                    return stem.to_string();
                }
                return word.to_string();
            }
        }

        if let Some(stem) = word.strip_suffix("ion") {
            if stem.len() >= 1 && (stem.ends_with('s') || stem.ends_with('t')) {
                if Self::measure(stem) > 1 {
                    return stem.to_string();
                }
            }
        }

        word.to_string()
    }

    fn step_5a(word: &str) -> String {
        if word.ends_with('e') {
            let stem = &word[..word.len() - 1];
            let m = Self::measure(stem);
            if m > 1 {
                return stem.to_string();
            }
            if m == 1 && !Self::ends_with_cvc(stem) {
                return stem.to_string();
            }
        }
        word.to_string()
    }

    fn step_5b(word: &str) -> String {
        let chars: Vec<char> = word.chars().collect();
        if chars.len() >= 2
            && chars[chars.len() - 1] == 'l'
            && chars[chars.len() - 2] == 'l'
            && Self::measure(word) > 1
        {
            return word[..word.len() - 1].to_string();
        }
        word.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_stems() {
        assert_eq!(PorterStemmer::stem("running"), "run");
        assert_eq!(PorterStemmer::stem("runner"), "runner");
        assert_eq!(PorterStemmer::stem("ran"), "ran");
        assert_eq!(PorterStemmer::stem("hoping"), "hope");
        assert_eq!(PorterStemmer::stem("hopping"), "hop");
    }

    #[test]
    fn test_plural() {
        assert_eq!(PorterStemmer::stem("caresses"), "caress");
        assert_eq!(PorterStemmer::stem("cats"), "cat");
        assert_eq!(PorterStemmer::stem("ponies"), "poni");
    }

    #[test]
    fn test_measure() {
        assert_eq!(PorterStemmer::measure("by"), 0);
        assert_eq!(PorterStemmer::measure("trouble"), 1);
        assert_eq!(PorterStemmer::measure("oops"), 1);
    }
}
