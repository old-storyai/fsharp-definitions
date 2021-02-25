#[derive(Clone)]
pub struct SourceBuilder {
    indent: String,
    code: String,
}

impl Default for SourceBuilder {
    fn default() -> Self {
        SourceBuilder::new(String::from("  "))
    }
}

impl SourceBuilder {
    pub fn todo(value: &str) -> Self {
        let mut def = SourceBuilder::default();
        def.push("(* TODO: ");
        def.push(value);
        def.push(" *)");
        def
    }
    pub fn simple(value: &str) -> Self {
        let mut def = SourceBuilder::default();
        def.push(value);
        def.push("(* simple *)");
        def
    }
    pub fn new(indent: String) -> Self {
        SourceBuilder {
            indent,
            code: String::new(),
        }
    }
    pub fn new_with_same_settings(&self) -> Self {
        SourceBuilder {
            code: String::new(),
            indent: self.indent.clone(),
        }
    }
    pub fn push(&mut self, s: &str) {
        self.code.push_str(s);
    }
    pub fn ln_push(&mut self, s: &str) {
        self.code.push_str("\n");
        self.code.push_str(s);
    }
    pub fn ln_push_1(&mut self, s: &str) {
        self.code.push_str("\n");
        self.code.push_str(&self.indent);
        self.code.push_str(s);
    }
    pub fn push_source(&mut self, other: Self) {
        self.code.extend(other.finish().drain(..));
    }
    pub fn push_source_1(&mut self, other: Self) {
        let indent_1 = "\n".to_owned() + &self.indent;
        self.code
            .extend(other.finish().replace("\n", &indent_1).drain(..));
    }
    pub fn push_source_2(&mut self, other: Self) {
        let indent_2 = "\n".to_owned() + &self.indent + &self.indent;
        self.code
            .extend(other.finish().replace("\n", &indent_2).drain(..));
    }
    pub fn finish(self) -> String {
        self.code
    }
}
