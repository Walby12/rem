use std::{fs::*, io::prelude::*, io::*, process::exit, sync::LazyLock};
use wasmtime::{Engine, Module};

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The input source file (.rem) to compile
    #[arg(required = true)]
    input_file: PathBuf,

    /// Dumps the AST in the stdout
    #[arg(long, default_value_t = false)]
    ast: bool,

    /// Specify the name of the output executable binary
    #[arg(short, long, default_value = "")]
    output: String,

    /// Autorun the final executable program
    #[arg(short, long, default_value_t = false)]
    run: bool,
}

#[derive(Default, Debug)]
struct ASTLetStmt {
    var_name: String,
    value: i32,
}

#[derive(Debug)]
enum ASTStmt {
    LetStmt(ASTLetStmt),
}

#[derive(Default, Debug)]
struct ASTFuncDef {
    name: String,
    body: Vec<ASTStmt>,
}

#[derive(Debug)]
enum ASTNode {
    FuncDef(ASTFuncDef),
}

#[derive(Debug, PartialEq, Clone)]
enum Token {
    Ident(String),
    Int(i32),
    Fn,
    Let,
    Equals,
    Semicolon,
    OpenParen,
    CloseParen,
    OpenCurly,
    CloseCurly,
    EOF,
}

static KEY_CHARS: LazyLock<Vec<u8>> = LazyLock::new(|| vec![b'(', b')', b'{', b'}', b'=', b';']);

struct Compiler {
    cur_tok: Token,
    src: Vec<u8>,
    index: usize,
    line: usize,
    program: Vec<ASTNode>,
}

impl Compiler {
    fn new(src: String) -> Compiler {
        Compiler {
            cur_tok: Token::EOF,
            src: src.into_bytes(),
            index: 0,
            line: 1,
            program: Vec::new(),
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
            b'=' => {
                self.cur_tok = Token::Equals;
                self.index += 1;
            }
            b';' => {
                self.cur_tok = Token::Semicolon;
                self.index += 1;
            }
            _ => {
                if c.is_ascii_alphabetic() {
                    let mut buff: Vec<u8> = Vec::new();

                    while self.index < self.src.len() {
                        let current_byte = self.src[self.index];

                        if current_byte.is_ascii_alphanumeric()
                            || current_byte == b'_' && !KEY_CHARS.contains(&current_byte)
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
                            "let" => self.cur_tok = Token::Let,
                            _ => self.cur_tok = Token::Ident(str),
                        },
                        Err(e) => {
                            println!("ERROR [line: {}]: Invalid UTF-8 bytes: {}", self.line, e);
                            exit(1);
                        }
                    }
                } else if c.is_ascii_digit() {
                    let mut buff: Vec<u8> = Vec::new();

                    while self.index < self.src.len() {
                        let current_byte = self.src[self.index];

                        if current_byte.is_ascii_digit() && !KEY_CHARS.contains(&current_byte) {
                            buff.push(current_byte);
                            self.index += 1;
                        } else {
                            break;
                        }
                    }

                    match String::from_utf8(buff) {
                        Ok(str) => self.cur_tok = Token::Int(str.parse().unwrap()),
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

    fn build_ast(&mut self) {
        self.lexe();

        while self.cur_tok != Token::EOF {
            match self.cur_tok {
                Token::Fn => self.parse_fn_def(),
                _ => {
                    println!(
                        "ERROR [line: {}]: Unexcpeted operation in global scope: {:?}",
                        self.line, self.cur_tok
                    );
                    exit(1);
                }
            }
            self.lexe();
        }
    }

    fn parse_fn_def(&mut self) {
        self.lexe();

        let mut function = ASTFuncDef::default();

        if let Token::Ident(ref name) = self.cur_tok {
            function.name = name.to_string();
        } else {
            println!(
                "ERROR [line: {}]: Excpected a function name got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }

        self.lexe();
        if self.cur_tok != Token::OpenParen {
            println!(
                "ERROR [line: {}]: Excpected '(' after function name got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }

        self.lexe();
        if self.cur_tok != Token::CloseParen {
            println!(
                "ERROR [line: {}]: Excpected ')' after function name got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }

        self.lexe();
        if self.cur_tok != Token::OpenCurly {
            println!(
                "ERROR [line: {}]: Excpected '{{' after function name got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }

        self.lexe();
        while self.cur_tok != Token::CloseCurly {
            function.body.push(self.parse_stmt());
        }

        self.program.push(ASTNode::FuncDef(function));
    }

    fn parse_stmt(&mut self) -> ASTStmt {
        let stmt;

        match &self.cur_tok {
            Token::Let => {
                stmt = self.parse_let_stmt();
            }
            _ => {
                println!(
                    "ERROR [line: {}]: Unknow statement: {:?}",
                    self.line, self.cur_tok
                );
                exit(1);
            }
        }

        if self.cur_tok != Token::Semicolon {
            println!(
                "ERROR [line: {}]: Excpected ';' after statement got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }

        self.lexe();
        stmt
    }
    fn parse_let_stmt(&mut self) -> ASTStmt {
        let mut variable = ASTLetStmt::default();
        self.lexe();

        if let Token::Ident(ref name) = self.cur_tok {
            variable.var_name = name.to_string();
        } else {
            println!(
                "ERROR [line: {}]: Excpected a variable name got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }

        self.lexe();
        if self.cur_tok != Token::Equals {
            println!(
                "ERROR [line: {}]: Excpected '=' after variable name got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }

        self.lexe();
        if let Token::Int(num) = self.cur_tok {
            variable.value = num;
        } else {
            println!(
                "ERROR [line: {}]: Excpected a number after '=' got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }
        self.lexe();

        ASTStmt::LetStmt(variable)
    }
}

struct Backend {
    module_name: String,
    program: Vec<ASTNode>,
    wat_file: File,
}

impl Backend {
    fn new(wat_file_name: String, module_name: String, program: Vec<ASTNode>) -> Backend {
        let wat_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(wat_file_name)
            .unwrap();

        Backend {
            module_name,
            program,
            wat_file,
        }
    }

    fn compile_program(&mut self) {
        self.wat_file.write_all(b"(module)").unwrap();
        self.wat_file.flush().unwrap();
        self.wat_file.seek(SeekFrom::Start(0)).unwrap();
    }
}

fn main() {
    let args = Args::parse();

    let output_file = if args.output.is_empty() {
        let mut path = PathBuf::from("out");
        if std::env::consts::OS == "windows" {
            path.set_extension("exe");
        }
        path.to_string_lossy().into_owned()
    } else {
        if std::env::consts::OS == "windows" && !args.output.ends_with(".exe") {
            format!("{}.exe", args.output)
        } else {
            args.output
        }
    };

    match args.input_file.extension() {
        Some(ext) if ext == "rem" => {}
        _ => {
            eprintln!("ERROR: Invalid file extension. The Rem compiler only accepts .rem files.");
            exit(1);
        }
    }

    if !args.input_file.exists() {
        println!(
            "ERROR: could not find file {}",
            args.input_file.to_string_lossy()
        );
        exit(1);
    }
    let src = std::fs::read_to_string(args.input_file).unwrap();

    let mut remc = Compiler::new(src);
    remc.build_ast();

    if args.ast {
        println!("{:#?}", remc.program);
    }

    let mut wat_path = PathBuf::from(&output_file);
    wat_path.set_extension("wat");
    let wat_filename = wat_path.to_string_lossy().into_owned();

    let mut wasm_path = PathBuf::from(&output_file);
    wasm_path.set_extension("wasm");
    let wasm_filename = wasm_path.to_string_lossy().into_owned();

    let mut backend = Backend::new(wat_filename, String::from("main"), remc.program);
    backend.compile_program();

    let mut wat_text = String::new();
    backend.wat_file.read_to_string(&mut wat_text).unwrap();

    let wasm_bytes = wat::parse_str(wat_text).unwrap();
    std::fs::write(&wasm_filename, &wasm_bytes).unwrap();

    let engine = Engine::default();

    let module = Module::new(&engine, &wasm_bytes).expect("Failed to compile WASM module");
    let native_exe_bytes = module
        .serialize()
        .expect("Failed to serialize native machine code");

    write(&output_file, &native_exe_bytes).unwrap();

    if args.run {
        std::process::Command::new("wasmtime")
            .args(&["run", "--allow-precompiled", &output_file])
            .status()
            .expect("Failed to execute wasmtime runner");
    }
}
