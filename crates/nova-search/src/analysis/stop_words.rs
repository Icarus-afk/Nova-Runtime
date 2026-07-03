use std::collections::HashSet;

pub struct StopWordsFilter {
    stop_words: HashSet<String>,
}

impl Default for StopWordsFilter {
    fn default() -> Self {
        let words = "a,an,the,and,or,but,if,when,where,how,what,which,who,whom,this,that,these,those,am,is,are,was,were,be,been,being,have,has,had,having,do,does,did,doing,will,would,can,could,shall,should,may,might,to,for,of,in,on,at,by,with,from,as,into,through,during,before,after,above,below,between,under,again,further,then,once,here,there,not,no,nor,so,yet,both,each,few,more,most,other,some,such,only,own,same,too,very,just,because,about,up,out,off,over,than,also,any,now,ever,never,all,every,enough,rather,quite,thus,per,among,until,while,upon,across,along,around,amongst,amongst,betwixt,thru,via,whereas,whereby,wherein,whereupon,wherewith,wherewithal";
        Self {
            stop_words: words.split(',').map(String::from).collect(),
        }
    }
}

impl StopWordsFilter {
    pub fn new(words: Vec<String>) -> Self {
        StopWordsFilter {
            stop_words: words.into_iter().collect(),
        }
    }

    pub fn is_stop_word(&self, word: &str) -> bool {
        self.stop_words.contains(word)
    }

    pub fn filter(&self, tokens: Vec<crate::analysis::tokenizer::Token>) -> Vec<crate::analysis::tokenizer::Token> {
        tokens
            .into_iter()
            .filter(|t| !self.is_stop_word(&t.term))
            .collect()
    }
}
