use std::{process::exit, sync::LazyLock};

#[derive(Debug, PartialEq)]
enum Token {
    Ident(String),
    Fn,
    OpenParen,
    CloseParen,
    OpenCurly,
    CloseCurly,
    EOF,
}

static KEY_CHARS: LazyLock<Vec<u8>> = LazyLock::new(|| vec![b'(', b')', b'{', b'}']);

struct Compiler {
    cur_tok: Token,
    src: Vec<u8>,
    index: usize,
    line: usize,
}

impl Compiler {
    fn new(src: String) -> Compiler {
        Compiler {
            cur_tok: Token::EOF,
            src: src.into_bytes(),
            index: 0,
            line: 1,
        }
    }

    fn lexe(&mut self) {
        while self.index < self.src.len() && self.src[self.index].is_ascii_whitespace() {
            if self.src[self.index] == b'\n' {
                self.line += 1;
            }
            self.index += 1;
        }

        if self.index >= self.src.len() {
            self.cur_tok = Token::EOF;
            return;
        }

        let c = self.src[self.index];

        match c {
            b'(' => {
                self.cur_tok = Token::OpenParen;
                self.index += 1;
            }
            b')' => {
                self.cur_tok = Token::CloseParen;
                self.index += 1;
            }
            b'{' => {
                self.cur_tok = Token::OpenCurly;
                self.index += 1;
            }
            b'}' => {
                self.cur_tok = Token::CloseCurly;
                self.index += 1;
            }
            _ => {
                if c.is_ascii_alphabetic() {
                    let mut buff: Vec<u8> = Vec::new();

                    while self.index < self.src.len() {
                        let current_byte = self.src[self.index];

                        if current_byte.is_ascii_alphanumeric()
                            && !KEY_CHARS.contains(&current_byte)
                        {
                            buff.push(current_byte);
                            self.index += 1;
                        } else {
                            break;
                        }
                    }

                    match String::from_utf8(buff) {
                        Ok(str) => match str.as_str() {
                            "fn" => self.cur_tok = Token::Fn,
                            _ => self.cur_tok = Token::Ident(str),
                        },
                        Err(e) => {
                            println!("ERROR [line: {}]: Invalid UTF-8 bytes: {}", self.line, e);
                            exit(1);
                        }
                    }
                } else {
                    println!(
                        "ERROR [line: {}]: Unknow character: '{}'",
                        self.line, c as char
                    );
                    exit(1);
                }
            }
        }
    }
}

fn main() {
    let mut remc = Compiler::new(String::from("fn main() {}"));
    remc.lexe();
    while remc.cur_tok != Token::EOF {
        println!("Tok; {:?}", remc.cur_tok);
        remc.lexe();
    }
}
